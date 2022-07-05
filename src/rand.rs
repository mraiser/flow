use std::time::{SystemTime, UNIX_EPOCH};

const KX: u32 = 123456789;
const KY: u32 = 362436069;
const KZ: u32 = 521288629;
const KW: u32 = 88675123;

static mut RANDOM:(u32, u32, u32, u32) = (0,0,0,0);

pub struct Rand {
    x: u32, y: u32, z: u32, w: u32
}

impl Rand{
    pub fn init() {
        let nanos = SystemTime::now()
          .duration_since(UNIX_EPOCH)
          .unwrap()
          .subsec_nanos();
        let rand = Rand::new(nanos);
        
        unsafe { 
          RANDOM = rand.get();
        }
    }
    
    pub fn new(seed: u32) -> Rand {
        Rand{
            x: KX^seed, y: KY^seed,
            z: KZ, w: KW
        }
    }
    
    pub fn get(&self) -> (u32, u32, u32, u32) {
      (self.x, self.y, self.z, self.w)
    }
    
    pub fn build(x:u32, y:u32, z:u32, w:u32) -> Rand {
        Rand{
            x: x, y: y,
            z: z, w: w
        }
    }

    // Xorshift 128, taken from German Wikipedia
    pub fn rand(&mut self) -> u32 {
        let t = self.x^self.x.wrapping_shl(11);
        self.x = self.y; self.y = self.z; self.z = self.w;
        self.w ^= self.w.wrapping_shr(19)^t^t.wrapping_shr(8);
        return self.w;
    }

    pub fn shuffle<T>(&mut self, a: &mut [T]) {
        if a.len()==0 {return;}
        let mut i = a.len()-1;
        while i>0 {
            let j = (self.rand() as usize)%(i+1);
            a.swap(i,j);
            i-=1;
        }
    }

    pub fn rand_range(&mut self, a: i32, b: i32) -> i32 {
        let m = (b-a+1) as u32;
        return a+(self.rand()%m) as i32;
    }

    pub fn rand_float(&mut self) -> f64 {
        (self.rand() as f64)/(<u32>::max_value() as f64)
    }
}

pub fn rand() -> u32 {
  unsafe {
    let mut rand = Rand::build(RANDOM.0, RANDOM.1, RANDOM.2, RANDOM.3);
    let x = rand.rand();
    RANDOM = rand.get();
    return x;
  }
}

pub fn shuffle<T>(a: &mut [T]) {
  unsafe {
    let mut rand = Rand::build(RANDOM.0, RANDOM.1, RANDOM.2, RANDOM.3);
    let o = rand.shuffle(a);
    RANDOM = rand.get();
    return o;
  }
}

pub fn rand_range(a: i32, b: i32) -> i32 {
  unsafe {
    let mut rand = Rand::build(RANDOM.0, RANDOM.1, RANDOM.2, RANDOM.3);
    let o = rand.rand_range(a, b);
    RANDOM = rand.get();
    return o;
  }
}

pub fn rand_float() -> f64 {
  unsafe {
    let mut rand = Rand::build(RANDOM.0, RANDOM.1, RANDOM.2, RANDOM.3);
    let o = rand.rand_float();
    RANDOM = rand.get();
    return o;
  }
}



