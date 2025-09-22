//! SurrealDB FFI shim for C callers (FreeSWITCH module).
//! This initial version is a stubbed implementation so the C side can link and evolve.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

pub type SurCommandCb = Option<extern "C" fn(topic: *const c_char, json: *const c_char, user_data: *mut c_void)>;

#[repr(C)]
pub struct SurHandle {
    // Placeholder fields. Real implementation will hold client, runtime, etc.
    is_connected: bool,
    last_error_code: AtomicI32,
    // Subscription state (stub): a single callback and user data.
    callback: Mutex<Option<(SurCommandCb, *mut c_void)>>,
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
    let handle = Box::new(SurHandle { is_connected: true, last_error_code: AtomicI32::new(0), callback: Mutex::new(None) });
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

/// Subscribes to incoming commands/messages (stub). The real implementation
/// will register a listener on SurrealDB and invoke the callback on events.
/// Returns 0 on success.
#[no_mangle]
pub extern "C" fn surreal_subscribe(
    handle: *mut SurHandle,
    _topic: *const c_char,
    cb: SurCommandCb,
    user_data: *mut c_void,
) -> c_int {
    if handle.is_null() { return -1; }
    let h = unsafe { &*handle };
    let mut guard = match h.callback.lock() { Ok(g) => g, Err(_) => return -2 };
    *guard = Some((cb, user_data));
    0
}

/// Unsubscribes from incoming commands/messages (stub).
#[no_mangle]
pub extern "C" fn surreal_unsubscribe(handle: *mut SurHandle, _topic: *const c_char) -> c_int {
    if handle.is_null() { return -1; }
    let h = unsafe { &*handle };
    let mut guard = match h.callback.lock() { Ok(g) => g, Err(_) => return -2 };
    *guard = None;
    0
}

/// Testing helper (stub only): emits a message to the registered callback.
#[no_mangle]
pub extern "C" fn surreal_debug_emit(handle: *mut SurHandle, topic: *const c_char, json: *const c_char) -> c_int {
    if handle.is_null() { return -1; }
    let h = unsafe { &*handle };
    let guard = match h.callback.lock() { Ok(g) => g, Err(_) => return -2 };
    if let Some((Some(cb), user)) = guard.as_ref() {
        cb(topic, json, *user);
        0
    } else { -3 }
}
