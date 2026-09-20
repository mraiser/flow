//! Conformance tests for the vendored AES-256 implementation (FIPS 197),
//! through the public API downstream code calls: `Aes256` block encrypt and
//! decrypt, and the block-aligned ECB helpers. As with the X25519 and
//! BLAKE2b suites, these vectors prove the code agrees with the standard
//! and with foreign implementations, not merely with itself.
//!
//! The single-block vector is FIPS 197 Appendix C.3 (the Appendix A.3 key
//! schedule is checked in-module, where the round keys are reachable); the
//! four-block vector is NIST SP 800-38A F.1.5.
//! Every other expected value was derived with OpenSSL's `enc -aes-256-ecb
//! -nopad` before being committed, not copied from this crate's output.

use flowlang::aes::{aes256_ecb_decrypt, aes256_ecb_encrypt, Aes256, AES256_KEYBYTES, AES_BLOCKBYTES};

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

fn key(s: &str) -> [u8; AES256_KEYBYTES] {
    let v = unhex(s);
    let mut k = [0u8; AES256_KEYBYTES];
    k.copy_from_slice(&v);
    k
}

fn block(s: &str) -> [u8; AES_BLOCKBYTES] {
    let v = unhex(s);
    let mut b = [0u8; AES_BLOCKBYTES];
    b.copy_from_slice(&v);
    b
}

/// Encrypts, checks the ciphertext, then decrypts back and checks the
/// round trip — both directions of every vector.
fn check_block(keyhex: &str, plainhex: &str, cipherhex: &str) {
    let cipher = Aes256::new(&key(keyhex));
    let plain = block(plainhex);
    let expected = block(cipherhex);

    let mut b = plain;
    cipher.encrypt_block(&mut b);
    assert_eq!(b, expected, "encrypt key={} pt={}", keyhex, plainhex);

    cipher.decrypt_block(&mut b);
    assert_eq!(b, plain, "decrypt key={} ct={}", keyhex, cipherhex);
}

/// FIPS 197 Appendix C.3: AES-256 single block.
#[test]
fn fips197_appendix_c3() {
    check_block(
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        "00112233445566778899aabbccddeeff",
        "8ea2b7ca516745bfeafc49904b496089",
    );
}

/// A second block under the C.3 key (all zero, OpenSSL-derived) so the
/// schedule is pinned by more than one published ciphertext. The schedule
/// words themselves are checked against Appendix A.3 by the in-module unit
/// test in `src/aes.rs`, where the private round keys are reachable.
#[test]
fn fips197_c3_key_second_block() {
    check_block(
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        "00000000000000000000000000000000",
        "f29000b62a499fd0a9f39a6add2e7780",
    );
}

/// NIST SP 800-38A F.1.5 / F.1.6: ECB-AES256, four blocks.
#[test]
fn sp800_38a_ecb_aes256() {
    let k = key("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
    let plain = unhex(
        "6bc1bee22e409f96e93d7e117393172a\
         ae2d8a571e03ac9c9eb76fac45af8e51\
         30c81c46a35ce411e5fbc1191a0a52ef\
         f69f2445df4f9b17ad2b417be66c3710",
    );
    let expected = unhex(
        "f3eed1bdb5d2a03c064b5a7e3db181f8\
         591ccb10d410ed26dc5ba74a31362870\
         b6ed21b99ca6f4f9f153e7b1beafed1d\
         23304b7a39f9f3ff067d8d8f9e24ecc7",
    );

    // One-shot helpers.
    assert_eq!(aes256_ecb_encrypt(&k, &plain), expected);
    assert_eq!(aes256_ecb_decrypt(&k, &expected), plain);

    // In-place multi-block, and block-by-block, agree.
    let cipher = Aes256::new(&k);
    let mut buf = plain.clone();
    cipher.encrypt_blocks(&mut buf);
    assert_eq!(buf, expected);
    cipher.decrypt_blocks(&mut buf);
    assert_eq!(buf, plain);

    for (i, (p, c)) in plain.chunks(16).zip(expected.chunks(16)).enumerate() {
        let mut b = [0u8; 16];
        b.copy_from_slice(p);
        cipher.encrypt_block(&mut b);
        assert_eq!(&b[..], c, "block {}", i);
    }
}

/// Edge keys and blocks (OpenSSL-derived): all zero and all 0xff.
#[test]
fn edge_keys_and_blocks() {
    check_block(
        "0000000000000000000000000000000000000000000000000000000000000000",
        "00000000000000000000000000000000",
        "dc95c078a2408989ad48a21492842087",
    );
    check_block(
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        "ffffffffffffffffffffffffffffffff",
        "d5f93d6d3311cb309f23621b02fbd5e2",
    );
    check_block(
        "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
        "a0a1a2a3a4a5a6a7a8a9aaabacadaeaf",
        "f127f0a7a45fd1029b0b2141b3c8725c",
    );
}

/// The shape newbound's peer handshake uses: a 36-byte UUID zero-padded to
/// three blocks, encrypted in ECB under an X25519-derived key. Expected
/// ciphertext from OpenSSL.
#[test]
fn zero_padded_uuid_ecb() {
    let k = key("202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f");
    let mut buf = b"123e4567-e89b-12d3-a456-426614174000".to_vec();
    buf.resize(48, 0);
    let expected = unhex(
        "dcb2f98133bf0a03d4165626d6e8fb99\
         4c5c0267cd01d6608cbefc3129c00a5f\
         7766ce58cb59ddcd15213be6e2d32475",
    );
    assert_eq!(aes256_ecb_encrypt(&k, &buf), expected);
    let mut back = aes256_ecb_decrypt(&k, &expected);
    back.resize(36, 0);
    assert_eq!(back, b"123e4567-e89b-12d3-a456-426614174000");
}

/// Any key and any block must round-trip; a different key must not.
#[test]
fn round_trip_many_keys() {
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..200 {
        let mut k = [0u8; 32];
        for chunk in k.chunks_mut(8) {
            chunk.copy_from_slice(&next().to_le_bytes());
        }
        let mut k2 = k;
        k2[31] ^= 1;
        let mut p = [0u8; 16];
        for chunk in p.chunks_mut(8) {
            chunk.copy_from_slice(&next().to_le_bytes());
        }
        let cipher = Aes256::new(&k);
        let mut b = p;
        cipher.encrypt_block(&mut b);
        assert_ne!(b, p);
        let mut wrong = b;
        Aes256::new(&k2).decrypt_block(&mut wrong);
        assert_ne!(wrong, p, "one-bit key change still decrypted");
        cipher.decrypt_block(&mut b);
        assert_eq!(b, p);
    }
}

/// `from_slice` is `new` for a 32-byte slice; `Clone` keeps the schedule;
/// `Debug` never prints key material.
#[test]
fn from_slice_clone_and_debug() {
    let k = key("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
    let a = Aes256::new(&k);
    let b = Aes256::from_slice(&k[..]);
    let c = a.clone();
    let p = block("6bc1bee22e409f96e93d7e117393172a");
    let expected = block("f3eed1bdb5d2a03c064b5a7e3db181f8");
    for cipher in [&a, &b, &c] {
        let mut x = p;
        cipher.encrypt_block(&mut x);
        assert_eq!(x, expected);
    }
    let dbg = format!("{:?}", a);
    assert_eq!(dbg, "Aes256 { .. }");
    assert!(!dbg.contains("603d"), "debug output leaks the key");
}

#[test]
#[should_panic(expected = "key must be exactly 32 bytes")]
fn rejects_short_key() {
    Aes256::from_slice(&[0u8; 16]);
}

#[test]
#[should_panic(expected = "multiple of 16 bytes")]
fn rejects_unaligned_encrypt() {
    Aes256::new(&[0u8; 32]).encrypt_blocks(&mut [0u8; 17]);
}

#[test]
#[should_panic(expected = "multiple of 16 bytes")]
fn rejects_unaligned_decrypt() {
    aes256_ecb_decrypt(&[0u8; 32], &[0u8; 15]);
}
