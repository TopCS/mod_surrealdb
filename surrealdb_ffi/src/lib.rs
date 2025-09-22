//! SurrealDB FFI shim for C callers (FreeSWITCH module).
//! Default build is a stub for offline development; enable `real` feature for client wiring.

#[cfg(feature = "stub")]
mod api {
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int, c_void};
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Mutex;

    pub type SurCommandCb = Option<extern "C" fn(topic: *const c_char, json: *const c_char, user_data: *mut c_void)>;

    #[repr(C)]
    pub struct SurHandle {
        pub(crate) is_connected: bool,
        pub(crate) last_error_code: AtomicI32,
        pub(crate) callback: Mutex<Option<(SurCommandCb, *mut c_void)>>,
    }

    fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
        if ptr.is_null() { return None; }
        unsafe { CStr::from_ptr(ptr) }.to_str().ok()
    }

    #[no_mangle]
    pub extern "C" fn surreal_init_runtime() -> c_int { 0 }

    #[no_mangle]
    pub extern "C" fn surreal_connect(url: *const c_char, ns: *const c_char, db: *const c_char, user: *const c_char, pass: *const c_char) -> *mut SurHandle {
        let _ = (cstr_to_str(url), cstr_to_str(ns), cstr_to_str(db), cstr_to_str(user), cstr_to_str(pass));
        let handle = Box::new(SurHandle { is_connected: true, last_error_code: AtomicI32::new(0), callback: Mutex::new(None) });
        Box::into_raw(handle)
    }

    #[no_mangle]
    pub extern "C" fn surreal_connect_with_token(url: *const c_char, ns: *const c_char, db: *const c_char, token: *const c_char) -> *mut SurHandle {
        let _ = (cstr_to_str(url), cstr_to_str(ns), cstr_to_str(db), cstr_to_str(token));
        let handle = Box::new(SurHandle { is_connected: true, last_error_code: AtomicI32::new(0), callback: Mutex::new(None) });
        Box::into_raw(handle)
    }

    #[no_mangle]
    pub extern "C" fn surreal_publish(handle: *mut SurHandle, table_or_topic: *const c_char, json_payload: *const c_char) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &*handle };
        if !h.is_connected { return -2; }
        let _ = (cstr_to_str(table_or_topic), cstr_to_str(json_payload));
        0
    }

    #[no_mangle]
    pub extern "C" fn surreal_close(handle: *mut SurHandle) { if handle.is_null() { return; } let _ = unsafe { Box::from_raw(handle) }; }

    #[no_mangle]
    pub extern "C" fn surreal_last_error_code(handle: *mut SurHandle) -> c_int { if handle.is_null() { return -1; } let h = unsafe { &*handle }; h.last_error_code.load(std::sync::atomic::Ordering::Relaxed) }

    #[no_mangle]
    pub extern "C" fn surreal_subscribe(handle: *mut SurHandle, _topic: *const c_char, cb: SurCommandCb, user_data: *mut c_void) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &*handle };
        let mut guard = match h.callback.lock() { Ok(g) => g, Err(_) => return -2 };
        *guard = Some((cb, user_data));
        0
    }

    #[no_mangle]
    pub extern "C" fn surreal_unsubscribe(handle: *mut SurHandle, _topic: *const c_char) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &*handle };
        let mut guard = match h.callback.lock() { Ok(g) => g, Err(_) => return -2 };
        *guard = None;
        0
    }

    #[no_mangle]
    pub extern "C" fn surreal_debug_emit(handle: *mut SurHandle, topic: *const c_char, json: *const c_char) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &*handle };
        let guard = match h.callback.lock() { Ok(g) => g, Err(_) => return -2 };
        if let Some((Some(cb), user)) = guard.as_ref() { cb(topic, json, *user); 0 } else { -3 }
    }
}

#[cfg(feature = "real")]
mod api {
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int, c_void};
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::{Mutex, OnceLock};

    use serde_json::Value as JsonValue;
    use tokio::runtime::Runtime;

    // type Client = surrealdb::Surreal<surrealdb::engine::remote::ws::Client>;
    // We'll wire the actual client when online docs are available.

    static RUNTIME: OnceLock<Runtime> = OnceLock::new();

    pub type SurCommandCb = Option<extern "C" fn(topic: *const c_char, json: *const c_char, user_data: *mut c_void)>;

    #[repr(C)]
    pub struct SurHandle {
        last_error_code: AtomicI32,
        callback: Mutex<Option<(SurCommandCb, *mut c_void)>>,
        // TODO: store client, ns, db, and connection state
    }

    fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> { if ptr.is_null() { return None; } unsafe { CStr::from_ptr(ptr) }.to_str().ok() }

    #[no_mangle]
    pub extern "C" fn surreal_init_runtime() -> c_int {
        if RUNTIME.get().is_some() { return 0; }
        match Runtime::new() {
            Ok(rt) => { let _ = RUNTIME.set(rt); 0 }
            Err(_) => -1,
        }
    }

    #[no_mangle]
    pub extern "C" fn surreal_connect(url: *const c_char, ns: *const c_char, db: *const c_char, user: *const c_char, pass: *const c_char) -> *mut SurHandle {
        let _ = (cstr_to_str(url), cstr_to_str(ns), cstr_to_str(db), cstr_to_str(user), cstr_to_str(pass));
        // TODO: Use RUNTIME.get().unwrap().block_on(...) to open connection and authenticate
        let handle = Box::new(SurHandle { last_error_code: AtomicI32::new(0), callback: Mutex::new(None) });
        Box::into_raw(handle)
    }

    #[no_mangle]
    pub extern "C" fn surreal_connect_with_token(url: *const c_char, ns: *const c_char, db: *const c_char, token: *const c_char) -> *mut SurHandle {
        let _ = (cstr_to_str(url), cstr_to_str(ns), cstr_to_str(db), cstr_to_str(token));
        let handle = Box::new(SurHandle { last_error_code: AtomicI32::new(0), callback: Mutex::new(None) });
        Box::into_raw(handle)
    }

    #[no_mangle]
    pub extern "C" fn surreal_publish(handle: *mut SurHandle, table_or_topic: *const c_char, json_payload: *const c_char) -> c_int {
        if handle.is_null() { return -1; }
        let _table = match cstr_to_str(table_or_topic) { Some(s) => s, None => return -3 };
        let json = match cstr_to_str(json_payload) { Some(s) => s, None => return -4 };
        // Validate JSON early
        if serde_json::from_str::<JsonValue>(json).is_err() { return -5; }
        // TODO: Perform actual write with client using runtime
        0
    }

    #[no_mangle]
    pub extern "C" fn surreal_close(handle: *mut SurHandle) { if handle.is_null() { return; } let _ = unsafe { Box::from_raw(handle) }; }

    #[no_mangle]
    pub extern "C" fn surreal_last_error_code(handle: *mut SurHandle) -> c_int { if handle.is_null() { return -1; } let h = unsafe { &*handle }; h.last_error_code.load(Ordering::Relaxed) }

    #[no_mangle]
    pub extern "C" fn surreal_subscribe(handle: *mut SurHandle, _topic: *const c_char, cb: SurCommandCb, user_data: *mut c_void) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &*handle };
        let mut guard = match h.callback.lock() { Ok(g) => g, Err(_) => return -2 };
        *guard = Some((cb, user_data));
        // TODO: start live query with runtime and dispatch into cb
        0
    }

    #[no_mangle]
    pub extern "C" fn surreal_unsubscribe(handle: *mut SurHandle, _topic: *const c_char) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &*handle };
        let mut guard = match h.callback.lock() { Ok(g) => g, Err(_) => return -2 };
        *guard = None;
        // TODO: stop live query
        0
    }

    #[no_mangle]
    pub extern "C" fn surreal_debug_emit(_handle: *mut SurHandle, _topic: *const c_char, _json: *const c_char) -> c_int { -1 }
}

pub use api::*;
