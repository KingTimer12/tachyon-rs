#![deny(clippy::all)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Arc;
use tachyon_core::Tachyon as CoreTachyon;

pub struct Tachyon {
    inner: Arc<CoreTachyon>,
}

impl Tachyon {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CoreTachyon::new()),
        }
    }

    pub fn listen(&self, port: u16) -> String {
        format!("Server listening on port {}", port)
    }
}

// FFI exports for Deno
#[no_mangle]
pub extern "C" fn tachyon_new() -> *mut Tachyon {
    let tachyon = Box::new(Tachyon::new());
    Box::into_raw(tachyon)
}

#[no_mangle]
pub extern "C" fn tachyon_listen(ptr: *mut Tachyon, port: u16) -> *mut c_char {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }

    let tachyon = unsafe { &*ptr };
    let result = tachyon.listen(port);

    match CString::new(result) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn tachyon_free(ptr: *mut Tachyon) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

#[no_mangle]
pub extern "C" fn tachyon_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}
