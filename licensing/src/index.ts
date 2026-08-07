/**
 * Snagreel licensing service.
 *
 * Issues short-lived, Ed25519-signed license tokens to clients that present a
 * valid activation key. The app verifies those tokens offline against a public
 * key compiled into the binary, so it keeps working without network access
 * until the token expires.
 *
 * What lives here and nowhere else: the pepper, the signing private key, the
 * key hashes, the seat counts and the revocation flag. The desktop app carries
 * only a public key.
 */

export interface Env {
  DB: D1Database;
  /** Peppers key hashes. Must match the value used for the v1.0.0 key list. */
  PEPPER: string;
  /** base64 PKCS#8 Ed25519 private key. */
  SIGNING_KEY: string;
  /** Bearer token guarding /admin/*. */
  ADMIN_TOKEN: string;
}

/** How long an issued token stays valid — also the offline grace period. */
const TOKEN_TTL_SECONDS = 30 * 24 * 60 * 60;

/** A device that hasn't checked in for this long stops consuming a seat. */
const STALE_DEVICE_DAYS = 60;

/** Unambiguous alphabet for generated keys (no O/0, I/1). */
const KEY_ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

// ---------------------------------------------------------------- utilities

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json; charset=utf-8" },
  });
}

function b64url(bytes: ArrayBuffer | Uint8Array): string {
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  let binary = "";
  for (const byte of view) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function decodeBase64(value: string): Uint8Array {
  const binary = atob(value);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
  return out;
}

/**
 * `sha256(PEPPER + "|" + key)`, lowercase hex.
 *
 * Trim is the only normalisation, matching the v1.0.0 client exactly — the 48
 * keys already in customers' hands must keep hashing to their seeded values.
 */
async function hashKey(pepper: string, key: string): Promise<string> {
  const data = new TextEncoder().encode(`${pepper}|${key.trim()}`);
  const digest = await crypto.subtle.digest("SHA-256", data);
  return [...new Uint8Array(digest)]
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/** Workers renamed this algorithm; accept either spelling. */
async function importSigningKey(env: Env): Promise<CryptoKey> {
  const der = decodeBase64(env.SIGNING_KEY);
  try {
    return await crypto.subtle.importKey("pkcs8", der, { name: "Ed25519" }, false, ["sign"]);
  } catch {
    type ImportAlgorithm = Parameters<SubtleCrypto["importKey"]>[2];
    return await crypto.subtle.importKey(
      "pkcs8",
      der,
      { name: "NODE-ED25519", namedCurve: "NODE-ED25519" } as unknown as ImportAlgorithm,
      false,
      ["sign"],
    );
  }
}

/** `base64url(payload).base64url(signature)` — verified offline by the app. */
async function signToken(env: Env, payload: Record<string, unknown>): Promise<string> {
  const encoded = b64url(new TextEncoder().encode(JSON.stringify(payload)));
  const key = await importSigningKey(env);
  const signature = await crypto.subtle.sign(
    { name: (key.algorithm as { name: string }).name },
    key,
    new TextEncoder().encode(encoded),
  );
  return `${encoded}.${b64url(signature)}`;
}

/** Constant-time string compare, so admin auth can't be probed by timing. */
function safeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

function isAdmin(request: Request, env: Env): boolean {
  const header = request.headers.get("authorization") ?? "";
  const token = header.startsWith("Bearer ") ? header.slice(7) : "";
  return token.length > 0 && safeEqual(token, env.ADMIN_TOKEN);
}

/** 20 chars over a 32-symbol alphabet ≈ 100 bits, grouped for readability. */
function generateKey(): string {
  const raw = new Uint8Array(20);
  crypto.getRandomValues(raw);
  const chars = [...raw].map((b) => KEY_ALPHABET[b % KEY_ALPHABET.length]);
  return [0, 5, 10, 15].map((i) => chars.slice(i, i + 5).join("")).join("-");
}

// ----------------------------------------------------------------- handlers

async function handleActivate(request: Request, env: Env): Promise<Response> {
  let body: { key?: string; device_id?: string; app_version?: string };
  try {
    body = await request.json();
  } catch {
    return json({ error: "invalid_request", message: "Malformed request body." }, 400);
  }

  const key = (body.key ?? "").trim();
  const deviceId = (body.device_id ?? "").trim();
  if (!key || !deviceId) {
    return json(
      { error: "invalid_request", message: "Both key and device_id are required." },
      400,
    );
  }
  // A device id is an opaque 64-char hex digest from the client; anything else
  // is either a bug or someone poking at the endpoint.
  if (!/^[0-9a-f]{64}$/.test(deviceId)) {
    return json({ error: "invalid_request", message: "Malformed device_id." }, 400);
  }

  const keyHash = await hashKey(env.PEPPER, key);
  const record = await env.DB.prepare(
    "SELECT id, seats, revoked FROM keys WHERE key_hash = ?",
  )
    .bind(keyHash)
    .first<{ id: number; seats: number; revoked: number }>();

  if (!record) {
    return json(
      { error: "invalid_key", message: "That activation key is not valid." },
      404,
    );
  }
  if (record.revoked) {
    return json(
      {
        error: "revoked",
        message: "This activation key has been revoked. Contact support.",
      },
      403,
    );
  }

  const existing = await env.DB.prepare(
    "SELECT id FROM activations WHERE key_id = ? AND device_id = ?",
  )
    .bind(record.id, deviceId)
    .first<{ id: number }>();

  if (existing) {
    await env.DB.prepare(
      "UPDATE activations SET last_seen = datetime('now'), app_version = ? WHERE id = ?",
    )
      .bind(body.app_version ?? null, existing.id)
      .run();
  } else {
    // Free up seats held by devices that stopped checking in, so a reinstall
    // or a replaced laptop doesn't permanently burn a seat.
    await env.DB.prepare(
      `DELETE FROM activations
        WHERE key_id = ?
          AND last_seen < datetime('now', ?)`,
    )
      .bind(record.id, `-${STALE_DEVICE_DAYS} days`)
      .run();

    const used = await env.DB.prepare(
      "SELECT count(*) AS n FROM activations WHERE key_id = ?",
    )
      .bind(record.id)
      .first<{ n: number }>();

    if ((used?.n ?? 0) >= record.seats) {
      return json(
        {
          error: "seat_limit",
          message: `This key is already active on ${record.seats} devices. Deactivate one, or contact support.`,
        },
        409,
      );
    }

    await env.DB.prepare(
      "INSERT INTO activations (key_id, device_id, app_version) VALUES (?, ?, ?)",
    )
      .bind(record.id, deviceId, body.app_version ?? null)
      .run();
  }

  const now = Math.floor(Date.now() / 1000);
  const expiresAt = now + TOKEN_TTL_SECONDS;
  const token = await signToken(env, {
    v: 1,
    kid: record.id,
    did: deviceId,
    tier: "pro",
    iat: now,
    exp: expiresAt,
  });

  return json({ token, expires_at: expiresAt });
}

async function handleMintKeys(request: Request, env: Env): Promise<Response> {
  let body: { count?: number; seats?: number; note?: string };
  try {
    body = await request.json();
  } catch {
    body = {};
  }

  const count = Math.min(Math.max(body.count ?? 1, 1), 50);
  const seats = Math.min(Math.max(body.seats ?? 3, 1), 100);
  const issued: string[] = [];

  for (let i = 0; i < count; i++) {
    const key = generateKey();
    const hash = await hashKey(env.PEPPER, key);
    await env.DB.prepare(
      "INSERT INTO keys (key_hash, seats, note) VALUES (?, ?, ?)",
    )
      .bind(hash, seats, body.note ?? null)
      .run();
    issued.push(key);
  }

  // The only time these plaintext keys ever exist — they are not recoverable.
  return json({ keys: issued, seats });
}

async function handleRevoke(request: Request, env: Env): Promise<Response> {
  let body: { key?: string; key_hash?: string; revoked?: boolean };
  try {
    body = await request.json();
  } catch {
    return json({ error: "invalid_request" }, 400);
  }

  const hash = body.key_hash ?? (body.key ? await hashKey(env.PEPPER, body.key) : null);
  if (!hash) {
    return json({ error: "invalid_request", message: "Provide key or key_hash." }, 400);
  }

  const revoked = body.revoked === false ? 0 : 1;
  const result = await env.DB.prepare("UPDATE keys SET revoked = ? WHERE key_hash = ?")
    .bind(revoked, hash)
    .run();

  if (!result.meta.changes) {
    return json({ error: "not_found", message: "No key with that hash." }, 404);
  }
  return json({ key_hash: hash, revoked: revoked === 1 });
}

async function handleListKeys(env: Env): Promise<Response> {
  const { results } = await env.DB.prepare(
    `SELECT k.id,
            substr(k.key_hash, 1, 12) AS hash_prefix,
            k.seats,
            k.revoked,
            k.note,
            k.created_at,
            (SELECT count(*) FROM activations a WHERE a.key_id = k.id) AS devices
       FROM keys k
      ORDER BY k.id`,
  ).all();
  return json({ keys: results });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    if (path === "/health") {
      return json({ ok: true });
    }

    if (path.startsWith("/admin/")) {
      if (!isAdmin(request, env)) {
        return json({ error: "unauthorized" }, 401);
      }
      if (path === "/admin/keys" && request.method === "POST") {
        return handleMintKeys(request, env);
      }
      if (path === "/admin/keys" && request.method === "GET") {
        return handleListKeys(env);
      }
      if (path === "/admin/revoke" && request.method === "POST") {
        return handleRevoke(request, env);
      }
      return json({ error: "not_found" }, 404);
    }

    if (path === "/v1/activate" && request.method === "POST") {
      return handleActivate(request, env);
    }

    return json({ error: "not_found" }, 404);
  },
} satisfies ExportedHandler<Env>;
