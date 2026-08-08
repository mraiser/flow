//! RFC 7748 conformance for the vendored X25519 implementation, through the
//! two public functions newbound's peer service actually calls. Peers
//! interoperating with each other only proves the code agrees with itself —
//! these vectors prove it agrees with the standard (and therefore with any
//! foreign implementation on the other end of a handshake).
//!
//! Every expected value below was re-derived with an independent
//! implementation (python `cryptography`, OpenSSL-backed) before being
//! committed, not copied from this crate's own output.

use flowlang::x25519::{generate_x25519_keypair, x25519, X25519_BASEPOINT_BYTES};

fn hex32(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
    }
    out
}

/// RFC 7748 section 5.2, vector 1.
#[test]
fn rfc7748_vector_1() {
    let scalar = hex32("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4");
    let u = hex32("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c");
    let expected = hex32("c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552");
    assert_eq!(x25519(scalar, u), expected);
}

/// RFC 7748 section 5.2, vector 2. The input u-coordinate has its high bit
/// set, so this vector specifically exercises the rule that bit 255 of the
/// u-coordinate is masked off before use.
#[test]
fn rfc7748_vector_2() {
    let scalar = hex32("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d");
    let u = hex32("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493");
    let expected = hex32("95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957");
    assert_eq!(x25519(scalar, u), expected);
}

/// RFC 7748 section 5.2 iterated test: k = u = basepoint, then repeatedly
/// k' = X25519(k, u); u = k; k = k'. Checked after 1 and after 1,000
/// iterations (the RFC's 1,000,000-round value is omitted for time).
#[test]
fn rfc7748_iterated() {
    let mut k = X25519_BASEPOINT_BYTES;
    let mut u = X25519_BASEPOINT_BYTES;
    for i in 0..1000 {
        let out = x25519(k, u);
        u = k;
        k = out;
        if i == 0 {
            assert_eq!(
                k,
                hex32("422c8e7a6227d7bca1350b3e2bb7279f7897b87bb6854b783c60e80311ae3079"),
                "after 1 iteration"
            );
        }
    }
    assert_eq!(
        k,
        hex32("684cf59ba83309552800ef566f2f4d3c1c3887c49360e3875f2eb94d99532c51"),
        "after 1,000 iterations"
    );
}

/// RFC 7748 section 6.1: the fixed Alice/Bob Diffie-Hellman vectors —
/// public-key derivation (scalar x basepoint) and the shared secret from
/// both directions.
#[test]
fn rfc7748_diffie_hellman() {
    let alice_priv = hex32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    let alice_pub = hex32("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
    let bob_priv = hex32("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
    let bob_pub = hex32("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
    let shared = hex32("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");

    assert_eq!(x25519(alice_priv, X25519_BASEPOINT_BYTES), alice_pub, "Alice's public key");
    assert_eq!(x25519(bob_priv, X25519_BASEPOINT_BYTES), bob_pub, "Bob's public key");
    assert_eq!(x25519(alice_priv, bob_pub), shared, "shared secret, Alice's side");
    assert_eq!(x25519(bob_priv, alice_pub), shared, "shared secret, Bob's side");
}

/// Freshly generated keypairs must agree on a shared secret, and the public
/// key must be the private key's scalar multiple of the basepoint.
#[test]
fn generated_keypairs_agree() {
    for _ in 0..8 {
        let (a_priv, a_pub) = generate_x25519_keypair();
        let (b_priv, b_pub) = generate_x25519_keypair();
        assert_eq!(x25519(a_priv, X25519_BASEPOINT_BYTES), a_pub);
        assert_eq!(x25519(b_priv, X25519_BASEPOINT_BYTES), b_pub);
        let k1 = x25519(a_priv, b_pub);
        let k2 = x25519(b_priv, a_pub);
        assert_eq!(k1, k2, "shared secrets disagree");
        assert_ne!(k1, [0u8; 32], "all-zero shared secret");
    }
}
