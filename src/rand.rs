use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt::Write as FmtWrite; // Alias to avoid conflict with io::Write

// Conditional imports for OS-specific RNG
#[cfg(target_family = "unix")]
use std::fs::File;
#[cfg(target_family = "unix")]
use std::io::Read; // Specifically for File::read_exact

// --- Windows-specific RNG using BCryptGenRandom ---
#[cfg(target_os = "windows")]
mod windows_rng {
  use std::ffi::c_void;
  use std::os::raw::{c_uchar, c_ulong};

  type NTSTATUS = std::os::raw::c_long;
  const STATUS_SUCCESS: NTSTATUS = 0;
  const BCRYPT_USE_SYSTEM_PREFERRED_RNG: c_ulong = 0x00000002;

  #[link(name = "bcrypt")]
  extern "system" {
    fn BCryptGenRandom(
      hAlgorithm: *mut c_void,
      pbBuffer: *mut c_uchar,
      cbBuffer: c_ulong,
      dwFlags: c_ulong,
    ) -> NTSTATUS;
  }

  /// Fills the buffer with cryptographically secure random bytes.
  pub fn get_random_bytes(buffer: &mut [u8]) -> Result<(), String> {
    if buffer.len() > c_ulong::MAX as usize {
      return Err(format!(
        "Buffer length {} exceeds maximum {} for BCryptGenRandom",
        buffer.len(),
                         c_ulong::MAX
      ));
    }
    let nt_status = unsafe {
      BCryptGenRandom(
        std::ptr::null_mut(),
                      buffer.as_mut_ptr(),
                      buffer.len() as c_ulong,
                      BCRYPT_USE_SYSTEM_PREFERRED_RNG,
      )
    };
    if nt_status == STATUS_SUCCESS {
      Ok(())
    } else {
      Err(format!(
        "BCryptGenRandom failed with NTSTATUS: {:#010X}",
        nt_status
      ))
    }
  }
}

// --- Helper function to get a secure u64 seed from OS RNG ---
fn try_get_os_random_u64_seed() -> Option<u64> {
  let mut seed_bytes = [0u8; 8]; // 8 bytes for u64

  #[cfg(target_family = "unix")]
  {
    if let Ok(mut f) = File::open("/dev/urandom") {
      if f.read_exact(&mut seed_bytes).is_ok() {
        return Some(u64::from_le_bytes(seed_bytes));
      } else {
        eprintln!("Warning: Failed to read 8 bytes from /dev/urandom for seed.");
      }
    } else {
      eprintln!("Warning: Failed to open /dev/urandom for seed.");
    }
  }

  #[cfg(target_os = "windows")]
  {
    // Ensure this module path is correct if windows_rng is an inner module.
    // It is, so self::windows_rng or just windows_rng works here.
    if self::windows_rng::get_random_bytes(&mut seed_bytes).is_ok() {
      return Some(u64::from_le_bytes(seed_bytes));
    } else {
      eprintln!("Warning: BCryptGenRandom failed to provide 8 bytes for seed.");
    }
  }

  // If neither cfg block was compiled or if they failed:
  #[cfg(not(any(target_family = "unix", target_os = "windows")))]
  eprintln!("Warning: No OS-specific RNG available for seeding on this target platform.");

  None // Fallback if no OS RNG worked or available
}


// --- XorShift64: A simple pseudo-random number generator ---
// This is NOT cryptographically secure. Used as a fallback or for user-seeded instances.
struct XorShift64 {
  state: u64,
}

impl XorShift64 {
  fn new(seed: u64) -> Self {
    let initial_state = if seed == 0 { 0xBAD5EED0BAD5EED0u64 } else { seed };
    XorShift64 { state: initial_state }
  }

  fn next(&mut self) -> u64 {
    let mut x = self.state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    self.state = x;
    x
  }

  #[allow(dead_code)]
  fn next_bytes_8(&mut self) -> [u8; 8] { // Helper for UUID fallback
    self.next().to_le_bytes()
  }

  #[allow(dead_code)] // Kept for potential future use, though not used by fill_bytes
  pub fn rand_range(&mut self, min: i64, max: i64) -> i64 {
    if min > max {
      panic!("min ({}) cannot be greater than max ({}) in XorShift64::rand_range", min, max);
    }
    if min == max { return min; }
    if min == i64::MIN && max == i64::MAX { return self.next() as i64; }
    let num_values_i128 = (max as i128) - (min as i128) + 1;
    debug_assert!(num_values_i128 > 0 && num_values_i128 <= u64::MAX as i128);
    let num_values = num_values_i128 as u64;
    let rand_u64 = self.next();
    let value_in_sub_range = rand_u64 % num_values;
    min + (value_in_sub_range as i64)
  }
}

// --- Global state for the PRNG ---
// WARNING: Global mutable statics are not thread-safe without external synchronization (e.g., Mutex).
static mut GLOBAL_RNG_STATE: u64 = 0; // Will be initialized by Rand::init()

// --- Rand struct providing PRNG functionalities ---
pub struct Rand {
  rng: XorShift64,
}

impl Rand {
  /// Initializes the global PRNG state.
  /// Attempts to use an OS-provided secure seed first, then falls back to a time-based seed.
  pub fn init() {
    let seed = match try_get_os_random_u64_seed() {
      Some(s) => {
        //eprintln!("Info: Global RNG seeded using OS-provided randomness.");
        s
      }
      None => {
        eprintln!("Warning: OS-provided seed failed or unavailable. Falling back to time-based seed for global RNG. This is NOT cryptographically secure.");
        SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0xBAD5EED0BAD5EED0u64, |d| d.as_nanos() as u64) // Provide a default if time fails
      }
    };

    let rand_instance = Rand::new(seed);
    unsafe { GLOBAL_RNG_STATE = rand_instance.rng.state; }
  }

  /// Creates a new `Rand` instance with the given `u64` seed (for XorShift64).
  pub fn new(seed: u64) -> Rand {
    Rand { rng: XorShift64::new(seed) }
  }

  /// Returns the internal `u64` state of the PRNG.
  pub fn get(&self) -> u64 { self.rng.state }

  /// Creates a `Rand` instance from a given `u64` state.
  #[allow(dead_code)]
  pub fn build(state: u64) -> Rand { Rand { rng: XorShift64 { state } } }

  /// Generates a pseudo-random `u32` number.
  pub fn rand(&mut self) -> u32 { self.rng.next() as u32 }

  /// Generates a pseudo-random `i64` number (full range).
  pub fn gen_i64(&mut self) -> i64 { self.rng.next() as i64 }

  /// Shuffles the elements of a slice randomly.
  pub fn shuffle<T>(&mut self, a: &mut [T]) {
    if a.is_empty() { return; }
    let mut i = a.len() - 1;
    while i > 0 {
      let j = (self.rand() as usize) % (i + 1);
      a.swap(i, j);
      i -= 1;
    }
  }

  /// Generates a pseudo-random `i32` in the range [a, b] (inclusive).
  pub fn rand_range(&mut self, a: i32, b: i32) -> i32 {
    self.rng.rand_range(a as i64, b as i64) as i32
  }

  /// Generates a pseudo-random `i64` in the range [min, max] (inclusive).
  pub fn rand_range_i64(&mut self, min: i64, max: i64) -> i64 {
    self.rng.rand_range(min, max)
  }

  /// Generates a pseudo-random `f64` between 0.0 (inclusive) and 1.0 (exclusive).
  pub fn rand_float(&mut self) -> f64 {
    (self.rng.next() as f64) / (u64::MAX as f64 + 1.0) // +1.0 to make it [0,1)
  }
}

// --- Global PRNG functions that use the global state ---
/// Generates a pseudo-random `u32` using the global PRNG instance.
#[allow(dead_code)]
pub fn rand() -> u32 {
  unsafe {
    let mut rand_instance = Rand::build(GLOBAL_RNG_STATE);
    let x = rand_instance.rand();
    GLOBAL_RNG_STATE = rand_instance.get();
    x
  }
}

/// Generates a pseudo-random `i64` using the global PRNG instance.
#[allow(dead_code)]
pub fn rand_i64() -> i64 {
  unsafe {
    let mut rand_instance = Rand::build(GLOBAL_RNG_STATE);
    let x = rand_instance.gen_i64();
    GLOBAL_RNG_STATE = rand_instance.get();
    x
  }
}

/// Shuffles a slice using the global PRNG instance.
#[allow(dead_code)]
pub fn shuffle<T>(a: &mut [T]) {
  unsafe {
    let mut rand_instance = Rand::build(GLOBAL_RNG_STATE);
    rand_instance.shuffle(a);
    GLOBAL_RNG_STATE = rand_instance.get();
  }
}

/// Generates a pseudo-random `i32` in range [a, b] (inclusive) using the global PRNG.
#[allow(dead_code)]
pub fn rand_range(a: i32, b: i32) -> i32 {
  unsafe {
    let mut rand_instance = Rand::build(GLOBAL_RNG_STATE);
    let result = rand_instance.rand_range(a, b);
    GLOBAL_RNG_STATE = rand_instance.get();
    result
  }
}

/// Generates a pseudo-random `i64` in range [min, max] (inclusive) using the global PRNG.
#[allow(dead_code)]
pub fn rand_range_i64(min: i64, max: i64) -> i64 {
  unsafe {
    let mut rand_instance = Rand::build(GLOBAL_RNG_STATE);
    let result = rand_instance.rand_range_i64(min, max);
    GLOBAL_RNG_STATE = rand_instance.get();
    result
  }
}

/// Generates a pseudo-random `f64` [0.0, 1.0) using the global PRNG.
#[allow(dead_code)]
pub fn rand_float() -> f64 {
  unsafe {
    let mut rand_instance = Rand::build(GLOBAL_RNG_STATE);
    let result = rand_instance.rand_float();
    GLOBAL_RNG_STATE = rand_instance.get();
    result
  }
}

// --- Function to fill a byte slice with random data ---
/// Fills the provided buffer with random bytes.
///
/// It attempts to use cryptographically secure OS-specific RNGs first:
/// - On Unix-like systems, it reads from `/dev/urandom`.
/// - On Windows, it uses `BCryptGenRandom`.
///
/// If OS-specific methods fail or are unavailable, it falls back to the
/// `XorShift64` pseudo-random number generator. For this PRNG fallback:
/// 1. It attempts to seed `XorShift64` using `try_get_os_random_u64_seed()`.
/// 2. If OS seed is unavailable, it uses a time-based seed.
///
/// Warnings are printed to `stderr` if fallbacks occur, especially if a
/// time-based seed is used, as this is **not cryptographically secure**.
///
/// # Arguments
///
/// * `buffer`: A mutable byte slice to be filled with random data.
///
/// # Example
///
/// ```
/// // let mut my_array = [0u8; 32];
/// // fill_bytes(&mut my_array);
/// // // my_array is now filled with random bytes.
/// ```
pub fn fill_bytes(buffer: &mut [u8]) {
  if buffer.is_empty() {
    return;
  }

  let mut random_bytes_sourced_securely = false;

  #[cfg(target_family = "unix")]
  {
    if !buffer.is_empty() { // read_exact panics on empty buffer
      if let Ok(mut f) = File::open("/dev/urandom") {
        if f.read_exact(buffer).is_ok() {
          random_bytes_sourced_securely = true;
        } else {
          eprintln!(
            "fill_bytes Warning: Failed to read {} bytes from /dev/urandom. Will use PRNG fallback.",
            buffer.len()
          );
        }
      } else {
        eprintln!("fill_bytes Warning: Failed to open /dev/urandom. Will use PRNG fallback.");
      }
    }
  }

  #[cfg(target_os = "windows")]
  {
    if !random_bytes_sourced_securely && !buffer.is_empty() {
      match self::windows_rng::get_random_bytes(buffer) {
        Ok(()) => {
          random_bytes_sourced_securely = true;
        }
        Err(e) => {
          eprintln!(
            "fill_bytes Warning: Windows BCryptGenRandom failed for {} bytes (Error: {}). Will use PRNG fallback.",
                    buffer.len(), e
          );
        }
      }
    }
  }

  if !random_bytes_sourced_securely {
    eprintln!(
      "fill_bytes Warning: OS-specific RNG failed or unavailable for {} bytes. \
Falling back to XorShift64 PRNG. This fallback is NOT cryptographically secure if itself seeded by time.",
buffer.len()
    );

    let seed = match try_get_os_random_u64_seed() {
      Some(s) => {
        // eprintln!("fill_bytes Info: Fallback PRNG seeded using OS-provided randomness."); // Can be verbose
        s
      }
      None => {
        eprintln!("fill_bytes Warning: OS-provided seed failed for fallback PRNG. Falling back to time-based seed. This is NOT cryptographically secure.");
        SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0xBAD5EED0BAD5EED0u64, |d| d.as_nanos() as u64)
      }
    };

    let mut fallback_rng = XorShift64::new(seed);
    let mut i = 0;
    while i < buffer.len() {
      let rand_u64 = fallback_rng.next();
      let rand_bytes_arr = rand_u64.to_le_bytes(); // u64 gives 8 bytes
      let remaining_in_buffer = buffer.len() - i;
      let copy_len = std::cmp::min(rand_bytes_arr.len(), remaining_in_buffer);
      buffer[i..i + copy_len].copy_from_slice(&rand_bytes_arr[0..copy_len]);
      i += copy_len;
    }
  }
}


// --- UUID Generation Function ---
/// Generates a v4 UUID string.
/// Attempts OS-specific RNG first, then falls back to time-seeded XorShift64.
/// This function is self-contained for its randomness needs.
pub fn generate_v4_uuid() -> Result<String, std::io::Error> {
  let mut bytes = [0u8; 16];
  // Use the new fill_bytes function for populating the UUID bytes.
  // This centralizes the random byte generation logic.
  fill_bytes(&mut bytes);
  // Warnings about fallback mechanisms will be printed by fill_bytes itself.

  bytes[6] = (bytes[6] & 0x0F) | 0x40; // Version 4
  bytes[8] = (bytes[8] & 0x3F) | 0x80; // Variant 1 (RFC 4122)

  let mut uuid_str = String::with_capacity(36);
  for (i, byte_val) in bytes.iter().enumerate() {
    if i == 4 || i == 6 || i == 8 || i == 10 {
      if FmtWrite::write_char(&mut uuid_str, '-').is_err() { // Use aliased FmtWrite
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to write hyphen"));
      }
    }
    if write!(uuid_str, "{:02x}", byte_val).is_err() {
      return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to write byte"));
    }
  }
  Ok(uuid_str)
}

/// Generate a UUID. Calls generate_v4_uuid
pub fn uuid() -> Result<String, std::io::Error> {
  generate_v4_uuid()
}


/* --- Main function for testing ---
 f n* main() {
 // Initialize the global RNG (important if using global functions)
 Rand::init(); // This will print info/warnings about seeding.

 println!("\n--- Global PRNG Tests (seeded via Rand::init) ---");
 println!("Global rand u32: {}", rand());
 println!("Global rand i64: {}", rand_i64());
 println!("Global rand_range(1, 100): {}", rand_range(1, 100));
 println!("Global rand_range_i64(-50, 50): {}", rand_range_i64(-50, 50));
 println!("Global rand_float: {}", rand_float());

 let mut numbers = vec![1, 2, 3, 4, 5];
 println!("Original numbers: {:?}", numbers);
 shuffle(&mut numbers);
 println!("Shuffled numbers (global): {:?}", numbers);

 println!("\n--- Instance PRNG Tests (user-defined seed) ---");
 let mut my_rand = Rand::new(42); // Create a local instance with a fixed seed
 println!("Instance rand u32: {}", my_rand.rand());
 println!("Instance gen_i64: {}", my_rand.gen_i64());
 println!("Instance rand_range(10, 20): {}", my_rand.rand_range(10, 20));
 println!("Instance rand_range_i64(1000, 1010): {}", my_rand.rand_range_i64(1000, 1010));
 println!("Instance rand_float: {}", my_rand.rand_float());

 let mut more_numbers = vec!['a', 'b', 'c', 'd', 'e'];
 println!("Original chars: {:?}", more_numbers);
 my_rand.shuffle(&mut more_numbers);
 println!("Shuffled chars (instance): {:?}", more_numbers);

 println!("\n--- fill_bytes Test ---");
 let mut byte_buffer_32 = [0u8; 32];
 fill_bytes(&mut byte_buffer_32);
 println!("Filled 32-byte buffer: {:02X?}", byte_buffer_32);

 let mut byte_buffer_5 = [0u8; 5];
 fill_bytes(&mut byte_buffer_5);
 println!("Filled 5-byte buffer: {:02X?}", byte_buffer_5);


 println!("\n--- UUID Generation Test ---");
 for i in 0..3 {
   match generate_v4_uuid() {
   Ok(uuid) => println!("Generated UUID {}: {}", i + 1, uuid),
   Err(e) => eprintln!("Error generating UUID {}: {}", i + 1, e),
   }
   }
   }
   */
