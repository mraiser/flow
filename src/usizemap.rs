use std::ops::Index;
use std::mem;
use std::fmt;

pub struct UsizeMap<T> {
  data: Vec<Option<T>>,
  empty: Vec<usize>,
}

impl<T: std::fmt::Debug> UsizeMap<T> {
  pub fn new() -> UsizeMap<T> {
    UsizeMap {
      data: Vec::new(),
      empty: Vec::new(),
    }
  }
  
  pub fn insert(&mut self, t:T) -> usize {
    if self.empty.len() > 0 {
      let i = self.empty.remove(0);
      self.data[i] = Some(t);
      return i;
    }
    let i = self.data.len();
    self.data.push(Some(t));
    i
  }
  
  pub fn get_mut(&mut self, i:usize) -> Option<&mut T> {
    self.data[i].as_mut()
  } 
  
  pub fn remove(&mut self, i:usize) -> Option<T> {
    self.empty.push(i);
    mem::replace(&mut self.data[i], None)
  }
  
  pub fn len(&self) -> usize {
    self.data.len() - self.empty.len()
  }
}

impl<T> Index<usize> for UsizeMap<T> {
  type Output = T;
  
  fn index(&self, i: usize) -> &Self::Output {
    self.data[i].as_ref().unwrap()
  }
}

impl<T: std::fmt::Debug> fmt::Debug for UsizeMap<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "count {}, empty {}", self.len(), self.empty.len()).unwrap();
    Ok(())
  }
}

