//! Conformance tests for the vendored BLAKE2b implementation (RFC 7693),
//! through the public API downstream code calls: the streaming `Blake2b`
//! hasher and the one-shot helpers. As with `tests/x25519.rs`, agreeing with
//! itself proves nothing — these vectors prove the code agrees with the
//! standard and with foreign implementations.
//!
//! The "abc" digest is RFC 7693 Appendix A. Every other expected value was
//! derived with an independent implementation (python `hashlib.blake2b`,
//! which wraps the BLAKE2 reference C code) before being committed, not
//! copied from this crate's own output.

use flowlang::blake2b::{blake2b, blake2b_256, blake2b_512, blake2b_keyed, to_hex, Blake2b};

/// Bytes 0x00..=0xFF, repeated as needed; the reference KAT input shape.
fn seq(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 256) as u8).collect()
}

fn hex(bytes: &[u8]) -> String {
    to_hex(bytes)
}

/// RFC 7693 Appendix A: BLAKE2b-512("abc").
#[test]
fn rfc7693_appendix_a() {
    assert_eq!(
        hex(&blake2b(64, b"abc")),
        "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d1\
         7d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
    );
}

/// The empty message still goes through one (final) compression.
#[test]
fn empty_input() {
    assert_eq!(
        hex(&blake2b(64, b"")),
        "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419\
         d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce"
    );
}

/// The digest length is a parameter-block input, so a shorter digest is a
/// different hash, not a truncation of the 64-byte one.
#[test]
fn digest_lengths() {
    assert_eq!(hex(&blake2b(1, b"abc")), "6b");
    assert_eq!(hex(&blake2b(10, b"abc")), "3619b2e9832d748d745e");
    assert_eq!(
        hex(&blake2b(32, b"abc")),
        "bddd813c634239723171ef3fee98579b94964e3bb1cb3e427262c8c068d52319"
    );
    let full = blake2b(64, b"abc");
    assert_ne!(&blake2b(32, b"abc")[..], &full[..32], "32-byte digest is not a prefix of the 64-byte one");
    assert_eq!(&blake2b_256(b"abc")[..], &blake2b(32, b"abc")[..]);
    assert_eq!(&blake2b_512(b"abc")[..], &full[..]);
}

/// Inputs straddling the 128-byte block boundary: exactly one block, one
/// byte short, one byte over, two blocks, and a long multi-block run.
#[test]
fn block_boundaries() {
    let cases: [(usize, &str); 8] = [
        (1, "2fa3f686df876995167e7c2e5d74c4c7b6e48f8068fe0e44208344d480f7904c36963e44115fe3eb2a3ac8694c28bcb4f5a0f3276f2e79487d8219057a506e4b"),
        (127, "b6292669ccd38d5f01caae96ba272c76a879a45743afa0725d83b9ebb26665b731f1848c52f11972b6644f554c064fa90780dbbbf3a89d4fc31f67df3e5857ef"),
        (128, "2319e3789c47e2daa5fe807f61bec2a1a6537fa03f19ff32e87eecbfd64b7e0e8ccff439ac333b040f19b0c4ddd11a61e24ac1fe0f10a039806c5dcc0da3d115"),
        (129, "f59711d44a031d5f97a9413c065d1e614c417ede998590325f49bad2fd444d3e4418be19aec4e11449ac1a57207898bc57d76a1bcf3566292c20c683a5c4648f"),
        (255, "5b21c5fd8868367612474fa2e70e9cfa2201ffeee8fafab5797ad58fefa17c9b5b107da4a3db6320baaf2c8617d5a51df914ae88da3867c2d41f0cc14fa67928"),
        (256, "1ecc896f34d3f9cac484c73f75f6a5fb58ee6784be41b35f46067b9c65c63a6794d3d744112c653f73dd7deb6666204c5a9bfa5b46081fc10fdbe7884fa5cbf8"),
        (257, "d8bfe068de0b4f9fa876a3f8024eb9f7b0029fd5dcf251199e065cee89e1a282c8dbf0442f2ade7294ac1c6be19b388dc990c34d8cb79f5f10c54fa813834fda"),
        (1000, "9fe687126e6566313081b43167cbfa0b4f721b45a5afd4076af327765d63a616478ffbd1cd5fbe4033e8638b8bcf8de6b3978b54a30f1d9d8d68fbe66c2b74cf"),
    ];
    for (n, expected) in cases.iter() {
        assert_eq!(hex(&blake2b(64, &seq(*n))), *expected, "{} bytes", n);
    }
}

/// A 100,000-byte input, so the byte counter runs well past a few blocks.
#[test]
fn long_input() {
    let data = vec![b'x'; 100_000];
    assert_eq!(
        hex(&blake2b(64, &data)),
        "49eeba5c8533f2695abf249fbe35e6287aab43a202ad487b2aad15ef445e1bc2\
         f0327bf8355723ce2be9ebc95e44dd43d9aee1a89a0e9aafa2f1752979483e34"
    );
}

/// Feeding the input in one call, one byte at a time, or in awkward chunk
/// sizes that cross block boundaries must all give the same digest.
#[test]
fn streaming_is_chunk_independent() {
    let data = seq(1000);
    let expected = blake2b(64, &data);

    let mut one_byte = Blake2b::new(64);
    for b in data.iter() {
        one_byte.update(&[*b]);
    }
    assert_eq!(one_byte.finalize(), expected, "one byte at a time");

    for chunk in [7usize, 127, 128, 129, 300] {
        let mut h = Blake2b::new(64);
        for piece in data.chunks(chunk) {
            h.update(piece);
        }
        assert_eq!(h.finalize(), expected, "chunk size {}", chunk);
    }

    let mut with_empty = Blake2b::new(64);
    with_empty.update(&[]);
    with_empty.update(&data[..128]);
    with_empty.update(&[]);
    with_empty.update(&data[128..]);
    with_empty.update(&[]);
    assert_eq!(with_empty.finalize(), expected, "interleaved empty updates");
}

/// Keyed mode (BLAKE2b as a MAC): the key becomes the first block and its
/// length enters the parameter block.
#[test]
fn keyed() {
    let key64 = seq(64);
    assert_eq!(
        hex(&blake2b_keyed(64, &key64, b"")),
        "10ebb67700b1868efb4417987acf4690ae9d972fb7a590c2f02871799aaa4786\
         b5e996e8f0f4eb981fc214b005f42d2ff4233499391653df7aefcbc13fc51568"
    );
    assert_eq!(
        hex(&blake2b_keyed(64, &key64, &seq(256))),
        "b72071e096277edebb8ee5134dd3714996307ba3a55aa4733d412abbe28e909e\
         10e57e6fbfb4ef53b3b960518294ff889a90829254412e2a60b85add07a3674f"
    );
    assert_eq!(
        hex(&blake2b_keyed(64, b"k", b"abc")),
        "aa65cf292e7df1f7439b350072d55485083ccf55b149a400c8c0548233f46447\
         d9f95242a31bf783081c997a6c26e086bc8c0f363dd0c03e8f8edfae0c4aa5ca"
    );
    assert_eq!(
        hex(&blake2b_keyed(32, &key64, b"abc")),
        "dff38c978666dff5631db35ca15535520d134f5c8060ea569c6a178ad393719f"
    );
    // An empty key is plain hashing.
    assert_eq!(blake2b_keyed(64, b"", b"abc"), blake2b(64, b"abc"));
    // The streaming constructor agrees with the one-shot helper.
    let mut h = Blake2b::new_keyed(64, &key64);
    h.update(&seq(100));
    h.update(&seq(256)[100..]);
    assert_eq!(h.finalize(), blake2b_keyed(64, &key64, &seq(256)));
}

/// Salt and personalization occupy words 4-7 of the parameter block.
#[test]
fn salt_and_personalization() {
    let salt = b"0123456789abcdef";
    let person = b"fedcba9876543210";

    let mut h = Blake2b::with_params(64, &[], salt, person);
    h.update(b"abc");
    assert_eq!(
        h.finalize_hex(),
        "a2338021d05db97f4f2c23e47b877d16808014678f7cc8f67b377ea0d99cdaa8\
         256f641f48ed84bd861dd83c84ad2ee3297889c629bd561066d1da35df6b7cc4"
    );

    let mut h = Blake2b::with_params(64, &[], salt, &[]);
    h.update(b"abc");
    assert_eq!(
        h.finalize_hex(),
        "c6104a4b90a13393da2bb00c06b0acf57a31cf55da241aa4d7661f9daec31f40\
         ad0a21ab8e077647680f08bd36d45cd71b395b65d46337dce570d20c0b00b55b"
    );

    let mut h = Blake2b::with_params(64, &[], &[], person);
    h.update(b"abc");
    assert_eq!(
        h.finalize_hex(),
        "8dae986e557b68ac40b0d5b005003030ac4e0df720db42bae449ff9450043f93\
         96ccc12b1ffd1d792e5b02985c7d8c02a2b98f0dd9e8d78281b7242961d497b5"
    );

    // Everything at once: 48-byte digest, 20-byte key, salt, personalization.
    let mut h = Blake2b::with_params(48, &seq(20), salt, person);
    h.update(b"abc");
    assert_eq!(
        h.finalize_hex(),
        "c38620eedf55ce38416424ac5f00cb2b3e95ff13b06b5ceee7063cd1c40263ef\
         67badc4fdc5623958162320360d8b711"
    );

    // Empty salt/personalization is the all-zero block, i.e. plain hashing.
    let mut h = Blake2b::with_params(64, &[], &[0u8; 16], &[0u8; 16]);
    h.update(b"abc");
    assert_eq!(h.finalize(), blake2b(64, b"abc"));
}

/// The shape newbound's peer service uses: a per-request salt prepended to
/// a UUID, hashed to 80 bits.
#[test]
fn salted_uuid_80_bit() {
    let mut h = Blake2b::new(10);
    h.update(b"saltvalue");
    h.update(b"123e4567-e89b-12d3-a456-426614174000");
    assert_eq!(h.output_size(), 10);
    assert_eq!(h.finalize_hex(), "4d83d52c9d0d436541c2");
}

/// A cloned hasher carries its state, so a common prefix can be hashed once
/// and then extended in different ways.
#[test]
fn clone_forks_state() {
    let mut prefix = Blake2b::new(64);
    prefix.update(&seq(200));
    let mut a = prefix.clone();
    let mut b = prefix;
    a.update(b"A");
    b.update(b"B");
    let mut full_a = seq(200);
    full_a.push(b'A');
    let mut full_b = seq(200);
    full_b.push(b'B');
    assert_eq!(a.finalize(), blake2b(64, &full_a));
    assert_eq!(b.finalize(), blake2b(64, &full_b));
}

#[test]
#[should_panic(expected = "digest length")]
fn rejects_zero_length_digest() {
    Blake2b::new(0);
}

#[test]
#[should_panic(expected = "digest length")]
fn rejects_oversized_digest() {
    Blake2b::new(65);
}

#[test]
#[should_panic(expected = "key must be")]
fn rejects_oversized_key() {
    Blake2b::new_keyed(64, &seq(65));
}

#[test]
#[should_panic(expected = "salt must be")]
fn rejects_wrong_size_salt() {
    Blake2b::with_params(64, &[], b"short", &[]);
}

#[test]
#[should_panic(expected = "personalization must be")]
fn rejects_wrong_size_personalization() {
    Blake2b::with_params(64, &[], &[], b"short");
}
