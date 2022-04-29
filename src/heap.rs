use crate::usizemap::*;

#[derive(Debug)]
struct Blob<T> {
  data: T,
  count: usize,
}

#[derive(Debug)]
pub struct Heap<T> {
  data: UsizeMap<Blob<T>>,
}

impl<T: std::fmt::Debug> Heap<T> {
  pub fn new() -> Heap<T> {
    Heap {
      data: UsizeMap::<Blob<T>>::new(),
    }
  }

  pub fn push(&mut self, data: T) -> usize {
    let blob = Blob{
      data: data,
      count: 1,
    };
    
    self.data.insert(blob)
  }
  
  pub fn get(&mut self, index:usize) -> &mut T {
    &mut self.data.get_mut(index).unwrap().data
  }

  pub fn count(&mut self, index:usize) -> usize {
    self.data[index].count
  }

  pub fn incr(&mut self, index:usize) {
    self.data.get_mut(index).unwrap().count += 1;
  }
 
  pub fn decr(&mut self, index: usize) {
    let b = self.data.get_mut(index).unwrap();
    let c = b.count;
    if c == 1 {
      self.data.remove(index);
    }
    else {
      b.count = c-1;
    }
  }
}


