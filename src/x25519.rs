use crate::rand::fill_bytes;
use core::fmt::{Debug, Formatter};
use core::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

//const DOTHEDALEK:bool = false;

pub fn generate_x25519_keypair() -> ([u8; 32], [u8; 32]) {
  //if DOTHEDALEK {
  //    generate_x25519_keypair_with_dalek()
  //}
  //else {
  generate_x25519_keypair_no_dalek()
  //}
}

pub fn x25519(private_key: [u8; 32], peer_public_key: [u8; 32]) -> [u8; 32] {
  //if DOTHEDALEK {
  //    x25519_with_dalek(private_key, peer_public_key)
  //}
  //else {
  x25519_no_dalek(private_key, peer_public_key)
  //}
}




/*
 *
 * /// Generates a new X25519 keypair using the x25519-dalek crate.
 * ///
 * /// Returns a tuple of `(private_key, public_key)`, both as `[u8; 32]`.
 * /// The returned private key is the clamped secret.
 * pub fn generate_x25519_keypair_with_dalek() -> ([u8; 32], [u8; 32]) {
 * //pub fn generate_x25519_keypair() -> ([u8; 32], [u8; 32]) {
 *    // 1. Generate a new StaticSecret.
 *    //    `StaticSecret::new` (or `random_from_rng` in older versions) uses OsRng
 *    //    and handles clamping internally.
 *    let static_secret = StaticSecret::new(&mut OsRng);
 *
 *    // 2. Derive the PublicKey from the StaticSecret.
 *    let public_key = PublicKey::from(&static_secret);
 *
 *    // 3. Return the byte representations.
 *    //    `static_secret.to_bytes()` returns the clamped 32-byte private key.
 *    //    `public_key.as_bytes()` returns the 32-byte public key.
 *    (static_secret.to_bytes(), *public_key.as_bytes())
 * }
 *
 * /// Performs the X25519 Diffie-Hellman function using the x25519-dalek crate.
 * ///
 * /// # Arguments
 * ///
 * /// * `private_key`: Your 32-byte X25519 private key.
 * ///   This will be clamped by `StaticSecret::from()`.
 * /// * `peer_public_key`: The peer's 32-byte X25519 public key.
 * ///
 * /// # Returns
 * ///
 * /// A 32-byte shared secret.
 * pub fn x25519_with_dalek(private_key: [u8; 32], peer_public_key: [u8; 32]) -> [u8; 32] {
 * //pub fn x25519(private_key: [u8; 32], peer_public_key: [u8; 32]) -> [u8; 32] {
 *    // 1. Create a StaticSecret from the provided private key bytes.
 *    //    This step includes the X25519 clamping.
 *    let my_static_secret = StaticSecret::from(private_key);
 *
 *    // 2. Create a PublicKey from the peer's public key bytes.
 *    let peer_public_key_obj = PublicKey::from(peer_public_key);
 *
 *    // 3. Perform the Diffie-Hellman operation.
 *    let shared_secret_obj: SharedSecret = my_static_secret.diffie_hellman(&peer_public_key_obj);
 *
 *    // 4. Return the shared secret as bytes.
 *    shared_secret_obj.to_bytes()
 * }
 *
 *
 */










/// Generates a new X25519 keypair.
///
/// Returns a tuple of `(private_key, public_key)`, both as `[u8; 32]`.
/// The returned private key is already clamped.
pub fn generate_x25519_keypair_no_dalek() -> ([u8; 32], [u8; 32]) {
  let mut private_key_random_bytes = [0u8; 32];
  fill_bytes(&mut private_key_random_bytes);

  let clamped_private_key = clamp_integer(private_key_random_bytes);
  let basepoint = MontgomeryPoint(X25519_BASEPOINT_BYTES);

  // Directly use the scalar multiplication logic
  // The MontgomeryPoint * Scalar logic is defined in montgomery.rs
  let s = Scalar::new(clamped_private_key);
  let public_key_point = &basepoint * &s; // This uses the Mul impl

  (clamped_private_key, public_key_point.to_bytes())
}

/// Performs the X25519 Diffie-Hellman function.
///
/// # Arguments
/// * `private_key`: Your 32-byte X25519 private key.
///   This function will clamp it internally as required by X25519.
/// * `peer_public_key`: The peer's 32-byte X25519 public key.
///
/// # Returns
/// A 32-byte shared secret.
pub fn x25519_no_dalek(private_key: [u8; 32], peer_public_key: [u8; 32]) -> [u8; 32] {
  let clamped_private_key = clamp_integer(private_key);
  let peer_public_point = MontgomeryPoint(peer_public_key);

  let s = Scalar::new(clamped_private_key);
  let shared_secret_point = &peer_public_point * &s; // This uses the Mul impl

  shared_secret_point.to_bytes()
}




// This is (A+2)/4 where A=486662 for the Montgomery curve y^2 = x^3 + Ax^2 + x
// (486662+2)/4 = 486664/4 = 121666
// This is used internally within the Montgomery ladder.
pub(crate) const APLUS2_OVER_FOUR: FieldElement51 =
FieldElement51::from_limbs_const([121666, 0, 0, 0, 0]);

/// Precomputed value of one of the square roots of -1 (mod p)
/// Used in FieldElement::sqrt_ratio_i, which is used in to_edwards,
/// but not directly in the X25519 scalar multiplication path.
/// Keeping it for completeness of the FieldElement code we pulled.
pub(crate) const SQRT_M1: FieldElement51 = FieldElement51::from_limbs_const([
  1718705420411056,
  234908883556509,
  2233514472574048,
  2117202627021982,
  765476049583133,
]);

/// The X25519 basepoint u-coordinate, as bytes. u = 9.
pub const X25519_BASEPOINT_BYTES: [u8; 32] = [
  9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
0, 0,
];





/// A `FieldElement51` represents an element of the field Z / (2^255 - 19).
/// It is represented in radix 2^51 as five u64s.
#[derive(Copy, Clone)]
pub struct FieldElement51(pub [u64; 5]); // Made pub for access from other modules in crypto_core

impl Debug for FieldElement51 {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "FieldElement51({:?})", &self.0[..])
  }
}

// --- Equality and Conditional Selection ---
impl Eq for FieldElement51 {}

impl PartialEq for FieldElement51 {
  fn eq(&self, other: &FieldElement51) -> bool {
    self.ct_eq(other).into()
  }
}

impl ConstantTimeEq for FieldElement51 {
  fn ct_eq(&self, other: &FieldElement51) -> Choice {
    // To be constant time, this requires that the encoding is canonical.
    // self.as_bytes() produces a canonical encoding.
    self.as_bytes().ct_eq(&other.as_bytes())
  }
}

impl ConditionallySelectable for FieldElement51 {
  fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
    FieldElement51([
      u64::conditional_select(&a.0[0], &b.0[0], choice),
                   u64::conditional_select(&a.0[1], &b.0[1], choice),
                   u64::conditional_select(&a.0[2], &b.0[2], choice),
                   u64::conditional_select(&a.0[3], &b.0[3], choice),
                   u64::conditional_select(&a.0[4], &b.0[4], choice),
    ])
  }
}

// --- Arithmetic Trait Implementations (from backend/serial/u64/field.rs) ---
impl<'b> AddAssign<&'b FieldElement51> for FieldElement51 {
  fn add_assign(&mut self, rhs: &'b FieldElement51) {
    for i in 0..5 {
      self.0[i] += rhs.0[i];
    }
    // N.B. AddAssign does not reduce for efficiency
  }
}

impl<'a, 'b> Add<&'b FieldElement51> for &'a FieldElement51 {
  type Output = FieldElement51;
  fn add(self, rhs: &'b FieldElement51) -> FieldElement51 {
    let mut output = *self;
    output += rhs;
    // Add does not reduce for efficiency with current impl strategy.
    // Reduction typically happens after a sequence of ops, like in mul or square.
    // Or explicitly via `reduce()` or `as_bytes()`.
    // For standalone `a+b`, if reduction is needed, caller should do it.
    // The high-level field.rs methods (like invert) handle their own reductions.
    output
  }
}

impl<'b> SubAssign<&'b FieldElement51> for FieldElement51 {
  fn sub_assign(&mut self, rhs: &'b FieldElement51) {
    let result = (self as &FieldElement51) - rhs; // Calls the Sub impl below
    self.0 = result.0;
  }
}

impl<'a, 'b> Sub<&'b FieldElement51> for &'a FieldElement51 {
  type Output = FieldElement51;
  fn sub(self, rhs: &'b FieldElement51) -> FieldElement51 {
    // Matches the original curve25519-dalek u64 backend sub
    FieldElement51::reduce_after_sub([
      (self.0[0] + 36028797018963664u64) - rhs.0[0], // PRECOMPUTED_P_TIMES_16[0]
                                     (self.0[1] + 36028797018963952u64) - rhs.0[1], // PRECOMPUTED_P_TIMES_16[1..]
                                     (self.0[2] + 36028797018963952u64) - rhs.0[2],
                                     (self.0[3] + 36028797018963952u64) - rhs.0[3],
                                     (self.0[4] + 36028797018963952u64) - rhs.0[4],
    ])
  }
}

impl<'b> MulAssign<&'b FieldElement51> for FieldElement51 {
  fn mul_assign(&mut self, rhs: &'b FieldElement51) {
    let result = (self as &FieldElement51) * rhs; // Calls the Mul impl below
    self.0 = result.0;
  }
}

impl<'a, 'b> Mul<&'b FieldElement51> for &'a FieldElement51 {
  type Output = FieldElement51;
  #[rustfmt::skip]
  fn mul(self, rhs: &'b FieldElement51) -> FieldElement51 {
    #[inline(always)]
    fn m(x: u64, y: u64) -> u128 { (x as u128) * (y as u128) }
    let a: &[u64; 5] = &self.0;
    let b: &[u64; 5] = &rhs.0;

    //let b1_19 = b[1].wrapping_mul(19);
    //let b2_19 = b[2].wrapping_mul(19);
    //let b3_19 = b[3].wrapping_mul(19);
    //let b4_19 = b[4].wrapping_mul(19);

    let b1_19 = b[1] * 19; // Changed from wrapping_mul
    let b2_19 = b[2] * 19; // Changed from wrapping_mul
    let b3_19 = b[3] * 19; // Changed from wrapping_mul
    let b4_19 = b[4] * 19; // Changed from wrapping_mul

    let mut c0: u128 = m(a[0], b[0]);
    c0 += m(a[4], b1_19); c0 += m(a[3], b2_19); c0 += m(a[2], b3_19); c0 += m(a[1], b4_19);

    let mut c1: u128 = m(a[1], b[0]);
    c1 += m(a[0], b[1]); c1 += m(a[4], b2_19); c1 += m(a[3], b3_19); c1 += m(a[2], b4_19);

    let mut c2: u128 = m(a[2], b[0]);
    c2 += m(a[1], b[1]); c2 += m(a[0], b[2]); c2 += m(a[4], b3_19); c2 += m(a[3], b4_19);

    let mut c3: u128 = m(a[3], b[0]);
    c3 += m(a[2], b[1]); c3 += m(a[1], b[2]); c3 += m(a[0], b[3]); c3 += m(a[4], b4_19);

    let mut c4: u128 = m(a[4], b[0]);
    c4 += m(a[3], b[1]); c4 += m(a[2], b[2]); c4 += m(a[1], b[3]); c4 += m(a[0], b[4]);

    const LOW_51_BIT_MASK: u64 = (1u64 << 51) - 1;
    let mut out = [0u64; 5];

    c1 += (c0 >> 51) as u64 as u128; out[0] = (c0 as u64) & LOW_51_BIT_MASK;
    c2 += (c1 >> 51) as u64 as u128; out[1] = (c1 as u64) & LOW_51_BIT_MASK;
    c3 += (c2 >> 51) as u64 as u128; out[2] = (c2 as u64) & LOW_51_BIT_MASK;
    c4 += (c3 >> 51) as u64 as u128; out[3] = (c3 as u64) & LOW_51_BIT_MASK;

    let carry: u64 = (c4 >> 51) as u64;
    out[4] = (c4 as u64) & LOW_51_BIT_MASK;
    //out[0] += carry.wrapping_mul(19);
    out[0] += carry * 19; // Changed from wrapping_mul
    out[1] += out[0] >> 51; out[0] &= LOW_51_BIT_MASK;

    FieldElement51(out)
  }
}

impl<'a> Neg for &'a FieldElement51 {
  type Output = FieldElement51;
  fn neg(self) -> FieldElement51 {
    let mut output = FieldElement51::ZERO; // Need FieldElement51::ZERO for this
    output -= self; // Use Sub which handles reduction
    output
  }
}

// --- Core FieldElement51 Methods (from backend/serial/u64/field.rs) ---
impl FieldElement51 {
  /// Internal constructor for const contexts.
  pub(crate) const fn from_limbs_const(limbs: [u64; 5]) -> Self {
    FieldElement51(limbs)
  }

  pub fn from_limbs(limbs: [u64; 5]) -> Self {
    FieldElement51(limbs)
  }

  pub const ZERO: FieldElement51 = FieldElement51::from_limbs_const([0, 0, 0, 0, 0]);
  pub const ONE: FieldElement51 = FieldElement51::from_limbs_const([1, 0, 0, 0, 0]);
  pub const MINUS_ONE: FieldElement51 = FieldElement51::from_limbs_const([
    2251799813685228, 2251799813685247, 2251799813685247, 2251799813685247, 2251799813685247,
  ]);





  // Method from the original high-level field.rs, uses pow22501
  #[rustfmt::skip]
  pub fn pow_p58(&self) -> FieldElement51 {
    // The bits of (p-5)/8 are 101111.....11.
    // nonzero bits of exponent
    let (t19, _) = self.pow22501();    // t19 is self^(2^250-1)
    let t20      = t19.pow2k(2);       // t19^4
    let t21      = self * &t20;        // self * t19^4
    t21
  }

  // Method from the original high-level field.rs
  // Needs SQRT_M1
  // Needs Neg for FieldElement51 and ConditionallyNegatable for FieldElement51
  pub fn sqrt_ratio_i(u: &FieldElement51, v: &FieldElement51) -> (Choice, FieldElement51) {
    let v3 = &v.square() * v;
    let v7 = &v3.square() * v;
    let mut r = &(u * &v3) * &(u * &v7).pow_p58();
    let check = v * &r.square();

    let i = &SQRT_M1; // Adjusted path to your constants

    let correct_sign_sqrt = check.ct_eq(u);
    let neg_u = -u; // Requires Neg for FieldElement51 or &FieldElement51
    let flipped_sign_sqrt = check.ct_eq(&neg_u);

    let neg_u_times_i = &neg_u * i;
    let flipped_sign_sqrt_i = check.ct_eq(&neg_u_times_i);

    let r_prime = i * &r; // Changed order to match common operator use
    r.conditional_assign(&r_prime, flipped_sign_sqrt | flipped_sign_sqrt_i);

    let r_is_negative = r.is_negative();
    r.conditional_negate(r_is_negative); // Requires ConditionallyNegatable

    let was_nonzero_square = correct_sign_sqrt | flipped_sign_sqrt;

    // Handle u == 0 case specifically to match test expectations for (1,0) output
    let u_is_zero = u.is_zero();
    let r_if_u_is_zero = FieldElement51::ZERO; // if u is zero, sqrt is zero
    r.conditional_assign(&r_if_u_is_zero, u_is_zero);

    // If u is zero, was_nonzero_square should be 1 (as u/v is 0, which is square).
    // If v is zero and u is non-zero, was_nonzero_square should be 0.
    // The original logic for was_nonzero_square already covers most cases.
    // The test cases will verify precise behavior for u=0 or v=0.
    // The test "1/0 should return (0,0)" means if v is zero and u is non-zero,
    // `was_nonzero_square` should be Choice(0).
    // If v is zero, v3 and v7 are zero. (u*v7).pow_p58() could involve 0.invert() which is 0.
    // So r becomes 0. check becomes 0.
    // If u=ONE, v=ZERO: check=0. u=ONE. correct_sign_sqrt=(0==1)=0. flipped_sign_sqrt=(0==-1)=0.
    // was_nonzero_square = 0. r=0. -> returns (0,0). This matches test.

    (was_nonzero_square, r)
  }

  // Method from the original high-level field.rs
  pub fn invsqrt(&self) -> (Choice, FieldElement51) {
    FieldElement51::sqrt_ratio_i(&FieldElement51::ONE, self)
  }

  // square2 from original backend u64/field.rs
  pub fn square2(&self) -> FieldElement51 {
    let mut square = self.pow2k(1); // self.square()
    for i in 0..5 {
      square.0[i] *= 2; // Limb-wise multiplication, no reduction here
    }
    square
  }







  /// Internal reduction helper used by subtraction and negation.
  #[inline(always)]
  fn reduce_after_sub(mut limbs: [u64; 5]) -> FieldElement51 {
    const LOW_51_BIT_MASK: u64 = (1u64 << 51) - 1;
    let c0 = limbs[0] >> 51; let c1 = limbs[1] >> 51;
    let c2 = limbs[2] >> 51; let c3 = limbs[3] >> 51;
    let c4 = limbs[4] >> 51;
    limbs[0] &= LOW_51_BIT_MASK; limbs[1] &= LOW_51_BIT_MASK;
    limbs[2] &= LOW_51_BIT_MASK; limbs[3] &= LOW_51_BIT_MASK;
    limbs[4] &= LOW_51_BIT_MASK;
    //limbs[0] += c4.wrapping_mul(19);
    limbs[0] += c4 * 19; // Changed from wrapping_mul
    limbs[1] += c0;
    limbs[2] += c1; limbs[3] += c2;
    limbs[4] += c3;
    FieldElement51(limbs)
  }

  #[rustfmt::skip]
  pub fn from_bytes(bytes: &[u8; 32]) -> FieldElement51 {
    let load8 = |input: &[u8]| -> u64 {
      (input[0] as u64)
      | ((input[1] as u64) << 8)  | ((input[2] as u64) << 16) | ((input[3] as u64) << 24)
      | ((input[4] as u64) << 32) | ((input[5] as u64) << 40) | ((input[6] as u64) << 48)
      | ((input[7] as u64) << 56)
    };
    const LOW_51_BIT_MASK: u64 = (1u64 << 51) - 1;
    FieldElement51(
      [  load8(&bytes[ 0..])             & LOW_51_BIT_MASK
      , (load8(&bytes[ 6..]) >>  3) & LOW_51_BIT_MASK
      , (load8(&bytes[12..]) >>  6) & LOW_51_BIT_MASK
      , (load8(&bytes[19..]) >>  1) & LOW_51_BIT_MASK
      , (load8(&bytes[24..]) >> 12) & LOW_51_BIT_MASK
      ])
  }

  /// This is the weak reduction from the original backend.
  /// It's used by other methods and should be available.
  /// It ensures limbs are roughly within 51-bit bounds.
  #[inline(always)]
  // Making this pub(crate) or pub(super) so as_bytes can call it.
  // If it's only used by as_bytes within this file, it could be private.
  // For clarity here, assuming it might be useful within the field module.
  pub(crate) fn reduce(mut limbs: [u64; 5]) -> FieldElement51 { // Renamed from reduce_after_sub for clarity as general reduce
    const LOW_51_BIT_MASK: u64 = (1u64 << 51) - 1;

    let c0 = limbs[0] >> 51;
    let c1 = limbs[1] >> 51;
    let c2 = limbs[2] >> 51;
    let c3 = limbs[3] >> 51;
    let c4 = limbs[4] >> 51;

    limbs[0] &= LOW_51_BIT_MASK;
    limbs[1] &= LOW_51_BIT_MASK;
    limbs[2] &= LOW_51_BIT_MASK;
    limbs[3] &= LOW_51_BIT_MASK;
    limbs[4] &= LOW_51_BIT_MASK;

    //limbs[0] += c4.wrapping_mul(19); // Using wrapping_mul for safety, though original implies no overflow
    limbs[0] += c4 * 19; // Changed from wrapping_mul
    limbs[1] += c0;
    limbs[2] += c1;
    limbs[3] += c2;
    limbs[4] += c3;

    // A second pass of carry propagation for the limbs[0] += c4 * 19 part,
    // if limbs[0] overflows. This is typical in such reductions.
    // The original `reduce` function in `backend/serial/u64/field.rs` might be more subtle or rely on
    // limb bounds from `mul`/`square`. Let's ensure this matches the original `reduce` closely.
    // The original `reduce` in `backend/serial/u64/field.rs` *only* does the parallel carry and add back.
    // It does *not* do a second full carry chain. The limbs are then "mostly" reduced.
    // The critical part is that `as_bytes` *calls this `reduce` first*.

    FieldElement51(limbs)
  }


  #[rustfmt::skip]
  pub fn as_bytes(&self) -> [u8; 32] {
    // ***** THIS IS THE CRITICAL CORRECTION: Call reduce first *****
    let reduced_self = FieldElement51::reduce(self.0);
    let mut limbs = reduced_self.0;
    // ***************************************************************

    // Now proceed with canonical reduction logic on 'limbs'
    // Note: Using standard arithmetic ops; wrapping_add/mul only if strictly necessary
    // and matching original intent if it was for specific overflow cases.
    // The original constants.rs and field.rs did not use wrapping_add/mul in these specific spots.
    let mut q = (limbs[0] + 19) >> 51;
    q = (limbs[1] + q) >> 51;
    q = (limbs[2] + q) >> 51;
    q = (limbs[3] + q) >> 51;
    q = (limbs[4] + q) >> 51;

    limbs[0] += 19 * q; // if q is 0 or 1, 19*q is small.

    const LOW_51_BIT_MASK: u64 = (1u64 << 51) - 1;
    limbs[1] += limbs[0] >> 51; limbs[0] &= LOW_51_BIT_MASK;
    limbs[2] += limbs[1] >> 51; limbs[1] &= LOW_51_BIT_MASK;
    limbs[3] += limbs[2] >> 51; limbs[2] &= LOW_51_BIT_MASK;
    limbs[4] += limbs[3] >> 51; limbs[3] &= LOW_51_BIT_MASK;
    limbs[4] &= LOW_51_BIT_MASK;

    let mut s = [0u8;32];
    s[ 0] =  (limbs[0]      ) as u8; s[ 1] =  (limbs[0] >>  8) as u8;
    s[ 2] =  (limbs[0] >> 16) as u8; s[ 3] =  (limbs[0] >> 24) as u8;
    s[ 4] =  (limbs[0] >> 32) as u8; s[ 5] =  (limbs[0] >> 40) as u8;
    s[ 6] = ((limbs[0] >> 48) | (limbs[1] << 3)) as u8;
    s[ 7] =  (limbs[1] >>  5) as u8; s[ 8] =  (limbs[1] >> 13) as u8;
    s[ 9] =  (limbs[1] >> 21) as u8; s[10] =  (limbs[1] >> 29) as u8;
    s[11] =  (limbs[1] >> 37) as u8;
    s[12] = ((limbs[1] >> 45) | (limbs[2] << 6)) as u8;
    s[13] =  (limbs[2] >>  2) as u8; s[14] =  (limbs[2] >> 10) as u8;
    s[15] =  (limbs[2] >> 18) as u8; s[16] =  (limbs[2] >> 26) as u8;
    s[17] =  (limbs[2] >> 34) as u8; s[18] =  (limbs[2] >> 42) as u8;
    s[19] = ((limbs[2] >> 50) | (limbs[3] << 1)) as u8;
    s[20] =  (limbs[3] >>  7) as u8; s[21] =  (limbs[3] >> 15) as u8;
    s[22] =  (limbs[3] >> 23) as u8; s[23] =  (limbs[3] >> 31) as u8;
    s[24] =  (limbs[3] >> 39) as u8;
    s[25] = ((limbs[3] >> 47) | (limbs[4] << 4)) as u8;
    s[26] =  (limbs[4] >>  4) as u8; s[27] =  (limbs[4] >> 12) as u8;
    s[28] =  (limbs[4] >> 20) as u8; s[29] =  (limbs[4] >> 28) as u8;
    s[30] =  (limbs[4] >> 36) as u8; s[31] =  (limbs[4] >> 44) as u8;
    s
  }

  #[rustfmt::skip]
  pub fn pow2k(&self, mut k: u32) -> FieldElement51 {
    debug_assert!(k > 0);
    #[inline(always)]
    fn m(x: u64, y: u64) -> u128 { (x as u128) * (y as u128) }
    let mut a: [u64; 5] = self.0;

    loop {
      //let a3_19 = a[3].wrapping_mul(19);
      //let a4_19 = a[4].wrapping_mul(19);
      let a3_19 = 19 * a[3]; // Changed from wrapping_mul
      let a4_19 = 19 * a[4]; // Changed from wrapping_mul
      /*
       *            let mut c0: u128 = m(a[0], a[0]);
       *            c0 += m(a[1], a4_19).wrapping_add(m(a[1], a4_19)); // * 2
       *            c0 += m(a[2], a3_19).wrapping_add(m(a[2], a3_19)); // * 2
       *
       *            let mut c1: u128 = m(a[3], a3_19);
       *            c1 += m(a[0], a[1]).wrapping_add(m(a[0], a[1]));   // * 2
       *            c1 += m(a[2], a4_19).wrapping_add(m(a[2], a4_19)); // * 2
       *
       *            let mut c2: u128 = m(a[1], a[1]);
       *            c2 += m(a[0], a[2]).wrapping_add(m(a[0], a[2]));   // * 2
       *            c2 += m(a[4], a3_19).wrapping_add(m(a[4], a3_19)); // * 2
       *
       *            let mut c3: u128 = m(a[4], a4_19);
       *            c3 += m(a[0], a[3]).wrapping_add(m(a[0], a[3]));   // * 2
       *            c3 += m(a[1], a[2]).wrapping_add(m(a[1], a[2]));   // * 2
       *
       *            let mut c4: u128 = m(a[2], a[2]);
       *            c4 += m(a[0], a[4]).wrapping_add(m(a[0], a[4]));   // * 2
       *            c4 += m(a[1], a[3]).wrapping_add(m(a[1], a[3]));   // * 2
       */

      // Changed c0, c1, c2, c3, c4 calculations to match original structure
      let     c0: u128 = m(a[0],  a[0]) + 2*( m(a[1], a4_19) + m(a[2], a3_19) );
      let mut c1: u128 = m(a[3], a3_19) + 2*( m(a[0],  a[1]) + m(a[2], a4_19) );
      let mut c2: u128 = m(a[1],  a[1]) + 2*( m(a[0],  a[2]) + m(a[4], a3_19) );
      let mut c3: u128 = m(a[4], a4_19) + 2*( m(a[0],  a[3]) + m(a[1],  a[2]) );
      let mut c4: u128 = m(a[2],  a[2]) + 2*( m(a[0],  a[4]) + m(a[1],  a[3]) );



      const LOW_51_BIT_MASK: u64 = (1u64 << 51) - 1;
      c1 += (c0 >> 51) as u64 as u128; a[0] = (c0 as u64) & LOW_51_BIT_MASK;
      c2 += (c1 >> 51) as u64 as u128; a[1] = (c1 as u64) & LOW_51_BIT_MASK;
      c3 += (c2 >> 51) as u64 as u128; a[2] = (c2 as u64) & LOW_51_BIT_MASK;
      c4 += (c3 >> 51) as u64 as u128; a[3] = (c3 as u64) & LOW_51_BIT_MASK;
      let carry: u64 = (c4 >> 51) as u64;
      a[4] = (c4 as u64) & LOW_51_BIT_MASK;
      //a[0] += carry.wrapping_mul(19);
      a[0] += carry * 19; // Changed from wrapping_mul
      a[1] += a[0] >> 51; a[0] &= LOW_51_BIT_MASK;
      k -= 1; if k == 0 { break; }
    }
    FieldElement51(a)
  }

  pub fn square(&self) -> FieldElement51 {
    self.pow2k(1)
  }

  // square2 removed as it wasn't directly used by higher-level functions we need for X25519.
  // It can be added back if necessary: self.square() + self.square() or similar.

  // --- High-level methods (from original field.rs) ---
  pub fn is_negative(&self) -> Choice {
    (self.as_bytes()[0] & 1).into()
  }

  pub fn is_zero(&self) -> Choice {
    let zero_bytes = [0u8; 32];
    self.as_bytes().ct_eq(&zero_bytes)
  }

  #[rustfmt::skip]
  fn pow22501(&self) -> (FieldElement51, FieldElement51) {
    let t0  = self.square();
    let t1  = t0.square().square();
    let t2  = self * &t1;
    let t3  = &t0 * &t2;
    let t4  = t3.square();
    let t5  = &t2 * &t4;
    let t6  = t5.pow2k(5);
    let t7  = &t6 * &t5;
    let t8  = t7.pow2k(10);
    let t9  = &t8 * &t7;
    let t10 = t9.pow2k(20);
    let t11 = &t10 * &t9;
    let t12 = t11.pow2k(10);
    let t13 = &t12 * &t7;
    let t14 = t13.pow2k(50);
    let t15 = &t14 * &t13;
    let t16 = t15.pow2k(100);
    let t17 = &t16 * &t15;
    let t18 = t17.pow2k(50);
    let t19 = &t18 * &t13;
    (t19, t3)
  }

  #[rustfmt::skip]
  pub fn invert(&self) -> FieldElement51 {
    let (t19, t3) = self.pow22501();
    let t20 = t19.pow2k(5);
    let t21 = &t20 * &t3;
    t21
  }

  // pow_p58, sqrt_ratio_i, invsqrt are omitted as they are not strictly needed for X25519 DH.
  // They can be added back if full FieldElement functionality is desired,
  // ensuring they correctly use FieldElement51 and its methods.
  // For example, sqrt_ratio_i uses SQRT_M1.
}

// Helper for making `Add, Sub, Mul` operator traits work by reference and by value
// This is a simplified version. The original uses macros.
// Add
impl Add<FieldElement51> for FieldElement51 { type Output = Self; fn add(self, rhs: Self) -> Self { &self + &rhs } }
impl Add<&FieldElement51> for FieldElement51 { type Output = Self; fn add(self, rhs: &Self) -> Self { &self + rhs } }
impl Add<FieldElement51> for &FieldElement51 { type Output = FieldElement51; fn add(self, rhs: FieldElement51) -> FieldElement51 { self + &rhs } }
// Sub
impl Sub<FieldElement51> for FieldElement51 { type Output = Self; fn sub(self, rhs: Self) -> Self { &self - &rhs } }
impl Sub<&FieldElement51> for FieldElement51 { type Output = Self; fn sub(self, rhs: &Self) -> Self { &self - rhs } }
impl Sub<FieldElement51> for &FieldElement51 { type Output = FieldElement51; fn sub(self, rhs: FieldElement51) -> FieldElement51 { self - &rhs } }
// Mul
impl Mul<FieldElement51> for FieldElement51 { type Output = Self; fn mul(self, rhs: Self) -> Self { &self * &rhs } }
impl Mul<&FieldElement51> for FieldElement51 { type Output = Self; fn mul(self, rhs: &Self) -> Self { &self * rhs } }
impl Mul<FieldElement51> for &FieldElement51 { type Output = FieldElement51; fn mul(self, rhs: FieldElement51) -> FieldElement51 { self * &rhs } }
// Neg
impl Neg for FieldElement51 { type Output = Self; fn neg(self) -> Self { -&self } }









/// A scalar, represented as a 32-byte array.
/// For X25519, this will hold the clamped private key.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Scalar {
  pub bytes: [u8; 32],
}

impl Scalar {
  pub fn new(bytes: [u8; 32]) -> Self {
    Scalar { bytes }
  }

  /// Get the bits of the scalar, in little-endian order.
  /// The iterator produces 256 bits, $b_0, b_1, \ldots, b_{255}$.
  pub(crate) fn bits_le(&self) -> impl DoubleEndedIterator<Item = bool> + Clone + '_ {
    (0..256).map(move |i| (((self.bytes[i >> 3] >> (i & 7)) & 1u8) == 1u8))
  }

  /// Convert this `Scalar` to its underlying sequence of bytes.
  pub const fn to_bytes(&self) -> [u8; 32] {
    self.bytes
  }
}

/// Clamps a 32-byte scalar for X25519 by manipulating the bits as per RFC 7748.
/// bytes[0]  &= 248;  (0b11111000)
/// bytes[31] &= 127;  (0b01111111)
/// bytes[31] |= 64;   (0b01000000)
#[must_use]
pub const fn clamp_integer(mut bytes: [u8; 32]) -> [u8; 32] {
  bytes[0] &= 0b1111_1000;
  bytes[31] &= 0b0111_1111;
  bytes[31] |= 0b0100_0000;
  bytes
}







#[derive(Copy, Clone, Debug, Default)]
pub struct MontgomeryPoint(pub [u8; 32]);

impl ConstantTimeEq for MontgomeryPoint {
  fn ct_eq(&self, other: &MontgomeryPoint) -> Choice {
    // The FieldElement::from_bytes and as_bytes should be from our FieldElement51
    let self_fe = FieldElement51::from_bytes(&self.0);
    let other_fe = FieldElement51::from_bytes(&other.0);
    self_fe.ct_eq(&other_fe)
  }
}

impl ConditionallySelectable for MontgomeryPoint {
  fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
    let mut new_bytes = [0u8; 32];
    for i in 0..32 {
      new_bytes[i] = u8::conditional_select(&a.0[i], &b.0[i], choice);
    }
    Self(new_bytes)
  }
}

impl PartialEq for MontgomeryPoint {
  fn eq(&self, other: &MontgomeryPoint) -> bool {
    self.ct_eq(other).into()
  }
}
impl Eq for MontgomeryPoint {}

// Removed Hash impl for now to avoid issues if FieldElement::from_bytes().as_bytes() isn't perfect yet.
// Can be added back:
// impl Hash for MontgomeryPoint {
//     fn hash<H: Hasher>(&self, state: &mut H) {
//         FieldElement51::from_bytes(&self.0).as_bytes().hash(state);
//     }
// }

// Replaced Identity trait with an inherent method
impl MontgomeryPoint {
  pub fn identity() -> MontgomeryPoint {
    MontgomeryPoint([0u8; 32])
  }

  // Removed: mul_base, mul_clamped (our public_api.rs handles clamping before calling core logic),
  // mul_base_clamped, to_edwards.

  /// Core Montgomery ladder for X25519.
  /// `bits` should be an iterator yielding the 255 bits of the clamped scalar,
  /// from most significant (bit 254) down to least significant (bit 0).
  pub fn mul_bits_be(&self, bits: impl Iterator<Item = bool>) -> MontgomeryPoint {
    // Algorithm 8 of Costello-Smith 2017
    let affine_u = FieldElement51::from_bytes(&self.0);
    let mut x0 = ProjectivePoint::identity();
    let mut x1 = ProjectivePoint {
      U: affine_u,
      W: FieldElement51::ONE,
    };

    // Go through the bits from most to least significant, using a sliding window of 2
    let mut prev_bit = false;
    for cur_bit in bits {
      let choice: u8 = (prev_bit ^ cur_bit) as u8;

      debug_assert!(choice == 0 || choice == 1);

      ProjectivePoint::conditional_swap(&mut x0, &mut x1, choice.into());
      differential_add_and_double(&mut x0, &mut x1, &affine_u);

      prev_bit = cur_bit;
    }
    // The final value of prev_bit above is scalar.bits()[0], i.e., the LSB of scalar
    ProjectivePoint::conditional_swap(&mut x0, &mut x1, Choice::from(prev_bit as u8));

    x0.as_affine()
  }

  pub const fn as_bytes(&self) -> &[u8; 32] {
    &self.0
  }

  pub const fn to_bytes(&self) -> [u8; 32] {
    self.0
  }
}

// Removed: elligator_encode function

#[allow(non_snake_case)]
#[derive(Copy, Clone, Debug)]
struct ProjectivePoint {
  pub U: FieldElement51, // Changed from FieldElement
  pub W: FieldElement51, // Changed from FieldElement
}

// Replaced Identity trait with an inherent method
impl ProjectivePoint {
  pub fn identity() -> ProjectivePoint {
    ProjectivePoint {
      U: FieldElement51::ONE,
      W: FieldElement51::ZERO,
    }
  }

  pub fn as_affine(&self) -> MontgomeryPoint {
    let u_inv_w = self.W.invert(); // Relies on FieldElement51::invert()
    let u = &self.U * &u_inv_w;    // Relies on FieldElement51 mul
    //let reduced_u = FieldElement51::reduce(u.0);  // doesn't seem to make any difference
    //MontgomeryPoint(reduced_u.as_bytes())
    MontgomeryPoint(u.as_bytes())  // Relies on FieldElement51::as_bytes()
  }
}

impl ConditionallySelectable for ProjectivePoint {
  fn conditional_select(
    a: &ProjectivePoint,
    b: &ProjectivePoint,
    choice: Choice,
  ) -> ProjectivePoint {
    ProjectivePoint {
      U: FieldElement51::conditional_select(&a.U, &b.U, choice),
      W: FieldElement51::conditional_select(&a.W, &b.W, choice),
    }
  }
}

#[allow(non_snake_case)]
#[rustfmt::skip]
fn differential_add_and_double(
  P: &mut ProjectivePoint,
  Q: &mut ProjectivePoint,
  affine_PmQ: &FieldElement51,
) {
  let t0 = &P.U + &P.W;
  let t1 = &P.U - &P.W;
  let t2 = &Q.U + &Q.W;
  let t3 = &Q.U - &Q.W;

  let t4 = t0.square();   // (U_P + W_P)^2 = U_P^2 + 2 U_P W_P + W_P^2
  let t5 = t1.square();   // (U_P - W_P)^2 = U_P^2 - 2 U_P W_P + W_P^2

  let t6 = &t4 - &t5;     // 4 U_P W_P

  let t7 = &t0 * &t3;     // (U_P + W_P) (U_Q - W_Q) = U_P U_Q + W_P U_Q - U_P W_Q - W_P W_Q
  let t8 = &t1 * &t2;     // (U_P - W_P) (U_Q + W_Q) = U_P U_Q - W_P U_Q + U_P W_Q - W_P W_Q

  let t9  = &t7 + &t8;    // 2 (U_P U_Q - W_P W_Q)
  let t10 = &t7 - &t8;    // 2 (W_P U_Q - U_P W_Q)

  let t11 =  t9.square(); // 4 (U_P U_Q - W_P W_Q)^2
  let t12 = t10.square(); // 4 (W_P U_Q - U_P W_Q)^2

  let t13 = &APLUS2_OVER_FOUR * &t6; // (A + 2) U_P U_Q

  let t14 = &t4 * &t5;    // ((U_P + W_P)(U_P - W_P))^2 = (U_P^2 - W_P^2)^2
  let t15 = &t13 + &t5;   // (U_P - W_P)^2 + (A + 2) U_P W_P

  let t16 = &t6 * &t15;   // 4 (U_P W_P) ((U_P - W_P)^2 + (A + 2) U_P W_P)

  let t17 = affine_PmQ * &t12; // U_D * 4 (W_P U_Q - U_P W_Q)^2
  let t18 = t11;               // W_D * 4 (U_P U_Q - W_P W_Q)^2

  P.U = t14;  // U_{P'} = (U_P + W_P)^2 (U_P - W_P)^2
  P.W = t16;  // W_{P'} = (4 U_P W_P) ((U_P - W_P)^2 + ((A + 2)/4) 4 U_P W_P)
  Q.U = t18;  // U_{Q'} = W_D * 4 (U_P U_Q - W_P W_Q)^2
  Q.W = t17;  // W_{Q'} = U_D * 4 (W_P U_Q - U_P W_Q)^2
}

// --- Multiplication Implementations ---
// This is the primary one used by our x25519 scalarmult logic
impl<'a, 'b> Mul<&'b Scalar> for &'a MontgomeryPoint {
  type Output = MontgomeryPoint;
  fn mul(self, scalar: &'b Scalar) -> MontgomeryPoint {
    self.mul_bits_be(scalar.bits_le().rev().skip(1))
  }
}

// Optional: Convenience implementations (forwarding)
impl Mul<Scalar> for MontgomeryPoint {
  type Output = MontgomeryPoint;
  fn mul(self, scalar: Scalar) -> MontgomeryPoint {
    &self * &scalar
  }
}
impl Mul<&Scalar> for MontgomeryPoint {
  type Output = MontgomeryPoint;
  fn mul(self, scalar: &Scalar) -> MontgomeryPoint {
    &self * scalar
  }
}
impl Mul<MontgomeryPoint> for Scalar {
  type Output = MontgomeryPoint;
  fn mul(self, point: MontgomeryPoint) -> MontgomeryPoint {
    &point * &self
  }
}
impl Mul<&MontgomeryPoint> for Scalar {
  type Output = MontgomeryPoint;
  fn mul(self, point: &MontgomeryPoint) -> MontgomeryPoint {
    point * &self
  }
}

impl MulAssign<&Scalar> for MontgomeryPoint {
  fn mul_assign(&mut self, scalar: &Scalar) {
    *self = (self as &MontgomeryPoint) * scalar;
  }
}
impl MulAssign<Scalar> for MontgomeryPoint {
  fn mul_assign(&mut self, scalar: Scalar) {
    *self = (self as &MontgomeryPoint) * &scalar;
  }
}





use core::cmp;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};
use core::option::Option;

/// The `Choice` struct represents a choice for use in conditional assignment.
///
/// It is a wrapper around a `u8`, which should have the value either `1` (true)
/// or `0` (false).
///
/// The conversion from `u8` to `Choice` passes the value through an optimization
/// barrier, as a best-effort attempt to prevent the compiler from inferring that
/// the `Choice` value is a boolean. This strategy is based on Tim Maclean's
/// [work on `rust-timing-shield`][rust-timing-shield], which attempts to provide
/// a more comprehensive approach for preventing software side-channels in Rust
/// code.
///
/// The `Choice` struct implements operators for AND, OR, XOR, and NOT, to allow
/// combining `Choice` values. These operations do not short-circuit.
///
/// [rust-timing-shield]:
/// https://www.chosenplaintext.ca/open-source/rust-timing-shield/security
#[derive(Copy, Clone, Debug)]
pub struct Choice(u8);

impl Choice {
  /// Unwrap the `Choice` wrapper to reveal the underlying `u8`.
  ///
  /// # Note
  ///
  /// This function only exists as an **escape hatch** for the rare case
  /// where it's not possible to use one of the `subtle`-provided
  /// trait impls.
  ///
  /// **To convert a `Choice` to a `bool`, use the `From` implementation instead.**
  #[inline]
  pub fn unwrap_u8(&self) -> u8 {
    self.0
  }
}

impl From<Choice> for bool {
  /// Convert the `Choice` wrapper into a `bool`, depending on whether
  /// the underlying `u8` was a `0` or a `1`.
  ///
  /// # Note
  ///
  /// This function exists to avoid having higher-level cryptographic protocol
  /// implementations duplicating this pattern.
  ///
  /// The intended use case for this conversion is at the _end_ of a
  /// higher-level primitive implementation: for example, in checking a keyed
  /// MAC, where the verification should happen in constant-time (and thus use
  /// a `Choice`) but it is safe to return a `bool` at the end of the
  /// verification.
  #[inline]
  fn from(source: Choice) -> bool {
    debug_assert!((source.0 == 0u8) | (source.0 == 1u8));
    source.0 != 0
  }
}

impl BitAnd for Choice {
  type Output = Choice;
  #[inline]
  fn bitand(self, rhs: Choice) -> Choice {
    (self.0 & rhs.0).into()
  }
}

impl BitAndAssign for Choice {
  #[inline]
  fn bitand_assign(&mut self, rhs: Choice) {
    *self = *self & rhs;
  }
}

impl BitOr for Choice {
  type Output = Choice;
  #[inline]
  fn bitor(self, rhs: Choice) -> Choice {
    (self.0 | rhs.0).into()
  }
}

impl BitOrAssign for Choice {
  #[inline]
  fn bitor_assign(&mut self, rhs: Choice) {
    *self = *self | rhs;
  }
}

impl BitXor for Choice {
  type Output = Choice;
  #[inline]
  fn bitxor(self, rhs: Choice) -> Choice {
    (self.0 ^ rhs.0).into()
  }
}

impl BitXorAssign for Choice {
  #[inline]
  fn bitxor_assign(&mut self, rhs: Choice) {
    *self = *self ^ rhs;
  }
}

impl Not for Choice {
  type Output = Choice;
  #[inline]
  fn not(self) -> Choice {
    (1u8 & (!self.0)).into()
  }
}

/// This function is a best-effort attempt to prevent the compiler from knowing
/// anything about the value of the returned `u8`, other than its type.
///
/// Because we want to support stable Rust, we don't have access to inline
/// assembly or test::black_box, so we use the fact that volatile values will
/// never be elided to register values.
///
/// Note: Rust's notion of "volatile" is subject to change over time. While this
/// code may break in a non-destructive way in the future, “constant-time” code
/// is a continually moving target, and this is better than doing nothing.
#[inline(never)]
fn black_box<T: Copy>(input: T) -> T {
  unsafe {
    // Optimization barrier
    //
    // SAFETY:
    //   - &input is not NULL because we own input;
    //   - input is Copy and always live;
    //   - input is always properly aligned.
    core::ptr::read_volatile(&input)
  }
}

impl From<u8> for Choice {
  #[inline]
  fn from(input: u8) -> Choice {
    debug_assert!((input == 0u8) | (input == 1u8));

    // Our goal is to prevent the compiler from inferring that the value held inside the
    // resulting `Choice` struct is really a `bool` instead of a `u8`.
    Choice(black_box(input))
  }
}

/// An `Eq`-like trait that produces a `Choice` instead of a `bool`.
///
/// # Example
///
/// ```
/// use subtle::ConstantTimeEq;
/// let x: u8 = 5;
/// let y: u8 = 13;
///
/// assert_eq!(x.ct_eq(&y).unwrap_u8(), 0);
/// assert_eq!(x.ct_eq(&x).unwrap_u8(), 1);
/// ```
//
// #[inline] is specified on these function prototypes to signify that they
#[allow(unused_attributes)] // should be in the actual implementation
pub trait ConstantTimeEq {
  /// Determine if two items are equal.
  ///
  /// The `ct_eq` function should execute in constant time.
  ///
  /// # Returns
  ///
  /// * `Choice(1u8)` if `self == other`;
  /// * `Choice(0u8)` if `self != other`.
  #[inline]
  #[allow(unused_attributes)]
  fn ct_eq(&self, other: &Self) -> Choice;

  /// Determine if two items are NOT equal.
  ///
  /// The `ct_ne` function should execute in constant time.
  ///
  /// # Returns
  ///
  /// * `Choice(0u8)` if `self == other`;
  /// * `Choice(1u8)` if `self != other`.
  #[inline]
  fn ct_ne(&self, other: &Self) -> Choice {
    !self.ct_eq(other)
  }
}

impl<T: ConstantTimeEq> ConstantTimeEq for [T] {
  /// Check whether two slices of `ConstantTimeEq` types are equal.
  ///
  /// # Note
  ///
  /// This function short-circuits if the lengths of the input slices
  /// are different.  Otherwise, it should execute in time independent
  /// of the slice contents.
  ///
  /// Since arrays coerce to slices, this function works with fixed-size arrays:
  ///
  /// ```
  /// # use subtle::ConstantTimeEq;
  /// #
  /// let a: [u8; 8] = [0,1,2,3,4,5,6,7];
  /// let b: [u8; 8] = [0,1,2,3,0,1,2,3];
  ///
  /// let a_eq_a = a.ct_eq(&a);
  /// let a_eq_b = a.ct_eq(&b);
  ///
  /// assert_eq!(a_eq_a.unwrap_u8(), 1);
  /// assert_eq!(a_eq_b.unwrap_u8(), 0);
  /// ```
  #[inline]
  fn ct_eq(&self, _rhs: &[T]) -> Choice {
    let len = self.len();

    // Short-circuit on the *lengths* of the slices, not their
    // contents.
    if len != _rhs.len() {
      return Choice::from(0);
    }

    // This loop shouldn't be shortcircuitable, since the compiler
    // shouldn't be able to reason about the value of the `u8`
    // unwrapped from the `ct_eq` result.
    let mut x = 1u8;
    for (ai, bi) in self.iter().zip(_rhs.iter()) {
      x &= ai.ct_eq(bi).unwrap_u8();
    }

    x.into()
  }
}

impl ConstantTimeEq for Choice {
  #[inline]
  fn ct_eq(&self, rhs: &Choice) -> Choice {
    !(*self ^ *rhs)
  }
}

/// Given the bit-width `$bit_width` and the corresponding primitive
/// unsigned and signed types `$t_u` and `$t_i` respectively, generate
/// an `ConstantTimeEq` implementation.
macro_rules! generate_integer_equal {
  ($t_u:ty, $t_i:ty, $bit_width:expr) => {
    impl ConstantTimeEq for $t_u {
      #[inline]
      fn ct_eq(&self, other: &$t_u) -> Choice {
        // x == 0 if and only if self == other
        let x: $t_u = self ^ other;

        // If x == 0, then x and -x are both equal to zero;
        // otherwise, one or both will have its high bit set.
        let y: $t_u = (x | x.wrapping_neg()) >> ($bit_width - 1);

        // Result is the opposite of the high bit (now shifted to low).
        ((y ^ (1 as $t_u)) as u8).into()
      }
    }
    impl ConstantTimeEq for $t_i {
      #[inline]
      fn ct_eq(&self, other: &$t_i) -> Choice {
        // Bitcast to unsigned and call that implementation.
        (*self as $t_u).ct_eq(&(*other as $t_u))
      }
    }
  };
}

generate_integer_equal!(u8, i8, 8);
generate_integer_equal!(u16, i16, 16);
generate_integer_equal!(u32, i32, 32);
generate_integer_equal!(u64, i64, 64);
generate_integer_equal!(usize, isize, ::core::mem::size_of::<usize>() * 8);

/// `Ordering` is `#[repr(i8)]` making it possible to leverage `i8::ct_eq`.
impl ConstantTimeEq for cmp::Ordering {
  #[inline]
  fn ct_eq(&self, other: &Self) -> Choice {
    (*self as i8).ct_eq(&(*other as i8))
  }
}

/// A type which can be conditionally selected in constant time.
///
/// This trait also provides generic implementations of conditional
/// assignment and conditional swaps.
//
// #[inline] is specified on these function prototypes to signify that they
#[allow(unused_attributes)] // should be in the actual implementation
pub trait ConditionallySelectable: Copy {
  /// Select `a` or `b` according to `choice`.
  ///
  /// # Returns
  ///
  /// * `a` if `choice == Choice(0)`;
  /// * `b` if `choice == Choice(1)`.
  ///
  /// This function should execute in constant time.
  ///
  /// # Example
  ///
  /// ```
  /// use subtle::ConditionallySelectable;
  /// #
  /// # fn main() {
  /// let x: u8 = 13;
  /// let y: u8 = 42;
  ///
  /// let z = u8::conditional_select(&x, &y, 0.into());
  /// assert_eq!(z, x);
  /// let z = u8::conditional_select(&x, &y, 1.into());
  /// assert_eq!(z, y);
  /// # }
  /// ```
  #[inline]
  #[allow(unused_attributes)]
  fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self;

  /// Conditionally assign `other` to `self`, according to `choice`.
  ///
  /// This function should execute in constant time.
  ///
  /// # Example
  ///
  /// ```
  /// use subtle::ConditionallySelectable;
  /// #
  /// # fn main() {
  /// let mut x: u8 = 13;
  /// let mut y: u8 = 42;
  ///
  /// x.conditional_assign(&y, 0.into());
  /// assert_eq!(x, 13);
  /// x.conditional_assign(&y, 1.into());
  /// assert_eq!(x, 42);
  /// # }
  /// ```
  #[inline]
  fn conditional_assign(&mut self, other: &Self, choice: Choice) {
    *self = Self::conditional_select(self, other, choice);
  }

  /// Conditionally swap `self` and `other` if `choice == 1`; otherwise,
  /// reassign both unto themselves.
  ///
  /// This function should execute in constant time.
  ///
  /// # Example
  ///
  /// ```
  /// use subtle::ConditionallySelectable;
  /// #
  /// # fn main() {
  /// let mut x: u8 = 13;
  /// let mut y: u8 = 42;
  ///
  /// u8::conditional_swap(&mut x, &mut y, 0.into());
  /// assert_eq!(x, 13);
  /// assert_eq!(y, 42);
  /// u8::conditional_swap(&mut x, &mut y, 1.into());
  /// assert_eq!(x, 42);
  /// assert_eq!(y, 13);
  /// # }
  /// ```
  #[inline]
  fn conditional_swap(a: &mut Self, b: &mut Self, choice: Choice) {
    let t: Self = *a;
    a.conditional_assign(&b, choice);
    b.conditional_assign(&t, choice);
  }
}

macro_rules! to_signed_int {
  (u8) => {
    i8
  };
  (u16) => {
    i16
  };
  (u32) => {
    i32
  };
  (u64) => {
    i64
  };
  (u128) => {
    i128
  };
  (i8) => {
    i8
  };
  (i16) => {
    i16
  };
  (i32) => {
    i32
  };
  (i64) => {
    i64
  };
  (i128) => {
    i128
  };
}

macro_rules! generate_integer_conditional_select {
  ($($t:tt)*) => ($(
    impl ConditionallySelectable for $t {
      #[inline]
      fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        // if choice = 0, mask = (-0) = 0000...0000
        // if choice = 1, mask = (-1) = 1111...1111
        let mask = -(choice.unwrap_u8() as to_signed_int!($t)) as $t;
        a ^ (mask & (a ^ b))
      }

      #[inline]
      fn conditional_assign(&mut self, other: &Self, choice: Choice) {
        // if choice = 0, mask = (-0) = 0000...0000
        // if choice = 1, mask = (-1) = 1111...1111
        let mask = -(choice.unwrap_u8() as to_signed_int!($t)) as $t;
        *self ^= mask & (*self ^ *other);
      }

      #[inline]
      fn conditional_swap(a: &mut Self, b: &mut Self, choice: Choice) {
        // if choice = 0, mask = (-0) = 0000...0000
        // if choice = 1, mask = (-1) = 1111...1111
        let mask = -(choice.unwrap_u8() as to_signed_int!($t)) as $t;
        let t = mask & (*a ^ *b);
        *a ^= t;
        *b ^= t;
      }
    }
  )*)
}

generate_integer_conditional_select!(  u8   i8);
generate_integer_conditional_select!( u16  i16);
generate_integer_conditional_select!( u32  i32);
generate_integer_conditional_select!( u64  i64);

/// `Ordering` is `#[repr(i8)]` where:
///
/// - `Less` => -1
/// - `Equal` => 0
/// - `Greater` => 1
///
/// Given this, it's possible to operate on orderings as if they're integers,
/// which allows leveraging conditional masking for predication.
impl ConditionallySelectable for cmp::Ordering {
  fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
    let a = *a as i8;
    let b = *b as i8;
    let ret = i8::conditional_select(&a, &b, choice);

    // SAFETY: `Ordering` is `#[repr(i8)]` and `ret` has been assigned to
    // a value which was originally a valid `Ordering` then cast to `i8`
    unsafe { *((&ret as *const _) as *const cmp::Ordering) }
  }
}

impl ConditionallySelectable for Choice {
  #[inline]
  fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
    Choice(u8::conditional_select(&a.0, &b.0, choice))
  }
}

/// A type which can be conditionally negated in constant time.
///
/// # Note
///
/// A generic implementation of `ConditionallyNegatable` is provided
/// for types `T` which are `ConditionallySelectable` and have `Neg`
/// implemented on `&T`.
//
// #[inline] is specified on these function prototypes to signify that they
#[allow(unused_attributes)] // should be in the actual implementation
pub trait ConditionallyNegatable {
  /// Negate `self` if `choice == Choice(1)`; otherwise, leave it
  /// unchanged.
  ///
  /// This function should execute in constant time.
  #[inline]
  #[allow(unused_attributes)]
  fn conditional_negate(&mut self, choice: Choice);
}

impl<T> ConditionallyNegatable for T
where
T: ConditionallySelectable,
for<'a> &'a T: Neg<Output = T>,
{
  #[inline]
  fn conditional_negate(&mut self, choice: Choice) {
    // Need to cast to eliminate mutability
    let self_neg: T = -(self as &T);
    self.conditional_assign(&self_neg, choice);
  }
}

/// The `CtOption<T>` type represents an optional value similar to the
/// [`Option<T>`](core::option::Option) type but is intended for
/// use in constant time APIs.
///
/// Any given `CtOption<T>` is either `Some` or `None`, but unlike
/// `Option<T>` these variants are not exposed. The
/// [`is_some()`](CtOption::is_some) method is used to determine if
/// the value is `Some`, and [`unwrap_or()`](CtOption::unwrap_or) and
/// [`unwrap_or_else()`](CtOption::unwrap_or_else) methods are
/// provided to access the underlying value. The value can also be
/// obtained with [`unwrap()`](CtOption::unwrap) but this will panic
/// if it is `None`.
///
/// Functions that are intended to be constant time may not produce
/// valid results for all inputs, such as square root and inversion
/// operations in finite field arithmetic. Returning an `Option<T>`
/// from these functions makes it difficult for the caller to reason
/// about the result in constant time, and returning an incorrect
/// value burdens the caller and increases the chance of bugs.
#[derive(Clone, Copy, Debug)]
pub struct CtOption<T> {
  value: T,
  is_some: Choice,
}

impl<T> From<CtOption<T>> for Option<T> {
  /// Convert the `CtOption<T>` wrapper into an `Option<T>`, depending on whether
  /// the underlying `is_some` `Choice` was a `0` or a `1` once unwrapped.
  ///
  /// # Note
  ///
  /// This function exists to avoid ending up with ugly, verbose and/or bad handled
  /// conversions from the `CtOption<T>` wraps to an `Option<T>` or `Result<T, E>`.
  /// This implementation doesn't intend to be constant-time nor try to protect the
  /// leakage of the `T` since the `Option<T>` will do it anyways.
  fn from(source: CtOption<T>) -> Option<T> {
    if source.is_some().unwrap_u8() == 1u8 {
      Option::Some(source.value)
    } else {
      None
    }
  }
}

impl<T> CtOption<T> {
  /// This method is used to construct a new `CtOption<T>` and takes
  /// a value of type `T`, and a `Choice` that determines whether
  /// the optional value should be `Some` or not. If `is_some` is
  /// false, the value will still be stored but its value is never
  /// exposed.
  #[inline]
  pub fn new(value: T, is_some: Choice) -> CtOption<T> {
    CtOption {
      value: value,
      is_some: is_some,
    }
  }

  /// Returns the contained value, consuming the `self` value.
  ///
  /// # Panics
  ///
  /// Panics if the value is none with a custom panic message provided by
  /// `msg`.
  pub fn expect(self, msg: &str) -> T {
    assert_eq!(self.is_some.unwrap_u8(), 1, "{}", msg);

    self.value
  }

  /// This returns the underlying value but panics if it
  /// is not `Some`.
  #[inline]
  pub fn unwrap(self) -> T {
    assert_eq!(self.is_some.unwrap_u8(), 1);

    self.value
  }

  /// This returns the underlying value if it is `Some`
  /// or the provided value otherwise.
  #[inline]
  pub fn unwrap_or(self, def: T) -> T
  where
  T: ConditionallySelectable,
  {
    T::conditional_select(&def, &self.value, self.is_some)
  }

  /// This returns the underlying value if it is `Some`
  /// or the value produced by the provided closure otherwise.
  ///
  /// This operates in constant time, because the provided closure
  /// is always called.
  #[inline]
  pub fn unwrap_or_else<F>(self, f: F) -> T
  where
  T: ConditionallySelectable,
  F: FnOnce() -> T,
  {
    T::conditional_select(&f(), &self.value, self.is_some)
  }

  /// Returns a true `Choice` if this value is `Some`.
  #[inline]
  pub fn is_some(&self) -> Choice {
    self.is_some
  }

  /// Returns a true `Choice` if this value is `None`.
  #[inline]
  pub fn is_none(&self) -> Choice {
    !self.is_some
  }

  /// Returns a `None` value if the option is `None`, otherwise
  /// returns a `CtOption` enclosing the value of the provided closure.
  /// The closure is given the enclosed value or, if the option is
  /// `None`, it is provided a dummy value computed using
  /// `Default::default()`.
  ///
  /// This operates in constant time, because the provided closure
  /// is always called.
  #[inline]
  pub fn map<U, F>(self, f: F) -> CtOption<U>
  where
  T: Default + ConditionallySelectable,
  F: FnOnce(T) -> U,
  {
    CtOption::new(
      f(T::conditional_select(
        &T::default(),
                              &self.value,
                              self.is_some,
      )),
      self.is_some,
    )
  }

  /// Returns a `None` value if the option is `None`, otherwise
  /// returns the result of the provided closure. The closure is
  /// given the enclosed value or, if the option is `None`, it
  /// is provided a dummy value computed using `Default::default()`.
  ///
  /// This operates in constant time, because the provided closure
  /// is always called.
  #[inline]
  pub fn and_then<U, F>(self, f: F) -> CtOption<U>
  where
  T: Default + ConditionallySelectable,
  F: FnOnce(T) -> CtOption<U>,
  {
    let mut tmp = f(T::conditional_select(
      &T::default(),
                                          &self.value,
                                          self.is_some,
    ));
    tmp.is_some &= self.is_some;

    tmp
  }

  /// Returns `self` if it contains a value, and otherwise returns the result of
  /// calling `f`. The provided function `f` is always called.
  #[inline]
  pub fn or_else<F>(self, f: F) -> CtOption<T>
  where
  T: ConditionallySelectable,
  F: FnOnce() -> CtOption<T>,
  {
    let is_none = self.is_none();
    let f = f();

    Self::conditional_select(&self, &f, is_none)
  }

  /// Convert the `CtOption<T>` wrapper into an `Option<T>`, depending on whether
  /// the underlying `is_some` `Choice` was a `0` or a `1` once unwrapped.
  ///
  /// # Note
  ///
  /// This function exists to avoid ending up with ugly, verbose and/or bad handled
  /// conversions from the `CtOption<T>` wraps to an `Option<T>` or `Result<T, E>`.
  /// This implementation doesn't intend to be constant-time nor try to protect the
  /// leakage of the `T` since the `Option<T>` will do it anyways.
  ///
  /// It's equivalent to the corresponding `From` impl, however this version is
  /// friendlier for type inference.
  pub fn into_option(self) -> Option<T> {
    self.into()
  }
}

impl<T: ConditionallySelectable> ConditionallySelectable for CtOption<T> {
  fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
    CtOption::new(
      T::conditional_select(&a.value, &b.value, choice),
                  Choice::conditional_select(&a.is_some, &b.is_some, choice),
    )
  }
}

impl<T: ConstantTimeEq> ConstantTimeEq for CtOption<T> {
  /// Two `CtOption<T>`s are equal if they are both `Some` and
  /// their values are equal, or both `None`.
  #[inline]
  fn ct_eq(&self, rhs: &CtOption<T>) -> Choice {
    let a = self.is_some();
    let b = rhs.is_some();

    (a & b & self.value.ct_eq(&rhs.value)) | (!a & !b)
  }
}

/// A type which can be compared in some manner and be determined to be greater
/// than another of the same type.
pub trait ConstantTimeGreater {
  /// Determine whether `self > other`.
  ///
  /// The bitwise-NOT of the return value of this function should be usable to
  /// determine if `self <= other`.
  ///
  /// This function should execute in constant time.
  ///
  /// # Returns
  ///
  /// A `Choice` with a set bit if `self > other`, and with no set bits
  /// otherwise.
  ///
  /// # Example
  ///
  /// ```
  /// use subtle::ConstantTimeGreater;
  ///
  /// let x: u8 = 13;
  /// let y: u8 = 42;
  ///
  /// let x_gt_y = x.ct_gt(&y);
  ///
  /// assert_eq!(x_gt_y.unwrap_u8(), 0);
  ///
  /// let y_gt_x = y.ct_gt(&x);
  ///
  /// assert_eq!(y_gt_x.unwrap_u8(), 1);
  ///
  /// let x_gt_x = x.ct_gt(&x);
  ///
  /// assert_eq!(x_gt_x.unwrap_u8(), 0);
  /// ```
  fn ct_gt(&self, other: &Self) -> Choice;
}

macro_rules! generate_unsigned_integer_greater {
  ($t_u: ty, $bit_width: expr) => {
    impl ConstantTimeGreater for $t_u {
      /// Returns Choice::from(1) iff x > y, and Choice::from(0) iff x <= y.
      ///
      /// # Note
      ///
      /// This algoritm would also work for signed integers if we first
      /// flip the top bit, e.g. `let x: u8 = x ^ 0x80`, etc.
      #[inline]
      fn ct_gt(&self, other: &$t_u) -> Choice {
        let gtb = self & !other; // All the bits in self that are greater than their corresponding bits in other.
        let mut ltb = !self & other; // All the bits in self that are less than their corresponding bits in other.
        let mut pow = 1;

        // Less-than operator is okay here because it's dependent on the bit-width.
        while pow < $bit_width {
          ltb |= ltb >> pow; // Bit-smear the highest set bit to the right.
          pow += pow;
        }
        let mut bit = gtb & !ltb; // Select the highest set bit.
        let mut pow = 1;

        while pow < $bit_width {
          bit |= bit >> pow; // Shift it to the right until we end up with either 0 or 1.
          pow += pow;
        }
        // XXX We should possibly do the above flattening to 0 or 1 in the
        //     Choice constructor rather than making it a debug error?
        Choice::from((bit & 1) as u8)
      }
    }
  };
}

generate_unsigned_integer_greater!(u8, 8);
generate_unsigned_integer_greater!(u16, 16);
generate_unsigned_integer_greater!(u32, 32);
generate_unsigned_integer_greater!(u64, 64);

impl ConstantTimeGreater for cmp::Ordering {
  #[inline]
  fn ct_gt(&self, other: &Self) -> Choice {
    // No impl of `ConstantTimeGreater` for `i8`, so use `u8`
    let a = (*self as i8) + 1;
    let b = (*other as i8) + 1;
    (a as u8).ct_gt(&(b as u8))
  }
}

/// A type which can be compared in some manner and be determined to be less
/// than another of the same type.
pub trait ConstantTimeLess: ConstantTimeEq + ConstantTimeGreater {
  /// Determine whether `self < other`.
  ///
  /// The bitwise-NOT of the return value of this function should be usable to
  /// determine if `self >= other`.
  ///
  /// A default implementation is provided and implemented for the unsigned
  /// integer types.
  ///
  /// This function should execute in constant time.
  ///
  /// # Returns
  ///
  /// A `Choice` with a set bit if `self < other`, and with no set bits
  /// otherwise.
  ///
  /// # Example
  ///
  /// ```
  /// use subtle::ConstantTimeLess;
  ///
  /// let x: u8 = 13;
  /// let y: u8 = 42;
  ///
  /// let x_lt_y = x.ct_lt(&y);
  ///
  /// assert_eq!(x_lt_y.unwrap_u8(), 1);
  ///
  /// let y_lt_x = y.ct_lt(&x);
  ///
  /// assert_eq!(y_lt_x.unwrap_u8(), 0);
  ///
  /// let x_lt_x = x.ct_lt(&x);
  ///
  /// assert_eq!(x_lt_x.unwrap_u8(), 0);
  /// ```
  #[inline]
  fn ct_lt(&self, other: &Self) -> Choice {
    !self.ct_gt(other) & !self.ct_eq(other)
  }
}

impl ConstantTimeLess for u8 {}
impl ConstantTimeLess for u16 {}
impl ConstantTimeLess for u32 {}
impl ConstantTimeLess for u64 {}

impl ConstantTimeLess for cmp::Ordering {
  #[inline]
  fn ct_lt(&self, other: &Self) -> Choice {
    // No impl of `ConstantTimeLess` for `i8`, so use `u8`
    let a = (*self as i8) + 1;
    let b = (*other as i8) + 1;
    (a as u8).ct_lt(&(b as u8))
  }
}

/// Wrapper type which implements an optimization barrier for all accesses.
#[derive(Clone, Copy, Debug)]
pub struct BlackBox<T: Copy>(T);

impl<T: Copy> BlackBox<T> {
  /// Constructs a new instance of `BlackBox` which will wrap the specified value.
  ///
  /// All access to the inner value will be mediated by a `black_box` optimization barrier.
  pub const fn new(value: T) -> Self {
    Self(value)
  }

  /// Read the inner value, applying an optimization barrier on access.
  pub fn get(self) -> T {
    black_box(self.0)
  }
}
