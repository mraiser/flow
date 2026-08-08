//! Percent-decoding is BYTE-oriented: one non-ASCII character arrives as a RUN
//! of consecutive %XX escapes (its UTF-8 bytes), so escapes must be collected
//! into a byte run before UTF-8 validation. Validating each escape alone
//! rejects every byte >= 0x80 and lets multi-byte characters through as
//! literal "%E2%80%94" text.
//!
//! This lives in tests/ rather than beside the source because
//! src/flowlang/http/hex_decode.rs is GENERATED from the data store
//! (data/flowlang/pzhy/ll18/0d37/7c16/pzhyll180d377c16coac.rs) and a rebuild
//! would discard anything added to it.

use flowlang::flowlang::http::hex_decode::hex_decode;

fn check(input: &str, expected: &str) {
    let got = hex_decode(input.to_string());
    assert_eq!(got, expected, "hex_decode({:?})", input);
}

#[test]
fn passes_through_text_with_no_escapes() {
    check("", "");
    check("hello", "hello");
    // raw non-ASCII is not an escape and must survive untouched
    check("café", "café");
}

#[test]
fn decodes_ascii_escapes() {
    check("%20", " ");
    check("a%20b", "a b");
    // the escape for '%' itself
    check("%25", "%");
}

#[test]
fn decodes_multibyte_characters() {
    // 2-byte: é
    check("caf%C3%A9", "café");
    // 3-byte: em-dash
    check("%E2%80%94", "—");
    // 4-byte: emoji
    check("%F0%9F%98%80", "😀");
    // consecutive multi-byte characters
    check("%E2%80%94%E2%80%94", "——");
}

#[test]
fn decodes_mixed_runs() {
    // a single run mixing multi-byte characters and ASCII escapes
    check("caf%C3%A9%20%E2%80%94%20ok", "café — ok");
}

#[test]
fn leaves_malformed_escapes_alone() {
    // bare trailing '%'
    check("100%", "100%");
    // truncated escape
    check("100%4", "100%4");
    // non-hex digits
    check("%GG", "%GG");
}

#[test]
fn falls_back_per_escape_on_invalid_utf8() {
    // a lone continuation byte is not valid UTF-8 on its own
    check("%E2", "%E2");
    // a run that is not valid UTF-8 as a whole degrades to the old
    // escape-by-escape behavior: decode what is valid, pass the rest through
    check("%41%E2", "A%E2");
}
