/*
    Adapted to Rust from: https://github.com/vog/sha1

    Original C Code
        -- Steve Reid <steve@edmweb.com>
    Small changes to fit into bglibs
        -- Bruce Guenter <bruce@untroubled.org>
    Translation to simpler C++ Code
        -- Volker Diels-Grabsch <v@njh.eu>
    Safety fixes
        -- Eugene Hopkinson <slowriot at voxelstorm dot com>
    Header-only library
        -- Zlatko Michailov <zlatko@michailov.org>
*/

use std::cmp;

const BLOCK_INTS: usize = 16;  /* number of 32bit integers per SHA1 block */
const BLOCK_BYTES: usize = BLOCK_INTS * 4;

pub struct SHA1 {
  digest: [u32; 5],
  buffer: Vec<u8>,
  transforms: u64,
}

impl SHA1 {
  pub fn new() -> SHA1 {
    let mut d = SHA1{
      digest: [0,0,0,0,0],
      buffer: Vec::new(),
      transforms: 0,
    };
    d.reset();
    d
  }
  
  pub fn reset(&mut self) {
    /* SHA1 initialization constants */
    self.digest[0] = 0x67452301;
    self.digest[1] = 0xefcdab89;
    self.digest[2] = 0x98badcfe;
    self.digest[3] = 0x10325476;
    self.digest[4] = 0xc3d2e1f0;

    /* Reset counters */
    self.buffer = Vec::new();
    self.transforms = 0;
  }
  
  pub fn update(&mut self, s:&str) {
    self.update_bytes(s.as_bytes().to_vec());
  }
  
  pub fn update_bytes(&mut self, mut ba: Vec<u8>) {
    loop {
      let n = cmp::min(ba.len(), BLOCK_BYTES - self.buffer.len());
      self.buffer.extend_from_slice(&ba[0..n]);
      if self.buffer.len() != BLOCK_BYTES {
        return;
      }
      ba = ba[n..].to_vec();
      let block = self.buffer_to_block();
      self.transform(block);
      self.buffer = Vec::new();
    }
  }
  
  pub fn finish(&mut self) -> Vec<u8> {
    /* Total number of hashed bits */
    let total_bits:u64 = (self.transforms * (BLOCK_BYTES as u64) + (self.buffer.len() as u64)) * 8;

    /* Padding */
    self.buffer.push(0x80);
    let orig_size = self.buffer.len();
    while self.buffer.len() < BLOCK_BYTES {
        self.buffer.push(0x00);
    }
    let mut block = self.buffer_to_block();
    if orig_size > BLOCK_BYTES - 8 {
      self.transform(block);
      let mut i = 0;
      while i < BLOCK_INTS - 2 {
        block[i] = 0;
        i += 1;
      }
    }
    
    /* Append total_bits, split this uint64_t into two uint32_t */
    block[BLOCK_INTS - 1] = total_bits as u32;
    block[BLOCK_INTS - 2] = (total_bits >> 32) as u32;
    self.transform(block);
    
    let res = to_bytes(&self.digest);
    
    /* Reset for next run */
    self.reset();
    
    res
  }
  
  pub fn buffer_to_block(&self) -> [u32; BLOCK_INTS]{
    let mut block: [u32; BLOCK_INTS] = [0; BLOCK_INTS];
    let mut i = 0;
    while i < BLOCK_INTS {
      block[i] = (self.buffer[4*i+3] as u32 & 0xff)
                 | (self.buffer[4*i+2] as u32 & 0xff)<<8
                 | (self.buffer[4*i+1] as u32 & 0xff)<<16
                 | (self.buffer[4*i+0] as u32 & 0xff)<<24;
      i += 1;
    }
    block
  }
  
  pub fn transform(&mut self, mut block:[u32; BLOCK_INTS]) {
    /* Copy digest[] to working vars */
    let mut a = self.digest[0];
    let mut b = self.digest[1];
    let mut c = self.digest[2];
    let mut d = self.digest[3];
    let mut e = self.digest[4];

    /* 4 rounds of 20 operations each. Loop unrolled. */
    (b, e, block) = r0(block, a, b, c, d, e,  0);
    (a, d, block) = r0(block, e, a, b, c, d,  1);
    (e, c, block) = r0(block, d, e, a, b, c,  2);
    (d, b, block) = r0(block, c, d, e, a, b,  3);
    (c, a, block) = r0(block, b, c, d, e, a,  4);
    (b, e, block) = r0(block, a, b, c, d, e,  5);
    (a, d, block) = r0(block, e, a, b, c, d,  6);
    (e, c, block) = r0(block, d, e, a, b, c,  7);
    (d, b, block) = r0(block, c, d, e, a, b,  8);
    (c, a, block) = r0(block, b, c, d, e, a,  9);
    (b, e, block) = r0(block, a, b, c, d, e, 10);
    (a, d, block) = r0(block, e, a, b, c, d, 11);
    (e, c, block) = r0(block, d, e, a, b, c, 12);
    (d, b, block) = r0(block, c, d, e, a, b, 13);
    (c, a, block) = r0(block, b, c, d, e, a, 14);
    (b, e, block) = r0(block, a, b, c, d, e, 15);
    (a, d, block) = r1(block, e, a, b, c, d,  0);
    (e, c, block) = r1(block, d, e, a, b, c,  1);
    (d, b, block) = r1(block, c, d, e, a, b,  2);
    (c, a, block) = r1(block, b, c, d, e, a,  3);
    (b, e, block) = r2(block, a, b, c, d, e,  4);
    (a, d, block) = r2(block, e, a, b, c, d,  5);
    (e, c, block) = r2(block, d, e, a, b, c,  6);
    (d, b, block) = r2(block, c, d, e, a, b,  7);
    (c, a, block) = r2(block, b, c, d, e, a,  8);
    (b, e, block) = r2(block, a, b, c, d, e,  9);
    (a, d, block) = r2(block, e, a, b, c, d, 10);
    (e, c, block) = r2(block, d, e, a, b, c, 11);
    (d, b, block) = r2(block, c, d, e, a, b, 12);
    (c, a, block) = r2(block, b, c, d, e, a, 13);
    (b, e, block) = r2(block, a, b, c, d, e, 14);
    (a, d, block) = r2(block, e, a, b, c, d, 15);
    (e, c, block) = r2(block, d, e, a, b, c,  0);
    (d, b, block) = r2(block, c, d, e, a, b,  1);
    (c, a, block) = r2(block, b, c, d, e, a,  2);
    (b, e, block) = r2(block, a, b, c, d, e,  3);
    (a, d, block) = r2(block, e, a, b, c, d,  4);
    (e, c, block) = r2(block, d, e, a, b, c,  5);
    (d, b, block) = r2(block, c, d, e, a, b,  6);
    (c, a, block) = r2(block, b, c, d, e, a,  7);
    (b, e, block) = r3(block, a, b, c, d, e,  8);
    (a, d, block) = r3(block, e, a, b, c, d,  9);
    (e, c, block) = r3(block, d, e, a, b, c, 10);
    (d, b, block) = r3(block, c, d, e, a, b, 11);
    (c, a, block) = r3(block, b, c, d, e, a, 12);
    (b, e, block) = r3(block, a, b, c, d, e, 13);
    (a, d, block) = r3(block, e, a, b, c, d, 14);
    (e, c, block) = r3(block, d, e, a, b, c, 15);
    (d, b, block) = r3(block, c, d, e, a, b,  0);
    (c, a, block) = r3(block, b, c, d, e, a,  1);
    (b, e, block) = r3(block, a, b, c, d, e,  2);
    (a, d, block) = r3(block, e, a, b, c, d,  3);
    (e, c, block) = r3(block, d, e, a, b, c,  4);
    (d, b, block) = r3(block, c, d, e, a, b,  5);
    (c, a, block) = r3(block, b, c, d, e, a,  6);
    (b, e, block) = r3(block, a, b, c, d, e,  7);
    (a, d, block) = r3(block, e, a, b, c, d,  8);
    (e, c, block) = r3(block, d, e, a, b, c,  9);
    (d, b, block) = r3(block, c, d, e, a, b, 10);
    (c, a, block) = r3(block, b, c, d, e, a, 11);
    (b, e, block) = r4(block, a, b, c, d, e, 12);
    (a, d, block) = r4(block, e, a, b, c, d, 13);
    (e, c, block) = r4(block, d, e, a, b, c, 14);
    (d, b, block) = r4(block, c, d, e, a, b, 15);
    (c, a, block) = r4(block, b, c, d, e, a,  0);
    (b, e, block) = r4(block, a, b, c, d, e,  1);
    (a, d, block) = r4(block, e, a, b, c, d,  2);
    (e, c, block) = r4(block, d, e, a, b, c,  3);
    (d, b, block) = r4(block, c, d, e, a, b,  4);
    (c, a, block) = r4(block, b, c, d, e, a,  5);
    (b, e, block) = r4(block, a, b, c, d, e,  6);
    (a, d, block) = r4(block, e, a, b, c, d,  7);
    (e, c, block) = r4(block, d, e, a, b, c,  8);
    (d, b, block) = r4(block, c, d, e, a, b,  9);
    (c, a, block) = r4(block, b, c, d, e, a, 10);
    (b, e, block) = r4(block, a, b, c, d, e, 11);
    (a, d, block) = r4(block, e, a, b, c, d, 12);
    (e, c, block) = r4(block, d, e, a, b, c, 13);
    (d, b, block) = r4(block, c, d, e, a, b, 14);
    (c, a, _) = r4(block, b, c, d, e, a, 15);

    /* Add the working vars back into digest[] */
    self.digest[0] = self.digest[0].wrapping_add(a);
    self.digest[1] = self.digest[1].wrapping_add(b);
    self.digest[2] = self.digest[2].wrapping_add(c);
    self.digest[3] = self.digest[3].wrapping_add(d);
    self.digest[4] = self.digest[4].wrapping_add(e);

    /* Count the number of transformations */
    self.transforms += 1;
  }
}

fn r0(block: [u32; BLOCK_INTS], v: u32, mut w: u32, x: u32, y: u32, mut z: u32, i: usize) -> (u32, u32, [u32; BLOCK_INTS]) {
  z = z.wrapping_add(((w&(x^y))^y).wrapping_add(block[i]).wrapping_add(0x5a827999).wrapping_add(rol(v, 5)));
  w = rol(w, 30);
  (w, z, block)
}

fn r1(mut block: [u32; BLOCK_INTS], v: u32, mut w: u32, x: u32, y: u32, mut z: u32, i: usize) -> (u32, u32, [u32; BLOCK_INTS]) {
  block[i] = blk(block, i);
  z = z.wrapping_add((w&(x^y))^y).wrapping_add(block[i]).wrapping_add(0x5a827999).wrapping_add(rol(v, 5));
  w = rol(w, 30);
  (w, z, block)
}

fn r2(mut block: [u32; BLOCK_INTS], v: u32, mut w: u32, x: u32, y: u32, mut z: u32, i: usize) -> (u32, u32, [u32; BLOCK_INTS]) {
  block[i] = blk(block, i);
  z = z.wrapping_add(w^x^y).wrapping_add(block[i]).wrapping_add(0x6ed9eba1).wrapping_add(rol(v, 5));
  w = rol(w, 30);
  (w, z, block)
}

fn r3(mut block: [u32; BLOCK_INTS], v: u32, mut w: u32, x: u32, y: u32, mut z: u32, i: usize) -> (u32, u32, [u32; BLOCK_INTS]) {
  block[i] = blk(block, i);
  z = z.wrapping_add(((w|x)&y)|(w&x)).wrapping_add(block[i]).wrapping_add(0x8f1bbcdc).wrapping_add(rol(v, 5));
  w = rol(w, 30);
  (w, z, block)
}

fn r4(mut block: [u32; BLOCK_INTS], v: u32, mut w: u32, x: u32, y: u32, mut z: u32, i: usize) -> (u32, u32, [u32; BLOCK_INTS]) {
  block[i] = blk(block, i);
  z = z.wrapping_add(w^x^y).wrapping_add(block[i]).wrapping_add(0xca62c1d6).wrapping_add(rol(v, 5));
  w = rol(w, 30);
  (w, z, block)
}

fn rol(value: u32, bits: usize) -> u32 {
  return (value << bits) | (value >> (32 - bits));
}

fn blk(block: [u32; BLOCK_INTS], i: usize) -> u32 {
  return rol(block[(i+13)&15] ^ block[(i+8)&15] ^ block[(i+2)&15] ^ block[i], 1);
}
  
fn to_bytes(input: &[u32]) -> Vec<u8> {
  let mut bytes = Vec::with_capacity(4 * input.len());

  for value in input {
      bytes.extend(&value.to_be_bytes());
  }

  bytes
}

