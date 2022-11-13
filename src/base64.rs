const MAPS:Maps = Maps::new();

pub struct Maps {
  map1: [char; 64],
  map2: [i8; 128],
}

impl Maps {
  pub const fn new() -> Maps{
    let mut map1: [char; 64] = [0 as char; 64];
    let mut i = 0;
    let mut c = 'A';
    while c <= 'Z' { map1[i] = c; i += 1; c = (c as u8 + 1) as char; }
    let mut c = 'a';
    while c <= 'z' { map1[i] = c; i += 1; c = (c as u8 + 1) as char; }
    let mut c = '0';
    while c <= '9' { map1[i] = c; i += 1; c = (c as u8 + 1) as char; }
    map1[i] = '+'; i += 1;
    map1[i] = '/';
    
    let mut map2:[i8; 128] = [-1; 128];
    let mut i = 0;
    while i<64 {
      map2[map1[i] as usize] = i as i8;
      i += 1;
    }
    
    Maps {
      map1: map1,
      map2: map2,
    }
  }
}

pub struct Base64 {
}

impl Base64 {
  pub fn encode_string(input: &str) -> String {
   Base64::encode(input.as_bytes().to_vec()).into_iter().collect()
  }
  
  pub fn encode(input: Vec<u8>) -> Vec<char> {
    let ilen = input.len();
    let o_data_len = (ilen*4+2)/3;
    let olen = ((ilen+2)/3)*4;
    let mut output: Vec<char> = vec![0 as char; olen];
    let mut ip = 0;
    let mut op = 0;
    let maps = &MAPS;
    while ip < ilen {
      let i0 = input[ip] & 0xff; ip += 1;
      let i1; if ip < ilen { i1 = input[ip] & 0xff; ip += 1;} else { i1 = 0; }
      let i2; if ip < ilen { i2 = input[ip] & 0xff; ip += 1;} else { i2 = 0; }
      let o0 = i0 >> 2;
      let o1 = ((i0 &   3) << 4) | (i1 >> 4);
      let o2 = ((i1 & 0xf) << 2) | (i2 >> 6);
      let o3 = i2 & 0x3F;
      output[op] = maps.map1[o0 as usize]; op += 1;
      output[op] = maps.map1[o1 as usize]; op += 1;
      if op < o_data_len { output[op] = maps.map1[o2 as usize]; } else { output[op] = '='; } op += 1;
      if op < o_data_len { output[op] = maps.map1[o3 as usize]; } else { output[op] = '='; } op += 1; 
    }
    output
  }
  
  pub fn decode_string(input: &str) -> String {
    std::str::from_utf8(&Base64::decode(input.chars().collect())).unwrap().to_string()
  }
  
  pub fn decode(input: Vec<char>) -> Vec<u8> {
    let mut ilen = input.len();
    if ilen % 4 != 0 { panic!("Length of Base64 encoded input string is not a multiple of 4."); }
    while ilen > 0 && input[ilen-1] == '=' { ilen -= 1; }
    let olen = (ilen * 3) / 4;
    let mut output: Vec<u8> = vec![0; olen];
    let mut ip = 0;
    let mut op = 0;
    let maps = &MAPS;
    while ip < ilen {
      let i0 = input[ip]; ip += 1;
      let i1 = input [ip]; ip += 1;
      let i2; if ip < ilen { i2 = input[ip]; ip += 1; } else { i2 = 'A'; }
      let i3; if ip < ilen { i3 = input[ip]; ip += 1; } else { i3 = 'A'; }
      if i0 as u8 > 127 || i1 as u8 > 127 || i2 as u8 > 127 || i3 as u8 > 127 {
        panic!("Illegal character in Base64 encoded data.");
      }
      let b0 = maps.map2[i0 as usize];
      let b1 = maps.map2[i1 as usize];
      let b2 = maps.map2[i2 as usize];
      let b3 = maps.map2[i3 as usize];
      if (b0 as i8) < 0 || (b1 as i8) < 0 || (b2 as i8) < 0 || (b3 as i8) < 0 {
        panic!("Illegal character in Base64 encoded data.");
      }
      let o0 = ( b0       <<2) | (b1>>4);
      let o1 = ((b1 & 0xf)<<4) | (b2>>2);
      let o2 = ((b2 &   3)<<6) |  b3;
      output[op] = o0 as u8; op += 1;
      if op < olen { output[op] = o1 as u8; op += 1; }
      if op < olen { output[op] = o2 as u8; op += 1; }
    }
    output
  }
}
