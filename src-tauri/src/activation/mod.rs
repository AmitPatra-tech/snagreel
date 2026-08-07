//! Pro activation / licensing.
//!
//! Security model: this binary carries an Ed25519 **public** key and nothing
//! else. No activation keys, no hashes of them, and no pepper — all of that
//! lives in the licensing service. When the user enters a key we send it to
//! that service, which checks it against its own records and, if the key is
//! good and has a free seat, returns a short-lived token binding the licence to
//! this device. The token is signed with the matching private key.
//!
//! `verify_token` then checks that signature offline on every `is_pro()` call,
//! so the app keeps working on a plane or behind a firewall right up until the
//! token expires (30 days). We refresh well before that, so a normal user never
//! notices the licence is time-boxed.
//!
//! What this buys over the old embedded-hash scheme: keys can be **revoked**,
//! seats are **limited per key** so one key can't be passed around, and the
//! repository no longer contains anything an attacker could use.
//!
//! (As before, this cannot stop someone who edits and rebuilds the source, or
//! patches the compiled binary — an offline-capable check never can. It raises
//! the floor for casual sharing, which is the realistic threat.)

use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Ed25519 public key matching the licensing service's signing key.
/// Rotating the server key means shipping a new build with this updated.
const LICENSE_PUBLIC_KEY: [u8; 32] = [
    0xd3, 0x34, 0x6c, 0xee, 0xd6, 0x4e, 0xad, 0x90, 0xe2, 0x8f, 0xdf, 0x3f, 0x00, 0xb3, 0x39, 0xc6,
    0x94, 0xf3, 0x4b, 0x1a, 0x75, 0x27, 0xc0, 0xd3, 0x7e, 0x49, 0xcd, 0xf8, 0x63, 0xf7, 0xc8, 0x77,
];

/// Licensing service base URL. Override with `SNAGREEL_LICENSE_URL` when
/// testing against a local `wrangler dev`.
const DEFAULT_LICENSE_URL: &str = "https://snagreel-licensing.hutzonsnagreel.workers.dev";

/// Refresh once the token has less than this long to live.
const REFRESH_WHEN_DAYS_LEFT: i64 = 14;

fn license_url() -> String {
    std::env::var("SNAGREEL_LICENSE_URL")
        .unwrap_or_else(|_| DEFAULT_LICENSE_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// State returned to the frontend. Never exposes the key or the token.
#[derive(Debug, Clone, Serialize)]
pub struct ActivationState {
    pub is_pro: bool,
    /// A licence from before the server-side scheme is present, but can no
    /// longer be checked — the user needs to re-enter their key once.
    pub needs_reactivation: bool,
    /// Unix seconds at which the current licence token lapses.
    pub expires_at: Option<i64>,
}

/// Claims carried by a licence token. Mirrors the Worker's payload.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenClaims {
    pub v: u8,
    #[allow(dead_code)]
    pub kid: i64,
    pub did: String,
    pub tier: String,
    #[allow(dead_code)]
    pub iat: i64,
    pub exp: i64,
}

/// A stable, non-identifying id for this installation.
///
/// It is a random value hashed to hex — it carries nothing about the machine,
/// it just has to stay the same across launches so re-activating doesn't burn a
/// fresh seat every time. A reinstall produces a new one; the service reclaims
/// seats that stop checking in, so that self-corrects.
pub fn new_device_id() -> String {
    let mut hasher = Sha256::new();
    hasher.update(uuid::Uuid::new_v4().as_bytes());
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Verify a licence token's signature and claims.
///
/// Returns the claims only when the signature is genuine, the token has not
/// expired, and it was issued for *this* device — a token copied from another
/// machine verifies cryptographically but is rejected here.
pub fn verify_token(token: &str, device_id: &str) -> Option<TokenClaims> {
    let (payload_b64, signature_b64) = token.trim().split_once('.')?;

    let signature_bytes = URL_SAFE_NO_PAD.decode(signature_b64).ok()?;
    let signature_bytes: [u8; 64] = signature_bytes.try_into().ok()?;
    let signature = Signature::from_bytes(&signature_bytes);

    let verifying_key = VerifyingKey::from_bytes(&LICENSE_PUBLIC_KEY).ok()?;
    // The signature covers the base64 payload exactly as transmitted.
    verifying_key
        .verify_strict(payload_b64.as_bytes(), &signature)
        .ok()?;

    let payload = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let claims: TokenClaims = serde_json::from_slice(&payload).ok()?;

    if claims.v != 1 || claims.tier != "pro" {
        return None;
    }
    if claims.exp <= now_unix() {
        return None;
    }
    if claims.did != device_id {
        return None;
    }
    Some(claims)
}

/// Should we try to renew this token yet?
pub fn needs_refresh(claims: &TokenClaims) -> bool {
    let remaining = claims.exp - now_unix();
    remaining < REFRESH_WHEN_DAYS_LEFT * 24 * 60 * 60
}

#[derive(Serialize)]
struct ActivateRequest<'a> {
    key: &'a str,
    device_id: &'a str,
    app_version: &'a str,
}

#[derive(Deserialize)]
struct ActivateResponse {
    token: String,
}

#[derive(Deserialize)]
struct ErrorResponse {
    #[allow(dead_code)]
    error: String,
    message: Option<String>,
}

/// Exchange an activation key for a signed licence token.
///
/// Errors are already phrased for the user — the service sends back copy for
/// the cases it knows about (bad key, revoked, seat limit) and we supply
/// something sensible for transport failures.
pub async fn request_token(key: &str, device_id: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| "Could not start the activation request.".to_string())?;

    let response = client
        .post(format!("{}/v1/activate", license_url()))
        .json(&ActivateRequest {
            key: key.trim(),
            device_id,
            app_version: env!("CARGO_PKG_VERSION"),
        })
        .send()
        .await
        .map_err(|_| {
            "Could not reach the activation server. Check your internet connection and try again."
                .to_string()
        })?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|_| "The activation server sent an unreadable response.".to_string())?;

    if status.is_success() {
        let parsed: ActivateResponse = serde_json::from_str(&body)
            .map_err(|_| "The activation server sent an unexpected response.".to_string())?;
        // Never store something we can't verify.
        if verify_token(&parsed.token, device_id).is_none() {
            return Err("The activation server sent a licence this app could not verify.".into());
        }
        return Ok(parsed.token);
    }

    let message = serde_json::from_str::<ErrorResponse>(&body)
        .ok()
        .and_then(|e| e.message)
        .unwrap_or_else(|| "That activation key could not be verified.".into());
    Err(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_key_matches_the_service_key() {
        assert_eq!(
            hex(&LICENSE_PUBLIC_KEY),
            "d3346ceed64ead90e28fdf3f00b339c694f34b1a7527c0d37e49cdf863f7c877"
        );
    }

    #[test]
    fn device_ids_are_64_hex_chars_and_unique() {
        // The service rejects anything that isn't exactly this shape.
        let a = new_device_id();
        let b = new_device_id();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        assert_ne!(a, b);
    }

    #[test]
    fn garbage_tokens_are_rejected() {
        let device = new_device_id();
        for token in ["", ".", "not-a-token", "abc.def", "eyJ2IjoxfQ.AAAA"] {
            assert!(verify_token(token, &device).is_none(), "accepted {token:?}");
        }
    }

    /// A token genuinely issued by the licensing service, captured from a local
    /// run. Its expiry will pass one day, so the signature and the policy
    /// checks are asserted separately — the point is that Workers' Ed25519 and
    /// `ed25519-dalek` agree on the wire format.
    const REAL_TOKEN: &str = "eyJ2IjoxLCJraWQiOjEsImRpZCI6ImFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWEiLCJ0aWVyIjoicHJvIiwiaWF0IjoxNzg2MTQwMjA3LCJleHAiOjE3ODg3MzIyMDd9.7bv0AK5gOT09_Z6so-wAA6YZ3yz7KRV47B3ZcHhJSNEsreEakfl6YIPObpXn_E6ObX3FNDP52J3I0wlYWkkNDw";

    #[test]
    fn service_issued_signature_verifies() {
        let (payload_b64, signature_b64) = REAL_TOKEN.split_once('.').unwrap();
        let signature_bytes: [u8; 64] = URL_SAFE_NO_PAD
            .decode(signature_b64)
            .unwrap()
            .try_into()
            .unwrap();
        let verifying_key = VerifyingKey::from_bytes(&LICENSE_PUBLIC_KEY).unwrap();
        verifying_key
            .verify_strict(payload_b64.as_bytes(), &Signature::from_bytes(&signature_bytes))
            .expect("a token the service really issued must verify");

        let claims: TokenClaims =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload_b64).unwrap()).unwrap();
        assert_eq!(claims.v, 1);
        assert_eq!(claims.tier, "pro");
    }

    #[test]
    fn a_token_from_another_device_is_rejected() {
        // The anti-sharing property: copying someone else's licence out of
        // their database gets you nothing, even though it is validly signed.
        assert!(verify_token(REAL_TOKEN, &new_device_id()).is_none());
    }

    /// Full round trip against a real licensing service. Excluded from the
    /// default run because it needs one; enable with:
    ///
    /// ```text
    /// SNAGREEL_LICENSE_URL=http://127.0.0.1:8788 \
    /// SNAGREEL_TEST_KEY=XXXXX-XXXXX-XXXXX-XXXXX \
    /// cargo test -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn activates_against_a_running_service() {
        let key = std::env::var("SNAGREEL_TEST_KEY").expect("set SNAGREEL_TEST_KEY");
        let device_id = new_device_id();

        let token = request_token(&key, &device_id)
            .await
            .expect("activation should succeed");
        let claims = verify_token(&token, &device_id).expect("issued token must verify");
        assert_eq!(claims.tier, "pro");
        assert!(claims.exp > now_unix());
        assert_eq!(claims.did, device_id);

        // A token issued to us must not validate for a different install.
        assert!(verify_token(&token, &new_device_id()).is_none());

        // And a bad key must be refused rather than silently unlocking.
        assert!(request_token("NOTA-REAL-KEY0-0000", &new_device_id())
            .await
            .is_err());
    }

    #[test]
    fn unsigned_payload_is_rejected() {
        // A well-formed payload with a garbage signature must not pass: this is
        // the exact forgery a user would try if they crafted their own token.
        let device = new_device_id();
        let claims = serde_json::json!({
            "v": 1, "kid": 1, "did": device, "tier": "pro",
            "iat": now_unix(), "exp": now_unix() + 3600,
        });
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        let forged = format!("{payload}.{}", URL_SAFE_NO_PAD.encode([0u8; 64]));
        assert!(verify_token(&forged, &device).is_none());
    }
}
