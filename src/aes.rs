//! Vendored AES-256 (FIPS 197), the companion to the vendored X25519 and
//! BLAKE2b: a self-contained block cipher with no external crypto
//! dependency chain, so downstream code that encrypts with a shared secret
//! can take the cipher from flowlang instead of pulling in another crate.
//!
//! This is the raw block cipher — a 32-byte key, 16-byte blocks, encrypt
//! and decrypt — plus block-aligned ECB helpers. It provides no padding, no
//! IV and no authentication; a mode of operation is the caller's job.
//!
//! On x86-64 with AES-NI and on AArch64 with the ARMv8 cryptography
//! extension the block operations run on the CPU's AES instructions through
//! `core::arch` intrinsics; support is detected once, at key setup, and the
//! choice is recorded in the [`Aes256`] as its [`AesBackend`]. Everywhere
//! else, and on CPUs without the instructions, the same API runs a
//! table-driven software implementation. Like every plain software AES the
//! fallback indexes lookup tables with secret-derived bytes and is therefore
//! not constant-time against a local cache-timing observer; the hardware
//! paths are. Both backends are verified against the FIPS 197 Appendix C.3
//! vector, the Appendix A.3 key schedule, the NIST SP 800-38A ECB vectors
//! and independently derived vectors in `tests/aes.rs`, and against each
//! other.
//!
//! ```
//! use flowlang::aes::Aes256;
//!
//! let key = [0x42u8; 32];
//! let cipher = Aes256::new(&key);
//!
//! let mut block = *b"sixteen byte msg";
//! let plain = block;
//! cipher.encrypt_block(&mut block);
//! assert_ne!(block, plain);
//! cipher.decrypt_block(&mut block);
//! assert_eq!(block, plain);
//! ```

use core::fmt::{Debug, Formatter};

/// Size in bytes of one AES block.
pub const AES_BLOCKBYTES: usize = 16;
/// Size in bytes of an AES-256 key.
pub const AES256_KEYBYTES: usize = 32;

/// Number of rounds for a 256-bit key.
const ROUNDS: usize = 14;
/// Number of 32-bit words in a 256-bit key.
const KEY_WORDS: usize = 8;

const SBOX: [u8; 256] = [
  0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
  0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
  0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
  0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
  0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
  0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
  0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
  0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
  0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
  0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
  0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
  0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
  0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
  0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
  0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
  0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const INV_SBOX: [u8; 256] = [
  0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
  0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
  0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
  0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
  0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
  0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
  0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
  0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
  0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
  0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
  0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
  0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
  0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
  0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
  0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
  0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
];

/// Round constants for the key schedule: rcon[i] = x^(i-1) in GF(2^8).
const RCON: [u8; 8] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];

/// Which implementation an [`Aes256`] runs its block operations on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AesBackend {
  /// Table-driven software, available everywhere.
  Software,
  /// x86-64 AES-NI (`aesenc`/`aesdec` and friends).
  X86Aesni,
  /// AArch64 ARMv8 cryptography extension (`aese`/`aesd`/`aesmc`/`aesimc`).
  Aarch64Aes,
}

impl AesBackend {
  /// The fastest backend this CPU supports. Hardware support is probed
  /// through the standard library's runtime feature detection, which caches
  /// its answer, so calling this often is cheap.
  pub fn detect() -> AesBackend {
    #[cfg(target_arch = "x86_64")]
    {
      if std::arch::is_x86_feature_detected!("aes") {
        return AesBackend::X86Aesni;
      }
    }
    #[cfg(target_arch = "aarch64")]
    {
      if std::arch::is_aarch64_feature_detected!("aes") {
        return AesBackend::Aarch64Aes;
      }
    }
    AesBackend::Software
  }

  /// Whether this backend can run on the current CPU.
  pub fn is_available(self) -> bool {
    match self {
      AesBackend::Software => true,
      #[cfg(target_arch = "x86_64")]
      AesBackend::X86Aesni => std::arch::is_x86_feature_detected!("aes"),
      #[cfg(target_arch = "aarch64")]
      AesBackend::Aarch64Aes => std::arch::is_aarch64_feature_detected!("aes"),
      #[allow(unreachable_patterns)]
      _ => false,
    }
  }
}

/// An expanded AES-256 key. Cheap to clone; build one per key and reuse it
/// for every block. Its `Debug` output never includes key material.
#[derive(Clone)]
pub struct Aes256 {
  /// Round keys 0..=14, each one 16-byte block in FIPS 197 column-major
  /// order (byte `r + 4*c` is row `r`, column `c`).
  round_keys: [[u8; AES_BLOCKBYTES]; ROUNDS + 1],
  /// Round keys 1..=13 passed through InvMixColumns (keys 0 and 14 copied
  /// unchanged), which is what the equivalent inverse cipher consumes on
  /// both x86 (`aesdec`) and AArch64 (`aesd` + `aesimc`). Only filled in
  /// for a hardware backend.
  #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
  dec_round_keys: [[u8; AES_BLOCKBYTES]; ROUNDS + 1],
  backend: AesBackend,
}

impl Debug for Aes256 {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "Aes256 {{ backend: {:?}, .. }}", self.backend)
  }
}

impl Aes256 {
  /// Expands a 32-byte key into the 15 round keys (FIPS 197 section 5.2)
  /// on the fastest backend this CPU supports (see [`AesBackend::detect`]).
  pub fn new(key: &[u8; AES256_KEYBYTES]) -> Aes256 {
    Aes256::with_backend(key, AesBackend::detect())
  }

  /// Like [`Aes256::new`] but always uses the software implementation.
  /// Useful for benchmarking the backends against each other and for
  /// tests; production code wants [`Aes256::new`].
  pub fn new_software(key: &[u8; AES256_KEYBYTES]) -> Aes256 {
    Aes256::with_backend(key, AesBackend::Software)
  }

  /// Expands the key for a specific backend.
  ///
  /// # Panics
  /// If `backend` is not available on this CPU (see
  /// [`AesBackend::is_available`]).
  pub fn with_backend(key: &[u8; AES256_KEYBYTES], backend: AesBackend) -> Aes256 {
    if !backend.is_available() {
      panic!("AES backend {:?} is not available on this CPU", backend);
    }
    // 4 * (Nr + 1) = 60 words.
    let total_words = 4 * (ROUNDS + 1);
    let mut w = [[0u8; 4]; 4 * (ROUNDS + 1)];
    for i in 0..KEY_WORDS {
      w[i] = [key[4 * i], key[4 * i + 1], key[4 * i + 2], key[4 * i + 3]];
    }
    for i in KEY_WORDS..total_words {
      let mut temp = w[i - 1];
      if i % KEY_WORDS == 0 {
        // RotWord, then SubWord, then xor with the round constant.
        temp = [SBOX[temp[1] as usize] ^ RCON[i / KEY_WORDS - 1], SBOX[temp[2] as usize], SBOX[temp[3] as usize], SBOX[temp[0] as usize]];
      } else if i % KEY_WORDS == 4 {
        temp = [SBOX[temp[0] as usize], SBOX[temp[1] as usize], SBOX[temp[2] as usize], SBOX[temp[3] as usize]];
      }
      let prev = w[i - KEY_WORDS];
      w[i] = [prev[0] ^ temp[0], prev[1] ^ temp[1], prev[2] ^ temp[2], prev[3] ^ temp[3]];
    }

    let mut round_keys = [[0u8; AES_BLOCKBYTES]; ROUNDS + 1];
    for r in 0..=ROUNDS {
      for c in 0..4 {
        round_keys[r][4 * c..4 * c + 4].copy_from_slice(&w[4 * r + c]);
      }
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    let dec_round_keys = {
      let mut d = [[0u8; AES_BLOCKBYTES]; ROUNDS + 1];
      match backend {
        // SAFETY: the backend was checked available just above.
        #[cfg(target_arch = "x86_64")]
        AesBackend::X86Aesni => unsafe { aesni::inv_mix_round_keys(&round_keys, &mut d) },
        #[cfg(target_arch = "aarch64")]
        AesBackend::Aarch64Aes => unsafe { armv8::inv_mix_round_keys(&round_keys, &mut d) },
        _ => {}
      }
      d
    };

    Aes256 {
      round_keys,
      #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
      dec_round_keys,
      backend,
    }
  }

  /// The backend this key was expanded for.
  pub fn backend(&self) -> AesBackend {
    self.backend
  }

  /// Builds a cipher from a key slice, which must be exactly 32 bytes.
  ///
  /// # Panics
  /// If `key` is not 32 bytes long.
  pub fn from_slice(key: &[u8]) -> Aes256 {
    if key.len() != AES256_KEYBYTES {
      panic!("AES-256 key must be exactly 32 bytes, got {}", key.len());
    }
    let mut k = [0u8; AES256_KEYBYTES];
    k.copy_from_slice(key);
    Aes256::new(&k)
  }

  /// Encrypts one 16-byte block in place (FIPS 197 section 5.1).
  pub fn encrypt_block(&self, block: &mut [u8; AES_BLOCKBYTES]) {
    match self.backend {
      AesBackend::Software => self.encrypt_block_soft(block),
      // SAFETY: the backend was checked available on this CPU in
      // `with_backend`, and CPU features do not go away.
      #[cfg(target_arch = "x86_64")]
      AesBackend::X86Aesni => unsafe { aesni::encrypt_block(&self.round_keys, block) },
      #[cfg(target_arch = "aarch64")]
      AesBackend::Aarch64Aes => unsafe { armv8::encrypt_block(&self.round_keys, block) },
      #[allow(unreachable_patterns)]
      _ => unreachable!(),
    }
  }

  /// Decrypts one 16-byte block in place (FIPS 197 section 5.3).
  pub fn decrypt_block(&self, block: &mut [u8; AES_BLOCKBYTES]) {
    match self.backend {
      AesBackend::Software => self.decrypt_block_soft(block),
      // SAFETY: as in `encrypt_block`.
      #[cfg(target_arch = "x86_64")]
      AesBackend::X86Aesni => unsafe { aesni::decrypt_block(&self.round_keys, &self.dec_round_keys, block) },
      #[cfg(target_arch = "aarch64")]
      AesBackend::Aarch64Aes => unsafe { armv8::decrypt_block(&self.dec_round_keys, block) },
      #[allow(unreachable_patterns)]
      _ => unreachable!(),
    }
  }

  /// Encrypts every 16-byte block of `data` in place, independently (ECB).
  /// The hardware backends work several blocks at a time here, which is
  /// where their throughput comes from.
  ///
  /// # Panics
  /// If `data.len()` is not a multiple of 16.
  pub fn encrypt_blocks(&self, data: &mut [u8]) {
    check_block_aligned(data.len());
    match self.backend {
      AesBackend::Software => {
        for chunk in data.chunks_exact_mut(AES_BLOCKBYTES) {
          self.encrypt_block_soft(chunk.try_into().unwrap());
        }
      }
      // SAFETY: as in `encrypt_block`.
      #[cfg(target_arch = "x86_64")]
      AesBackend::X86Aesni => unsafe { aesni::encrypt_blocks(&self.round_keys, data) },
      #[cfg(target_arch = "aarch64")]
      AesBackend::Aarch64Aes => unsafe { armv8::encrypt_blocks(&self.round_keys, data) },
      #[allow(unreachable_patterns)]
      _ => unreachable!(),
    }
  }

  /// Decrypts every 16-byte block of `data` in place, independently (ECB).
  ///
  /// # Panics
  /// If `data.len()` is not a multiple of 16.
  pub fn decrypt_blocks(&self, data: &mut [u8]) {
    check_block_aligned(data.len());
    match self.backend {
      AesBackend::Software => {
        for chunk in data.chunks_exact_mut(AES_BLOCKBYTES) {
          self.decrypt_block_soft(chunk.try_into().unwrap());
        }
      }
      // SAFETY: as in `encrypt_block`.
      #[cfg(target_arch = "x86_64")]
      AesBackend::X86Aesni => unsafe { aesni::decrypt_blocks(&self.round_keys, &self.dec_round_keys, data) },
      #[cfg(target_arch = "aarch64")]
      AesBackend::Aarch64Aes => unsafe { armv8::decrypt_blocks(&self.dec_round_keys, data) },
      #[allow(unreachable_patterns)]
      _ => unreachable!(),
    }
  }

  fn encrypt_block_soft(&self, block: &mut [u8; AES_BLOCKBYTES]) {
    add_round_key(block, &self.round_keys[0]);
    for round in 1..ROUNDS {
      sub_bytes(block);
      shift_rows(block);
      mix_columns(block);
      add_round_key(block, &self.round_keys[round]);
    }
    sub_bytes(block);
    shift_rows(block);
    add_round_key(block, &self.round_keys[ROUNDS]);
  }

  fn decrypt_block_soft(&self, block: &mut [u8; AES_BLOCKBYTES]) {
    add_round_key(block, &self.round_keys[ROUNDS]);
    for round in (1..ROUNDS).rev() {
      inv_shift_rows(block);
      inv_sub_bytes(block);
      add_round_key(block, &self.round_keys[round]);
      inv_mix_columns(block);
    }
    inv_shift_rows(block);
    inv_sub_bytes(block);
    add_round_key(block, &self.round_keys[0]);
  }
}

/// x86-64 AES-NI backend. Every function here must only be called after
/// `is_x86_feature_detected!("aes")` returned true, which `Aes256` ensures.
#[cfg(target_arch = "x86_64")]
mod aesni {
  use super::{AES_BLOCKBYTES, ROUNDS};
  use core::arch::x86_64::*;

  /// How many independent blocks the ECB loops keep in flight, so the
  /// pipelined `aesenc`/`aesdec` units are not stalled on one block's
  /// round-to-round latency.
  const LANES: usize = 8;

  #[inline(always)]
  unsafe fn load(k: &[u8; AES_BLOCKBYTES]) -> __m128i {
    _mm_loadu_si128(k.as_ptr() as *const __m128i)
  }

  #[inline(always)]
  unsafe fn load_keys(rk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1]) -> [__m128i; ROUNDS + 1] {
    let mut k = [_mm_setzero_si128(); ROUNDS + 1];
    for r in 0..=ROUNDS {
      k[r] = load(&rk[r]);
    }
    k
  }

  #[inline(always)]
  unsafe fn enc_one(k: &[__m128i; ROUNDS + 1], mut s: __m128i) -> __m128i {
    s = _mm_xor_si128(s, k[0]);
    for r in 1..ROUNDS {
      s = _mm_aesenc_si128(s, k[r]);
    }
    _mm_aesenclast_si128(s, k[ROUNDS])
  }

  /// Equivalent inverse cipher (FIPS 197 section 5.3.5): `aesdec` wants
  /// the InvMixColumns-transformed round keys for rounds 13..=1, the plain
  /// key 14 first and the plain key 0 last.
  #[inline(always)]
  unsafe fn dec_one(k: &[__m128i; ROUNDS + 1], dk: &[__m128i; ROUNDS + 1], mut s: __m128i) -> __m128i {
    s = _mm_xor_si128(s, k[ROUNDS]);
    for r in (1..ROUNDS).rev() {
      s = _mm_aesdec_si128(s, dk[r]);
    }
    _mm_aesdeclast_si128(s, k[0])
  }

  /// Derives the decryption round keys: InvMixColumns of round keys
  /// 1..=13 (`aesimc`), with keys 0 and 14 copied through unchanged. The
  /// same transform the software `inv_mix_columns` computes, on the
  /// instruction built for it.
  #[target_feature(enable = "aes")]
  pub unsafe fn inv_mix_round_keys(
    rk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1],
    out: &mut [[u8; AES_BLOCKBYTES]; ROUNDS + 1],
  ) {
    out[0] = rk[0];
    out[ROUNDS] = rk[ROUNDS];
    for r in 1..ROUNDS {
      let t = _mm_aesimc_si128(load(&rk[r]));
      _mm_storeu_si128(out[r].as_mut_ptr() as *mut __m128i, t);
    }
  }

  #[target_feature(enable = "aes")]
  pub unsafe fn encrypt_block(rk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1], block: &mut [u8; AES_BLOCKBYTES]) {
    let k = load_keys(rk);
    let s = enc_one(&k, _mm_loadu_si128(block.as_ptr() as *const __m128i));
    _mm_storeu_si128(block.as_mut_ptr() as *mut __m128i, s);
  }

  #[target_feature(enable = "aes")]
  pub unsafe fn decrypt_block(
    rk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1],
    drk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1],
    block: &mut [u8; AES_BLOCKBYTES],
  ) {
    let k = load_keys(rk);
    let dk = load_keys(drk);
    let s = dec_one(&k, &dk, _mm_loadu_si128(block.as_ptr() as *const __m128i));
    _mm_storeu_si128(block.as_mut_ptr() as *mut __m128i, s);
  }

  #[target_feature(enable = "aes")]
  pub unsafe fn encrypt_blocks(rk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1], data: &mut [u8]) {
    let k = load_keys(rk);
    let mut wide = data.chunks_exact_mut(AES_BLOCKBYTES * LANES);
    for chunk in &mut wide {
      let p = chunk.as_mut_ptr() as *mut __m128i;
      let mut s = [_mm_setzero_si128(); LANES];
      for i in 0..LANES {
        s[i] = _mm_xor_si128(_mm_loadu_si128(p.add(i)), k[0]);
      }
      for r in 1..ROUNDS {
        for i in 0..LANES {
          s[i] = _mm_aesenc_si128(s[i], k[r]);
        }
      }
      for i in 0..LANES {
        _mm_storeu_si128(p.add(i), _mm_aesenclast_si128(s[i], k[ROUNDS]));
      }
    }
    for chunk in wide.into_remainder().chunks_exact_mut(AES_BLOCKBYTES) {
      let p = chunk.as_mut_ptr() as *mut __m128i;
      _mm_storeu_si128(p, enc_one(&k, _mm_loadu_si128(p)));
    }
  }

  #[target_feature(enable = "aes")]
  pub unsafe fn decrypt_blocks(
    rk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1],
    drk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1],
    data: &mut [u8],
  ) {
    let k = load_keys(rk);
    let dk = load_keys(drk);
    let mut wide = data.chunks_exact_mut(AES_BLOCKBYTES * LANES);
    for chunk in &mut wide {
      let p = chunk.as_mut_ptr() as *mut __m128i;
      let mut s = [_mm_setzero_si128(); LANES];
      for i in 0..LANES {
        s[i] = _mm_xor_si128(_mm_loadu_si128(p.add(i)), k[ROUNDS]);
      }
      for r in (1..ROUNDS).rev() {
        for i in 0..LANES {
          s[i] = _mm_aesdec_si128(s[i], dk[r]);
        }
      }
      for i in 0..LANES {
        _mm_storeu_si128(p.add(i), _mm_aesdeclast_si128(s[i], k[0]));
      }
    }
    for chunk in wide.into_remainder().chunks_exact_mut(AES_BLOCKBYTES) {
      let p = chunk.as_mut_ptr() as *mut __m128i;
      _mm_storeu_si128(p, dec_one(&k, &dk, _mm_loadu_si128(p)));
    }
  }
}

/// AArch64 ARMv8 cryptography extension backend. Every function here must
/// only be called after `is_aarch64_feature_detected!("aes")` returned
/// true, which `Aes256` ensures.
///
/// The ARM instructions split a round differently from x86: `aese` is
/// AddRoundKey then SubBytes and ShiftRows, `aesmc` is MixColumns, and
/// `aesd`/`aesimc` are their inverses. Encryption therefore uses the round
/// keys untransformed, with the final round an `aese` plus a plain xor
/// with the last key. Decryption is the equivalent inverse cipher (FIPS
/// 197 section 5.3.5) just as on x86: the round-key xor of each middle
/// round sits before that round's InvMixColumns, and because InvMixColumns
/// is linear the xor is pushed through it, so `aesd` in rounds 13..=1
/// consumes the InvMixColumns-transformed keys. Using the plain keys there
/// decrypts to garbage — which is exactly what the first cut of this
/// module did.
#[cfg(target_arch = "aarch64")]
mod armv8 {
  use super::{AES_BLOCKBYTES, ROUNDS};
  use core::arch::aarch64::*;

  const LANES: usize = 8;

  #[inline(always)]
  unsafe fn load_keys(rk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1]) -> [uint8x16_t; ROUNDS + 1] {
    let mut k = [vdupq_n_u8(0); ROUNDS + 1];
    for r in 0..=ROUNDS {
      k[r] = vld1q_u8(rk[r].as_ptr());
    }
    k
  }

  #[inline(always)]
  unsafe fn enc_one(k: &[uint8x16_t; ROUNDS + 1], mut s: uint8x16_t) -> uint8x16_t {
    for r in 0..ROUNDS - 1 {
      s = vaesmcq_u8(vaeseq_u8(s, k[r]));
    }
    s = vaeseq_u8(s, k[ROUNDS - 1]);
    veorq_u8(s, k[ROUNDS])
  }

  /// `dk` is the decryption schedule: key 14 and key 0 as expanded, keys
  /// 13..=1 passed through InvMixColumns.
  #[inline(always)]
  unsafe fn dec_one(dk: &[uint8x16_t; ROUNDS + 1], mut s: uint8x16_t) -> uint8x16_t {
    for r in (2..=ROUNDS).rev() {
      s = vaesimcq_u8(vaesdq_u8(s, dk[r]));
    }
    s = vaesdq_u8(s, dk[1]);
    veorq_u8(s, dk[0])
  }

  /// Derives the decryption round keys: InvMixColumns of round keys
  /// 1..=13 (`aesimc`), with keys 0 and 14 copied through unchanged.
  #[target_feature(enable = "aes")]
  pub unsafe fn inv_mix_round_keys(
    rk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1],
    out: &mut [[u8; AES_BLOCKBYTES]; ROUNDS + 1],
  ) {
    out[0] = rk[0];
    out[ROUNDS] = rk[ROUNDS];
    for r in 1..ROUNDS {
      vst1q_u8(out[r].as_mut_ptr(), vaesimcq_u8(vld1q_u8(rk[r].as_ptr())));
    }
  }

  #[target_feature(enable = "aes")]
  pub unsafe fn encrypt_block(rk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1], block: &mut [u8; AES_BLOCKBYTES]) {
    let k = load_keys(rk);
    vst1q_u8(block.as_mut_ptr(), enc_one(&k, vld1q_u8(block.as_ptr())));
  }

  #[target_feature(enable = "aes")]
  pub unsafe fn decrypt_block(drk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1], block: &mut [u8; AES_BLOCKBYTES]) {
    let dk = load_keys(drk);
    vst1q_u8(block.as_mut_ptr(), dec_one(&dk, vld1q_u8(block.as_ptr())));
  }

  #[target_feature(enable = "aes")]
  pub unsafe fn encrypt_blocks(rk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1], data: &mut [u8]) {
    let k = load_keys(rk);
    let mut wide = data.chunks_exact_mut(AES_BLOCKBYTES * LANES);
    for chunk in &mut wide {
      let p = chunk.as_mut_ptr();
      let mut s = [vdupq_n_u8(0); LANES];
      for i in 0..LANES {
        s[i] = vld1q_u8(p.add(i * AES_BLOCKBYTES));
      }
      for r in 0..ROUNDS - 1 {
        for i in 0..LANES {
          s[i] = vaesmcq_u8(vaeseq_u8(s[i], k[r]));
        }
      }
      for i in 0..LANES {
        let t = veorq_u8(vaeseq_u8(s[i], k[ROUNDS - 1]), k[ROUNDS]);
        vst1q_u8(p.add(i * AES_BLOCKBYTES), t);
      }
    }
    for chunk in wide.into_remainder().chunks_exact_mut(AES_BLOCKBYTES) {
      let p = chunk.as_mut_ptr();
      vst1q_u8(p, enc_one(&k, vld1q_u8(p)));
    }
  }

  #[target_feature(enable = "aes")]
  pub unsafe fn decrypt_blocks(drk: &[[u8; AES_BLOCKBYTES]; ROUNDS + 1], data: &mut [u8]) {
    let dk = load_keys(drk);
    let mut wide = data.chunks_exact_mut(AES_BLOCKBYTES * LANES);
    for chunk in &mut wide {
      let p = chunk.as_mut_ptr();
      let mut s = [vdupq_n_u8(0); LANES];
      for i in 0..LANES {
        s[i] = vld1q_u8(p.add(i * AES_BLOCKBYTES));
      }
      for r in (2..=ROUNDS).rev() {
        for i in 0..LANES {
          s[i] = vaesimcq_u8(vaesdq_u8(s[i], dk[r]));
        }
      }
      for i in 0..LANES {
        let t = veorq_u8(vaesdq_u8(s[i], dk[1]), dk[0]);
        vst1q_u8(p.add(i * AES_BLOCKBYTES), t);
      }
    }
    for chunk in wide.into_remainder().chunks_exact_mut(AES_BLOCKBYTES) {
      let p = chunk.as_mut_ptr();
      vst1q_u8(p, dec_one(&dk, vld1q_u8(p)));
    }
  }
}

fn check_block_aligned(len: usize) {
  if len % AES_BLOCKBYTES != 0 {
    panic!("AES ECB input must be a multiple of 16 bytes, got {}", len);
  }
}

#[inline(always)]
fn add_round_key(state: &mut [u8; 16], round_key: &[u8; 16]) {
  for i in 0..16 {
    state[i] ^= round_key[i];
  }
}

#[inline(always)]
fn sub_bytes(state: &mut [u8; 16]) {
  for b in state.iter_mut() {
    *b = SBOX[*b as usize];
  }
}

#[inline(always)]
fn inv_sub_bytes(state: &mut [u8; 16]) {
  for b in state.iter_mut() {
    *b = INV_SBOX[*b as usize];
  }
}

/// Row `r` rotates left by `r` positions. State is column-major, so row `r`
/// is bytes `r, r+4, r+8, r+12`.
#[inline(always)]
fn shift_rows(state: &mut [u8; 16]) {
  let s = *state;
  for r in 1..4 {
    for c in 0..4 {
      state[r + 4 * c] = s[r + 4 * ((c + r) % 4)];
    }
  }
}

#[inline(always)]
fn inv_shift_rows(state: &mut [u8; 16]) {
  let s = *state;
  for r in 1..4 {
    for c in 0..4 {
      state[r + 4 * c] = s[r + 4 * ((c + 4 - r) % 4)];
    }
  }
}

/// Multiplication by x (i.e. by 2) in GF(2^8) modulo x^8 + x^4 + x^3 + x + 1.
#[inline(always)]
fn xtime(x: u8) -> u8 {
  (x << 1) ^ (((x >> 7) & 1) * 0x1b)
}

#[inline(always)]
fn mix_columns(state: &mut [u8; 16]) {
  for c in 0..4 {
    let a0 = state[4 * c];
    let a1 = state[4 * c + 1];
    let a2 = state[4 * c + 2];
    let a3 = state[4 * c + 3];
    // 3*a == 2*a ^ a
    state[4 * c]     = xtime(a0) ^ (xtime(a1) ^ a1) ^ a2 ^ a3;
    state[4 * c + 1] = a0 ^ xtime(a1) ^ (xtime(a2) ^ a2) ^ a3;
    state[4 * c + 2] = a0 ^ a1 ^ xtime(a2) ^ (xtime(a3) ^ a3);
    state[4 * c + 3] = (xtime(a0) ^ a0) ^ a1 ^ a2 ^ xtime(a3);
  }
}

/// InvMixColumns factored as a cheap pre-step followed by MixColumns
/// (FIPS 197 section 5.3.3 / the "efficient implementation" note in the
/// AES proposal): the inverse matrix {0e,0b,0d,09} equals the forward
/// matrix {02,03,01,01} times {05,00,04,00}. Applying the second factor
/// costs two doublings per column instead of sixteen general GF(2^8)
/// multiplications, which is what made decryption twice the cost of
/// encryption before.
#[inline(always)]
fn inv_mix_columns(state: &mut [u8; 16]) {
  for c in 0..4 {
    let a0 = state[4 * c];
    let a1 = state[4 * c + 1];
    let a2 = state[4 * c + 2];
    let a3 = state[4 * c + 3];
    // 4 * (a0 ^ a2) and 4 * (a1 ^ a3); 5a = 4a ^ a.
    let u = xtime(xtime(a0 ^ a2));
    let v = xtime(xtime(a1 ^ a3));
    state[4 * c]     = a0 ^ u;
    state[4 * c + 1] = a1 ^ v;
    state[4 * c + 2] = a2 ^ u;
    state[4 * c + 3] = a3 ^ v;
  }
  mix_columns(state);
}

/// One-shot ECB encryption of block-aligned data.
///
/// # Panics
/// If `data.len()` is not a multiple of 16.
pub fn aes256_ecb_encrypt(key: &[u8; AES256_KEYBYTES], data: &[u8]) -> Vec<u8> {
  let mut out = data.to_vec();
  Aes256::new(key).encrypt_blocks(&mut out);
  out
}

/// One-shot ECB decryption of block-aligned data.
///
/// # Panics
/// If `data.len()` is not a multiple of 16.
pub fn aes256_ecb_decrypt(key: &[u8; AES256_KEYBYTES], data: &[u8]) -> Vec<u8> {
  let mut out = data.to_vec();
  Aes256::new(key).decrypt_blocks(&mut out);
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  /// FIPS 197 Appendix A.3: the expansion of the 256-bit key
  /// 000102...1f. Words 0-7 are the key, words 8-15 are the first derived
  /// round key pair, and words 56-59 are the last round key. The schedule
  /// is private, so this lives in-module; `tests/aes.rs` covers the public
  /// API.
  #[test]
  fn fips197_appendix_a3_key_expansion() {
    let mut key = [0u8; 32];
    for i in 0..32 {
      key[i] = i as u8;
    }
    let cipher = Aes256::new(&key);

    let words = |round: usize| -> [u32; 4] {
      let rk = cipher.round_keys[round];
      let mut w = [0u32; 4];
      for c in 0..4 {
        w[c] = u32::from_be_bytes([rk[4 * c], rk[4 * c + 1], rk[4 * c + 2], rk[4 * c + 3]]);
      }
      w
    };

    assert_eq!(words(0), [0x00010203, 0x04050607, 0x08090a0b, 0x0c0d0e0f], "w[0..4]");
    assert_eq!(words(1), [0x10111213, 0x14151617, 0x18191a1b, 0x1c1d1e1f], "w[4..8]");
    assert_eq!(words(2), [0xa573c29f, 0xa176c498, 0xa97fce93, 0xa572c09c], "w[8..12]");
    assert_eq!(words(3), [0x1651a8cd, 0x0244beda, 0x1a5da4c1, 0x0640bade], "w[12..16]");
    assert_eq!(words(14), [0x24fc79cc, 0xbf0979e9, 0x371ac23c, 0x6d68de36], "w[56..60]");
  }
}

#[cfg(all(test, any(target_arch = "x86_64", target_arch = "aarch64")))]
mod hw_tests {
  use super::*;

  /// `aesimc` and the software InvMixColumns must produce the same
  /// decryption key schedule; this pins the hardware key derivation to the
  /// software reference directly, not only through decrypt results.
  #[test]
  fn aesimc_matches_software_inv_mix_columns() {
    let backend = AesBackend::detect();
    if backend == AesBackend::Software {
      return;
    }
    let mut key = [0u8; 32];
    for i in 0..32 {
      key[i] = (i * 7 + 3) as u8;
    }
    let cipher = Aes256::with_backend(&key, backend);
    let mut expected = cipher.round_keys;
    for r in 1..ROUNDS {
      inv_mix_columns(&mut expected[r]);
    }
    assert_eq!(cipher.dec_round_keys, expected);
  }
}
