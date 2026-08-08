//! hex_encode must match JavaScript's encodeURIComponent byte-for-byte:
//! percent-encode the UTF-8 BYTES of every character outside the unreserved
//! set, two uppercase hex digits per byte. Expected strings below were
//! generated with node's encodeURIComponent, not derived by hand.
//!
//! Lives in tests/ because src/flowlang/http/hex_encode.rs is GENERATED from
//! the data store and a rebuild would discard anything added to it.

use flowlang::flowlang::http::hex_decode::hex_decode;
use flowlang::flowlang::http::hex_encode::hex_encode;

fn check(input: &str, expected: &str) {
    let got = hex_encode(input.to_string());
    assert_eq!(got, expected, "hex_encode({:?})", input);
}

#[test]
fn matches_encode_uri_component() {
    check("", "");
    check("abc123XYZ", "abc123XYZ");
    // the full JS unreserved set survives unescaped
    check("a-b_c.d!x~y*z'(q)", "a-b_c.d!x~y*z'(q)");
    check("hello world", "hello%20world");
    check(" ", "%20");
    check("100%", "100%25");
    // reserved ASCII is escaped exactly as JS escapes it
    check("a+b/c=d&e?f#g", "a%2Bb%2Fc%3Dd%26e%3Ff%23g");
    // control characters get two digits (the old encoder emitted "%9")
    check("\t", "%09");
    check("\n1", "%0A1");
    // multi-byte characters encode their UTF-8 bytes, not the code point
    check("café", "caf%C3%A9");
    check("—", "%E2%80%94");
    check("😀", "%F0%9F%98%80");
}

#[test]
fn round_trips_through_hex_decode() {
    for input in ["", "hello world", "a-b_c.d!x~y*z'(q)", "100%", "café",
                  "— dash —", "😀ok😀", "\t\n\r", "a+b/c=d&e?f#g",
                  "\u{9A}", "\t1"] {
        let enc = hex_encode(input.to_string());
        let dec = hex_decode(enc.clone());
        assert_eq!(dec, input, "round-trip {:?} via {:?}", input, enc);
    }
}

#[test]
fn distinct_inputs_encode_distinctly() {
    // the old encoder mapped "\t"+"a" and U+009A to case-variants of the
    // same escape; byte-oriented encoding cannot collide
    let a = hex_encode("\ta".to_string());
    let b = hex_encode("\u{9A}".to_string());
    assert_ne!(a.to_uppercase(), b.to_uppercase());
}
