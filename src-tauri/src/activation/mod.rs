//! Pro activation / licensing.
//!
//! Security model: the actual activation keys are **never** shipped inside the
//! application. Only the peppered SHA-256 hashes of the valid keys are compiled
//! into the binary. When the user enters a key we hash the (trimmed) input with
//! the same pepper and check it for membership in the embedded set.
//!
//! Because the keys are high-entropy (20 characters over a large symbol set,
//! ~120+ bits), the hashes cannot be reversed or brute-forced, so an attacker
//! reading the binary/strings cannot recover a working key — only a legitimate
//! user who was given a key can activate.
//!
//! The accepted key's hash is persisted (in the `settings` table). On every
//! `is_pro()` check we re-verify that the stored hash is a genuine member of the
//! embedded set, so simply writing an arbitrary value into the database does not
//! unlock Pro — the value has to be the hash of a real key.
//!
//! (As with any fully-offline licence check, a determined attacker who patches
//! the compiled binary can bypass the gate entirely; defeating that requires a
//! server-side check. This scheme meets the stated goal: keys stay secret and
//! only a valid key activates the app.)

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Compiled-in pepper. Combined with each key before hashing so the stored
/// digests are specific to this application.
const PEPPER: &str = "AIO-DL::v1::pro-activation::pepper::8f3c";

/// Peppered SHA-256 (`sha256(PEPPER + "|" + key)`, lowercase hex) of every valid
/// Pro activation key. The plaintext keys are intentionally absent.
const VALID_KEY_HASHES: &[&str] = &[
    "d5f4a32f74df435398d79d7afcc599fef6e8fa26727711e8172942307a99dda2",
    "fec518382fd82507fc5fa229a0c83c3f6470e2e5710f685c2172fa7e2a8f23a1",
    "4f7ccb6f09a04e88f60a623b9ad9cedd9b9c7044b58a2d5ea77257ee0ca0a70c",
    "03686d99190a84d6c85c36c728360df3ea84ef6e208f77b9993729f5947119b5",
    "28ee8792c43bc1ebbbe397d8d5928061faacd4379ec101d2f175a69c61f9afa1",
    "1fb0a0de0e47b721d90992322081e856cc91f2ce867e8be7ae9307ac429e3d76",
    "5b26416402f12d833a5c09ba195a1847009740ec8fc73581ee8d54ce057d7995",
    "518eb21b2261a776b6eb4b02a1d45eb44dc4d0d8b91f250d64a62b31a3ee6d89",
    "164a31564db56e37881108058e0f04cf7dc566c1a6e9e59952d20957766ca128",
    "366d1a1c13ed71443c5e155e4a94984bcfa7ae4ba4d8e85e17b3b70813671bf1",
    "01715fbbd4decbb8ae32c84b647434807312700aa56cd14ec73b2abd81b387db",
    "bfa0fc01187899c232886e8bdee45bfa41ef9a9dc9d509ae4e264fcd93269501",
    "50f66e170499e48c2b943ecea4110095f1aa362925d39ec24957d9e2cf485bd6",
    "1c64b3abdc87ed1146ac5539cb11fafc53c04f6060d67c83293dc3751cfc499f",
    "7282d1b9bd031fa69ac49c19f23daceca6bc5221a4cbcbe155620492f2ff8e72",
    "421f914eef6b356dfd08af2a8ad88f5843f7c46f70edf8a0488e8e8c87c875a7",
    "fa52f1127ee92c081712b9d0a88f8ca2a737b368d7b86d6cfee31b51b7588a1b",
    "5eb3723e6b4bb63b9145e059dba71c580e8b220c6332f0f2a147e914457827bf",
    "fae6b8f89c41185815b6651ef1a9645f34b6cf7c083534a42d69a5cea15613db",
    "2ba924c8796bd0e09eb825eb7f4cb46e77ac80fa1d8164de2e97c7c42e0efc7c",
    "1c0dc5f8135e98241a17f243fd019f2affc1f6fec81cb5824d6d61a49d2f5d27",
    "cd558ac639882a6db3b56ea6a200979bbeb0fb097f9c4e07f5e833f912386429",
    "0aa422d592d72691a136353b0a13b7ae1bda86055d681728dcf7fe8f7575b7d4",
    "58f42889569b11aebe12c4759612687359b73e2505bda6d97d3392695bd6ca19",
    "df2bc472595009cfe3e74b62f00453b2a0235c4d72f4a62fda746d948b216018",
    "fefa2fee044b32d551224fe1934923129a7f01c53c998e522b4e86f193b0266f",
    "6092c30ba24c2e64a4498514f62b026770e2edf7182b068c726222645fb9b553",
    "93f02a14af285e8e3d92c99a774d875509f71d65288eb1aacf9859faa0020755",
    "c966b8f92aab6628e161478044c0f72aa60c045be36602612d0da4bfeadd3d9f",
    "f4038cdf735a0f7fd62342f3dd410a0d90f24f924724a618bcbb83727c767383",
    "86fae721c1df980ea0b59c5a1b4d25abe569be21220aa218dd3e77a2794efefb",
    "a6cbd10f58fcbd5f629ea2b08caf7787368ffe6a7fb4df68681d6b444b77ee54",
    "608647be6abb7aec07e28909d19b27dc2172396a6398002df4800a0688807d64",
    "cbcabdf50e07a33e24b6b53bb8da7d0372f97acbfce833fd4aff7aede57cdca7",
    "e4ceea5adfae02694a30597745719d37a20caf240a3da850c88d1184d4427334",
    "aed4cb93be52d1ce396d67f86cf942f365d566202986eae0c1073c97738fdba9",
    "d7d595a2b281b277246a17a985413ea6cefed3be8e6f3ab1f7bcde334c6bf042",
    "c6a4ebcd439e05e00af99e21e4fd6168c2b4576eb43293cbeb7ee4115effea1b",
    "fb9f87522162842d0784d45a7d8ff40a5f769de2a8a6896cd329dd45727e4a07",
    "6f832eea1b621a6ae8cd7f50bf3663873f57b120fc6daeeec7c6c0cf713d52e4",
    "3fe4fd069bbffd1738f62fdc506505aad0e5613846bee5f8c0944e28191fea9c",
    "3f5797184a8e333711c82f5398c88ae4acd58c330c85a91f64e80a3f43d1b628",
    "b32de1d046d1bb0b1af5878a114650ac77848e489a3a8a27b73859dc5958689a",
    "2f2421bafbaa5bc91477019b4f091b3e3fc7b028c597e7446d4715a9029cf337",
    "c5eae2a42097e5e2ca7920b20ea6ff4294a902744e128c0ebb36634cff55350a",
    "124f7b67b5e775f2a05ac3b90ff994905a1eb7e8764dc9c09044aeb9a08ec4d4",
    "72d1138402281029571f4ad20a5794c44141bd697aaa4fd8d27cd512f7a2dc8a",
    "1a5ee5dd4fb4826bb9339b91ffc3f967aa208bbb5774ea35554c5c23828e6e62",
];

/// State returned to the frontend. Never exposes the key or its hash.
#[derive(Debug, Clone, Serialize)]
pub struct ActivationState {
    pub is_pro: bool,
}

/// Peppered SHA-256 of a candidate key, lowercase hex.
pub fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PEPPER.as_bytes());
    hasher.update(b"|");
    hasher.update(key.trim().as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Constant-time-ish comparison of two equal-length hex digests.
fn hashes_equal(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Is this exact hash one of the embedded valid-key hashes?
pub fn is_valid_hash(hash: &str) -> bool {
    VALID_KEY_HASHES.iter().any(|h| hashes_equal(h, hash))
}

/// Does the user-entered key correspond to a valid activation key?
pub fn is_valid_key(key: &str) -> bool {
    if key.trim().is_empty() {
        return false;
    }
    is_valid_hash(&hash_key(key))
}
