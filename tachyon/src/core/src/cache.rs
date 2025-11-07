use std::sync::{Arc, atomic::AtomicPtr};

use crate::router::TachyonHandler;

pub struct HotCache {
  key: AtomicPtr<String>,
  handler: AtomicPtr<TachyonHandler>,
}

impl HotCache {
  pub fn new() -> Self {
    Self {
      key: AtomicPtr::new(std::ptr::null_mut()),
      handler: AtomicPtr::new(std::ptr::null_mut()),
    }
  }

  #[inline(always)]
  pub fn get(&self, route_key: &str) -> Option<TachyonHandler> {
    use std::sync::atomic::Ordering;

    let key_ptr = self.key.load(Ordering::Acquire);
    if key_ptr.is_null() {
      return None;
    }

    unsafe {
      let cached_key = &*key_ptr;
      if cached_key == route_key {
        let handler_ptr = self.handler.load(Ordering::Acquire);
        if !handler_ptr.is_null() {
          let handler = &*handler_ptr;
          return Some(Arc::clone(handler));
        }
      }
    }
    None
  }

  #[inline(always)]
  pub fn set(&self, route_key: String, handler: TachyonHandler) {
    use std::sync::atomic::Ordering;

    let key_box = Box::new(route_key);
    let handler_box = Box::new(handler);

    let old_key = self.key.swap(Box::into_raw(key_box), Ordering::Release);
    let old_handler = self
      .handler
      .swap(Box::into_raw(handler_box), Ordering::Release);

    // Clean up old values
    if !old_key.is_null() {
      unsafe {
        drop(Box::from_raw(old_key));
      }
    }
    if !old_handler.is_null() {
      unsafe {
        drop(Box::from_raw(old_handler));
      }
    }
  }
}

impl Drop for HotCache {
  fn drop(&mut self) {
    use std::sync::atomic::Ordering;

    let key_ptr = self.key.load(Ordering::Acquire);
    let handler_ptr = self.handler.load(Ordering::Acquire);

    if !key_ptr.is_null() {
      unsafe {
        drop(Box::from_raw(key_ptr));
      }
    }
    if !handler_ptr.is_null() {
      unsafe {
        drop(Box::from_raw(handler_ptr));
      }
    }
  }
}