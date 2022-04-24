use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
struct Blob<T> {
  data: T,
  count: usize,
  index: usize,
}

pub struct Heap<T> {
  data: Vec<Blob<T>>,
  ref_index: usize,
}

impl<T: std::fmt::Debug> Heap<T> {
  pub fn new() -> Heap<T> {
    Heap {
      data: Vec::new(),
      ref_index: 0,
    }
  }

  pub fn push(&mut self, data: T) -> usize {
    let index = self.ref_index;
    self.ref_index += 1;
    
    let blob = Blob{
      data: data,
      count: 1,
      index: index,
    };
    
    self.data.push(blob);
    index
  }
  
  fn lookup(&mut self, index:usize) -> (usize, &mut Blob<T>) {
    let mut i = self.data.len();
    let data = &mut self.data;
    while i>0 {
      i -= 1;
      let x = &mut data[i];
      if x.index == index {
        break;
      }
    }
    (i,&mut data[i])
  }
  
  pub fn get(&mut self, index:usize) -> &mut T {
    &mut self.lookup(index).1.data
  }

  pub fn count(&mut self, index:usize) -> usize {
    self.lookup(index).1.count
  }

  pub fn incr(&mut self, index:usize) {
    let b = self.lookup(index).1;
    b.count += 1;
  }
 
  pub fn decr(&mut self, index: usize) {
    let (i, b) = &mut self.lookup(index);
    let c = b.count;
    if c == 1 {
      self.data.remove(*i);
    }
    else {
      b.count = c-1;
    }
  }
}

impl<T: std::fmt::Debug> fmt::Debug for Heap<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "ref_index {}", self.ref_index).unwrap();
    let mut i = 0;
    while i<self.data.len() {
      if let Some(blob) = self.data.get(i) {
        let c = blob.count;
        let mut s = format!("{:?}", blob);
        if s.len() > 66 { s = s[0..66].to_string()+"..."; }
        writeln!(f, "{}: {} - {}", i, c, s).unwrap();
      }
      i = i + 1;
    }
    Ok(())
  }
}
