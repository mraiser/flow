use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
struct Blob<T> {
  data: T,
  count: usize,
}

pub struct Heap<T> {
  data: HashMap<usize, Blob<T>>,
  ref_index: usize,
}

impl<T: std::fmt::Debug> Heap<T> {
  pub fn new() -> Heap<T> {
    Heap {
      data: HashMap::<usize, Blob<T>>::new(),
      ref_index: 0,
    }
  }

  pub fn push(&mut self, data: T) -> usize {
    let index = self.ref_index;
    self.ref_index += 1;
    
    let blob = Blob{
      data: data,
      count: 1,
    };
    
    self.data.insert(index, blob);
    index
  }
  
  pub fn get(&mut self, index:usize) -> &mut T {
    &mut self.data.get_mut(&index).unwrap().data
  }

  pub fn count(&mut self, index:usize) -> usize {
    self.data[&index].count
  }

  pub fn incr(&mut self, index:usize) {
    self.data.get_mut(&index).unwrap().count += 1;
  }
 
  pub fn decr(&mut self, index: usize) {
    let b = self.data.get_mut(&index).unwrap();
    let c = b.count;
    if c == 1 {
      self.data.remove(&index);
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
    while i<self.ref_index {
      if let Some(blob) = self.data.get(&i) {
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
