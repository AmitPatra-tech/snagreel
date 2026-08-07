# Snagreel licensing service

A Cloudflare Worker that validates Pro activation keys and issues short-lived,
signed licence tokens. Runs on the free tier (100k requests/day; real usage is a
handful per user per month).

## Why it exists

The app used to carry the hashes of every valid key. That worked, but it meant a
key could never be revoked, a single key could be shared with the world, and the
key material sat in the repository. Now the app carries only an Ed25519 **public
key**, and everything sensitive lives here.

## How activation works

```
app                          worker                         D1
 |  POST /v1/activate          |                             |
 |  {key, device_id}  -------> |                             |
 |                             |  sha256(PEPPER|key) ------> |
 |                             |  <-- seats, revoked ------- |
 |                             |  seat check + record device |
 |  <---- signed token ------- |                             |
 |                                                           |
 |  verify_token() offline on every is_pro() call            |
```

The token carries `{v, kid, did, tier, iat, exp}` and is valid for 30 days. The
app re-verifies the signature locally on every check, so it keeps working
offline, and renews once fewer than 14 days remain. A revoked key simply fails
to renew and lapses at expiry.

Tokens are bound to `device_id`, so copying one out of another machine's
database does nothing — see `a_token_from_another_device_is_rejected` in
`src-tauri/src/activation/mod.rs`.

## Endpoints

| Method | Path            | Auth   | Purpose                                   |
| ------ | --------------- | ------ | ----------------------------------------- |
| POST   | `/v1/activate`  | none   | key + device_id → signed token            |
| POST   | `/admin/keys`   | bearer | mint keys (returned in plaintext **once**) |
| GET    | `/admin/keys`   | bearer | list keys with seat/device counts          |
| POST   | `/admin/revoke` | bearer | revoke or un-revoke a key                  |
| GET    | `/health`       | none   | liveness                                   |

Activation failures are returned as `{error, message}`, and the app shows
`message` verbatim — so error copy is edited here, not in the client.

## Secrets

Set once per environment; none of them are in the repo.

```bash
wrangler secret put PEPPER       # must match the v1.0.0 client pepper
wrangler secret put SIGNING_KEY  # base64 PKCS#8 Ed25519 private key
wrangler secret put ADMIN_TOKEN  # long random string
```

The signing private key lives at `~/.snagreel/licensing-signing.key`. **Losing it
means every installed app stops renewing**, because the matching public key is
compiled into shipped binaries — keep a backup. Rotating it requires shipping a
new build with the new public key in `src-tauri/src/activation/mod.rs`.

## Issuing a key when an invoice arrives

```bash
curl -X POST https://<worker-url>/admin/keys \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H 'content-type: application/json' \
  -d '{"count":1,"seats":3,"note":"invoice #1234 — buyer@example.com"}'
```

The response is the only time the plaintext key exists — only its hash is
stored. Send it to the buyer.

To revoke a refunded or leaked key:

```bash
curl -X POST https://<worker-url>/admin/revoke \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H 'content-type: application/json' \
  -d '{"key":"XXXXX-XXXXX-XXXXX-XXXXX"}'
```

## Local development

```bash
cp .dev.vars.example .dev.vars     # fill in the three secrets
npx wrangler d1 execute snagreel-licensing --local --file=schema.sql
npx wrangler dev
```

Point the desktop app at it with `SNAGREEL_LICENSE_URL=http://127.0.0.1:8788`,
and run the client-side round-trip test:

```bash
cd ../src-tauri
SNAGREEL_LICENSE_URL=http://127.0.0.1:8788 \
SNAGREEL_TEST_KEY=<a key minted locally> \
cargo test -- --ignored
```

## Deploying

```bash
wrangler login
wrangler deploy
```

The D1 binding in `wrangler.toml` already points at the production database,
which is seeded with the 48 keys that shipped in v1.0.0 — those keep working.

## What this does not do

It does not stop someone from editing the source to remove the check and
rebuilding. No offline-capable licence check can, and the repository is public.
It raises the floor for casual key sharing, which is the realistic threat, and
gives us revocation and seat limits.
