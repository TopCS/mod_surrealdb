//! SurrealDB FFI shim for C callers (FreeSWITCH module).
//! This initial version is a stubbed implementation so the C side can link and evolve.

use std::ffi::{CStr};
use std::os::raw::{c_char, c_int};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicI32, Ordering};

#[repr(C)]
pub struct SurHandle {
    // Placeholder fields. Real implementation will hold client, runtime, etc.
    is_connected: bool,
    last_error_code: AtomicI32,
}

fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() { return None; }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

#[no_mangle]
pub extern "C" fn surreal_init_runtime() -> c_int {
    // In a full implementation, spin up a Tokio runtime once.
    // For now, no-op success.
    0
}

#[no_mangle]
pub extern "C" fn surreal_connect(
    url: *const c_char,
    ns: *const c_char,
    db: *const c_char,
    user: *const c_char,
    pass: *const c_char,
) -> *mut SurHandle {
    let _ = (cstr_to_str(url), cstr_to_str(ns), cstr_to_str(db), cstr_to_str(user), cstr_to_str(pass));

    // TODO: replace stub with actual client connect using SurrealDB v3 Rust client
    let handle = Box::new(SurHandle { is_connected: true, last_error_code: AtomicI32::new(0) });
    Box::into_raw(handle)
}

/// Publish a JSON payload to a given table/record or channel.
/// Returns 0 on success, non-zero on error.
#[no_mangle]
pub extern "C" fn surreal_publish(
    handle: *mut SurHandle,
    table_or_topic: *const c_char,
    json_payload: *const c_char,
) -> c_int {
    if handle.is_null() { return -1; }
    let h = unsafe { &*handle };
    if !h.is_connected { return -2; }

    let _table = match cstr_to_str(table_or_topic) { Some(s) => s, None => return -3 };
    let _json = match cstr_to_str(json_payload) { Some(s) => s, None => return -4 };

    // TODO: perform actual SurrealDB write (e.g., create/update or RPC)
    0
}

#[no_mangle]
pub extern "C" fn surreal_close(handle: *mut SurHandle) {
    if handle.is_null() { return; }
    let _ = unsafe { Box::from_raw(handle) };
}

#[no_mangle]
pub extern "C" fn surreal_last_error_code(handle: *mut SurHandle) -> c_int {
    if handle.is_null() { return -1; }
    let h = unsafe { &*handle };
    h.last_error_code.load(Ordering::Relaxed)
}

