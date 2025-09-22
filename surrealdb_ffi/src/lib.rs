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
    // Real client wiring placeholder.
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int, c_void};
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Mutex;

    pub type SurCommandCb = Option<extern "C" fn(topic: *const c_char, json: *const c_char, user_data: *mut c_void)>;

    #[repr(C)]
    pub struct SurHandle {
        last_error_code: AtomicI32,
        callback: Mutex<Option<(SurCommandCb, *mut c_void)>>,
    }

    fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> { if ptr.is_null() { return None; } unsafe { CStr::from_ptr(ptr) }.to_str().ok() }

    #[no_mangle]
    pub extern "C" fn surreal_init_runtime() -> c_int { 0 /* TODO: start tokio runtime */ }

    #[no_mangle]
    pub extern "C" fn surreal_connect(url: *const c_char, ns: *const c_char, db: *const c_char, user: *const c_char, pass: *const c_char) -> *mut SurHandle {
        let _ = (cstr_to_str(url), cstr_to_str(ns), cstr_to_str(db), cstr_to_str(user), cstr_to_str(pass));
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
        let _ = (cstr_to_str(table_or_topic), cstr_to_str(json_payload));
        0 /* TODO: perform write */
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
        0 /* TODO: start live query */
    }

    #[no_mangle]
    pub extern "C" fn surreal_unsubscribe(handle: *mut SurHandle, _topic: *const c_char) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &*handle };
        let mut guard = match h.callback.lock() { Ok(g) => g, Err(_) => return -2 };
        *guard = None;
        0 /* TODO: stop live query */
    }

    #[no_mangle]
    pub extern "C" fn surreal_debug_emit(_handle: *mut SurHandle, _topic: *const c_char, _json: *const c_char) -> c_int { -1 }
}

pub use api::*;
