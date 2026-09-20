//! Vendored BLAKE2b (RFC 7693), the companion to the vendored X25519 in
//! `x25519.rs`: a self-contained hash with no external crypto dependency
//! chain, so downstream code that needs a content hash or a salted short
//! identifier can take it from flowlang instead of pulling in another crate.
//!
//! Supports every parameter the reference implementation exposes for the
//! sequential (non-tree) mode: digest lengths from 1 to 64 bytes, an
//! optional key of up to 64 bytes (keyed MAC mode), and the optional 16-byte
//! salt and personalization strings from the BLAKE2 parameter block.
//!
//! Verified against the RFC 7693 Appendix A vector and against independently
//! derived vectors in `tests/blake2b.rs`.
//!
//! ```
//! use flowlang::blake2b::{Blake2b, blake2b};
//!
//! // One shot, 64-byte digest.
//! let digest = blake2b(64, b"abc");
//! assert_eq!(digest.len(), 64);
//!
//! // Streaming, 10-byte digest — the same result as the one-shot form.
//! let mut h = Blake2b::new(10);
//! h.update(b"a");
//! h.update(b"bc");
//! assert_eq!(h.finalize(), blake2b(10, b"abc"));
//! ```

/// Size in bytes of one BLAKE2b compression block.
pub const BLAKE2B_BLOCKBYTES: usize = 128;
/// Largest digest BLAKE2b can produce, in bytes.
pub const BLAKE2B_OUTBYTES: usize = 64;
/// Largest key BLAKE2b accepts, in bytes.
pub const BLAKE2B_KEYBYTES: usize = 64;
/// Exact size of the optional salt, in bytes.
pub const BLAKE2B_SALTBYTES: usize = 16;
/// Exact size of the optional personalization string, in bytes.
pub const BLAKE2B_PERSONALBYTES: usize = 16;

/// The BLAKE2b initialization vector — the same constants as SHA-512's.
const IV: [u64; 8] = [
  0x6a09e667f3bcc908, 0xbb67ae8584caa73b,
  0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1,
  0x510e527fade682d1, 0x9b05688c2b3e6c1f,
  0x1f83d9abfb41bd6b, 0x5be0cd19137e2179,
];

/// Message word permutation schedule. BLAKE2b runs 12 rounds; rounds 10 and
/// 11 reuse rows 0 and 1.
const SIGMA: [[usize; 16]; 12] = [
  [ 0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15],
  [14, 10,  4,  8,  9, 15, 13,  6,  1, 12,  0,  2, 11,  7,  5,  3],
  [11,  8, 12,  0,  5,  2, 15, 13, 10, 14,  3,  6,  7,  1,  9,  4],
  [ 7,  9,  3,  1, 13, 12, 11, 14,  2,  6,  5, 10,  4,  0, 15,  8],
  [ 9,  0,  5,  7,  2,  4, 10, 15, 14,  1, 11, 12,  6,  8,  3, 13],
  [ 2, 12,  6, 10,  0, 11,  8,  3,  4, 13,  7,  5, 15, 14,  1,  9],
  [12,  5,  1, 15, 14, 13,  4, 10,  0,  7,  6,  3,  9,  2,  8, 11],
  [13, 11,  7, 14, 12,  1,  3,  9,  5,  0, 15,  4,  8,  6,  2, 10],
  [ 6, 15, 14,  9, 11,  3,  0,  8, 12,  2, 13,  7,  1,  4, 10,  5],
  [10,  2,  8,  4,  7,  6,  1,  5, 15, 11,  9, 14,  3, 12, 13,  0],
  [ 0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15],
  [14, 10,  4,  8,  9, 15, 13,  6,  1, 12,  0,  2, 11,  7,  5,  3],
];

/// Incremental BLAKE2b hasher.
///
/// Build one with [`Blake2b::new`] (plain hash), [`Blake2b::new_keyed`]
/// (keyed MAC), or [`Blake2b::with_params`] (everything, including salt and
/// personalization), feed it with [`Blake2b::update`] as many times as you
/// like, and take the digest with [`Blake2b::finalize`].
#[derive(Clone)]
pub struct Blake2b {
  /// Chained state.
  h: [u64; 8],
  /// 128-bit byte counter, low word first.
  t: [u64; 2],
  /// Pending input, always compressed one block behind so the final block
  /// can be flagged as such.
  buf: [u8; BLAKE2B_BLOCKBYTES],
  /// Number of valid bytes in `buf`.
  buflen: usize,
  /// Requested digest length in bytes.
  outlen: usize,
}

impl Blake2b {
  /// Creates an unkeyed hasher producing an `outlen`-byte digest.
  ///
  /// # Panics
  /// If `outlen` is 0 or greater than 64.
  pub fn new(outlen: usize) -> Blake2b {
    Blake2b::with_params(outlen, &[], &[], &[])
  }

  /// Creates a keyed hasher (BLAKE2b as a MAC) producing an `outlen`-byte
  /// digest. An empty key is the same as [`Blake2b::new`].
  ///
  /// # Panics
  /// If `outlen` is 0 or greater than 64, or `key` is longer than 64 bytes.
  pub fn new_keyed(outlen: usize, key: &[u8]) -> Blake2b {
    Blake2b::with_params(outlen, key, &[], &[])
  }

  /// Creates a hasher from the full sequential-mode parameter block.
  ///
  /// `salt` and `personal` must each be empty or exactly 16 bytes; an
  /// empty value means "not used" (all zero bytes, which is identical).
  ///
  /// # Panics
  /// If `outlen` is 0 or greater than 64, `key` is longer than 64 bytes, or
  /// `salt`/`personal` is neither empty nor 16 bytes.
  pub fn with_params(outlen: usize, key: &[u8], salt: &[u8], personal: &[u8]) -> Blake2b {
    if outlen == 0 || outlen > BLAKE2B_OUTBYTES {
      panic!("BLAKE2b digest length must be 1..=64 bytes, got {}", outlen);
    }
    if key.len() > BLAKE2B_KEYBYTES {
      panic!("BLAKE2b key must be at most 64 bytes, got {}", key.len());
    }
    if !salt.is_empty() && salt.len() != BLAKE2B_SALTBYTES {
      panic!("BLAKE2b salt must be empty or exactly 16 bytes, got {}", salt.len());
    }
    if !personal.is_empty() && personal.len() != BLAKE2B_PERSONALBYTES {
      panic!("BLAKE2b personalization must be empty or exactly 16 bytes, got {}", personal.len());
    }

    // Parameter block, sequential mode: digest length, key length,
    // fanout = 1, depth = 1, everything else zero except salt/personal.
    let mut h = IV;
    h[0] ^= 0x0101_0000 ^ ((key.len() as u64) << 8) ^ (outlen as u64);
    if !salt.is_empty() {
      h[4] ^= load_u64_le(&salt[0..8]);
      h[5] ^= load_u64_le(&salt[8..16]);
    }
    if !personal.is_empty() {
      h[6] ^= load_u64_le(&personal[0..8]);
      h[7] ^= load_u64_le(&personal[8..16]);
    }

    let mut hasher = Blake2b {
      h,
      t: [0, 0],
      buf: [0u8; BLAKE2B_BLOCKBYTES],
      buflen: 0,
      outlen,
    };

    // A key is hashed as a first block, zero-padded to the block size.
    if !key.is_empty() {
      hasher.buf[..key.len()].copy_from_slice(key);
      hasher.buflen = BLAKE2B_BLOCKBYTES;
    }

    hasher
  }

  /// The digest length this hasher was created with, in bytes.
  pub fn output_size(&self) -> usize {
    self.outlen
  }

  /// Absorbs more input. May be called any number of times; splitting the
  /// input across calls never changes the digest.
  pub fn update(&mut self, data: &[u8]) {
    let mut data = data;
    while !data.is_empty() {
      // Only flush a full buffer once we know there is more input, so the
      // last block is always the one compressed with the final flag.
      if self.buflen == BLAKE2B_BLOCKBYTES {
        self.increment_counter(BLAKE2B_BLOCKBYTES as u64);
        let block = self.buf;
        self.compress(&block, false);
        self.buflen = 0;
      }
      let n = core::cmp::min(data.len(), BLAKE2B_BLOCKBYTES - self.buflen);
      self.buf[self.buflen..self.buflen + n].copy_from_slice(&data[..n]);
      self.buflen += n;
      data = &data[n..];
    }
  }

  /// Consumes the hasher and returns the digest, `output_size()` bytes long.
  pub fn finalize(mut self) -> Vec<u8> {
    self.increment_counter(self.buflen as u64);
    for i in self.buflen..BLAKE2B_BLOCKBYTES {
      self.buf[i] = 0;
    }
    let block = self.buf;
    self.compress(&block, true);

    let mut out = [0u8; BLAKE2B_OUTBYTES];
    for (i, word) in self.h.iter().enumerate() {
      out[i * 8..i * 8 + 8].copy_from_slice(&word.to_le_bytes());
    }
    out[..self.outlen].to_vec()
  }

  /// Consumes the hasher and returns the digest as lowercase hex.
  pub fn finalize_hex(self) -> String {
    to_hex(&self.finalize())
  }

  fn increment_counter(&mut self, inc: u64) {
    let (low, carry) = self.t[0].overflowing_add(inc);
    self.t[0] = low;
    if carry {
      self.t[1] = self.t[1].wrapping_add(1);
    }
  }

  /// The BLAKE2b compression function F (RFC 7693 section 3.2).
  fn compress(&mut self, block: &[u8; BLAKE2B_BLOCKBYTES], last: bool) {
    let mut m = [0u64; 16];
    for i in 0..16 {
      m[i] = load_u64_le(&block[i * 8..i * 8 + 8]);
    }

    let mut v = [0u64; 16];
    v[..8].copy_from_slice(&self.h);
    v[8..].copy_from_slice(&IV);
    v[12] ^= self.t[0];
    v[13] ^= self.t[1];
    if last {
      v[14] = !v[14];
    }

    for s in SIGMA.iter() {
      g(&mut v, 0, 4,  8, 12, m[s[ 0]], m[s[ 1]]);
      g(&mut v, 1, 5,  9, 13, m[s[ 2]], m[s[ 3]]);
      g(&mut v, 2, 6, 10, 14, m[s[ 4]], m[s[ 5]]);
      g(&mut v, 3, 7, 11, 15, m[s[ 6]], m[s[ 7]]);
      g(&mut v, 0, 5, 10, 15, m[s[ 8]], m[s[ 9]]);
      g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
      g(&mut v, 2, 7,  8, 13, m[s[12]], m[s[13]]);
      g(&mut v, 3, 4,  9, 14, m[s[14]], m[s[15]]);
    }

    for i in 0..8 {
      self.h[i] ^= v[i] ^ v[i + 8];
    }
  }
}

/// The BLAKE2b mixing function G (RFC 7693 section 3.1).
#[inline(always)]
fn g(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
  v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
  v[d] = (v[d] ^ v[a]).rotate_right(32);
  v[c] = v[c].wrapping_add(v[d]);
  v[b] = (v[b] ^ v[c]).rotate_right(24);
  v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
  v[d] = (v[d] ^ v[a]).rotate_right(16);
  v[c] = v[c].wrapping_add(v[d]);
  v[b] = (v[b] ^ v[c]).rotate_right(63);
}

#[inline(always)]
fn load_u64_le(bytes: &[u8]) -> u64 {
  let mut b = [0u8; 8];
  b.copy_from_slice(&bytes[..8]);
  u64::from_le_bytes(b)
}

/// Lowercase hex encoding of a digest.
pub fn to_hex(bytes: &[u8]) -> String {
  let mut s = String::with_capacity(bytes.len() * 2);
  for b in bytes {
    s.push_str(&format!("{:02x}", b));
  }
  s
}

/// One-shot unkeyed BLAKE2b with an `outlen`-byte digest (1..=64).
pub fn blake2b(outlen: usize, data: &[u8]) -> Vec<u8> {
  let mut h = Blake2b::new(outlen);
  h.update(data);
  h.finalize()
}

/// One-shot keyed BLAKE2b with an `outlen`-byte digest (1..=64).
pub fn blake2b_keyed(outlen: usize, key: &[u8], data: &[u8]) -> Vec<u8> {
  let mut h = Blake2b::new_keyed(outlen, key);
  h.update(data);
  h.finalize()
}

/// One-shot BLAKE2b-512: the full 64-byte digest.
pub fn blake2b_512(data: &[u8]) -> [u8; 64] {
  let mut out = [0u8; 64];
  out.copy_from_slice(&blake2b(64, data));
  out
}

/// One-shot BLAKE2b-256: a 32-byte digest.
pub fn blake2b_256(data: &[u8]) -> [u8; 32] {
  let mut out = [0u8; 32];
  out.copy_from_slice(&blake2b(32, data));
  out
}
