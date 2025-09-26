//! SurrealDB FFI shim for C callers (FreeSWITCH module).
//! Default build is a stub for offline development; enable `real` feature for client wiring.

#[cfg(feature = "stub")]
mod api {
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int, c_void};
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::net::TcpStream;
    use std::time::Duration;

    pub type SurCommandCb = Option<extern "C" fn(topic: *const c_char, json: *const c_char, user_data: *mut c_void)>;
    pub type SurLogCb = Option<extern "C" fn(msg: *const c_char, user_data: *mut c_void)>;

    #[repr(C)]
    pub struct SurHandle {
        pub(crate) is_connected: bool,
        pub(crate) last_error_code: AtomicI32,
        pub(crate) callback: Mutex<Option<(SurCommandCb, *mut c_void)>>,
        pub(crate) last_error_msg: Mutex<String>,
    }

    static LOGGER: OnceLock<Mutex<Option<(SurLogCb, usize)>>> = OnceLock::new();

    fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
        if ptr.is_null() { return None; }
        unsafe { CStr::from_ptr(ptr) }.to_str().ok()
    }

    fn parse_host_port(url: &str) -> Option<(String, u16)> {
        let mut scheme = "";
        let mut rest = url;
        if let Some(idx) = url.find("://") {
            scheme = &url[..idx];
            rest = &url[idx+3..];
        }
        let hostport = rest.split('/').next().unwrap_or(rest);
        if hostport.is_empty() { return None; }
        // IPv6 in brackets
        if hostport.starts_with('[') {
            let end = hostport.find(']')?;
            let host = &hostport[1..end];
            let port = if let Some(col) = hostport[end+1..].find(':') {
                hostport[end+2+col..].parse::<u16>().ok()
            } else { None };
            let default_port = match scheme { "wss"|"https" => 443, "ws"|"http" => 80, _ => 8000 };
            return Some((host.to_string(), port.unwrap_or(default_port)));
        }
        // host[:port]
        let mut parts = hostport.splitn(2, ':');
        let host = parts.next().unwrap_or("");
        if host.is_empty() { return None; }
        let port = parts.next().and_then(|p| p.parse::<u16>().ok()).unwrap_or_else(|| match scheme { "wss"|"https" => 443, "ws"|"http" => 80, _ => 8000 });
        Some((host.to_string(), port))
    }

    fn tcp_probe(url: &str) -> bool {
        if let Some((host, port)) = parse_host_port(url) {
            let addr = format!("{}:{}", host, port);
            let to = Duration::from_millis(800);
            match addr.parse() {
                Ok(sockaddr) => TcpStream::connect_timeout(&sockaddr, to).is_ok(),
                Err(_) => false,
            }
        } else {
            false
        }
    }

    #[no_mangle]
    pub extern "C" fn surreal_init_runtime() -> c_int { 0 }

    #[no_mangle]
    #[no_mangle]
    #[no_mangle]
    pub extern "C" fn surreal_connect(url: *const c_char, ns: *const c_char, db: *const c_char, user: *const c_char, pass: *const c_char) -> *mut SurHandle {
        let url = match cstr_to_str(url) { Some(s) => s, None => return std::ptr::null_mut() };
        let _ = (cstr_to_str(ns), cstr_to_str(db), cstr_to_str(user), cstr_to_str(pass));
        if !tcp_probe(url) {
            return std::ptr::null_mut();
        }
        let handle = Box::new(SurHandle { is_connected: true, last_error_code: AtomicI32::new(0), callback: Mutex::new(None), last_error_msg: Mutex::new(String::new()) });
        Box::into_raw(handle)
    }

    #[no_mangle]
    pub extern "C" fn surreal_connect_with_token(url: *const c_char, ns: *const c_char, db: *const c_char, token: *const c_char) -> *mut SurHandle {
        let url = match cstr_to_str(url) { Some(s) => s, None => return std::ptr::null_mut() };
        let _ = (cstr_to_str(ns), cstr_to_str(db), cstr_to_str(token));
        if !tcp_probe(url) {
            return std::ptr::null_mut();
        }
        let handle = Box::new(SurHandle { is_connected: true, last_error_code: AtomicI32::new(0), callback: Mutex::new(None), last_error_msg: Mutex::new(String::new()) });
        Box::into_raw(handle)
    }

    #[no_mangle]
    pub extern "C" fn surreal_publish(handle: *mut SurHandle, table_or_topic: *const c_char, json_payload: *const c_char) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &*handle };
        if !h.is_connected { return -2; }
        let _ = (cstr_to_str(table_or_topic), cstr_to_str(json_payload));
        if let Ok(mut msg) = h.last_error_msg.lock() { msg.clear(); }
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

    #[no_mangle]
    pub extern "C" fn surreal_last_error_text(handle: *mut SurHandle, buf: *mut c_char, len: u32) -> c_int {
        if handle.is_null() || buf.is_null() || len == 0 { return -1; }
        let h = unsafe { &*handle };
        let s = match h.last_error_msg.lock() { Ok(g) => g.clone(), Err(_) => String::new() };
        let bytes = s.as_bytes();
        let n = (bytes.len().min((len - 1) as usize)) as usize;
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, n) };
        unsafe { *(buf.wrapping_add(n as usize)) = 0 }; // NUL terminate
        n as c_int
    }

    #[no_mangle]
    pub extern "C" fn surreal_is_stub() -> c_int { 1 }

    #[no_mangle]
    pub extern "C" fn surreal_set_logger(cb: SurLogCb, user_data: *mut c_void) -> c_int {
        let m = LOGGER.get_or_init(|| Mutex::new(None));
        match m.lock() {
            Ok(mut g) => { *g = Some((cb, user_data as usize)); 0 }
            Err(_) => -1,
        }
    }

    #[no_mangle]
    pub extern "C" fn surreal_select(handle: *mut SurHandle, table: *const c_char, limit: u32, out_json: *mut c_char, out_len: u32) -> c_int {
        if handle.is_null() || out_json.is_null() || out_len == 0 { return -1; }
        let _ = (cstr_to_str(table), limit);
        let empty = b"[]";
        let n = (empty.len().min((out_len - 1) as usize)) as usize;
        unsafe { std::ptr::copy_nonoverlapping(empty.as_ptr(), out_json as *mut u8, n) };
        unsafe { *(out_json.wrapping_add(n as usize)) = 0 };
        0
    }

    #[no_mangle]
    pub extern "C" fn surreal_get(handle: *mut SurHandle, _table: *const c_char, _id: *const c_char, out_json: *mut c_char, out_len: u32) -> c_int {
        if handle.is_null() || out_json.is_null() || out_len == 0 { return -1; }
        let empty = b"null";
        let n = (empty.len().min((out_len - 1) as usize)) as usize;
        unsafe { std::ptr::copy_nonoverlapping(empty.as_ptr(), out_json as *mut u8, n) };
        unsafe { *(out_json.wrapping_add(n as usize)) = 0 };
        0
    }

    #[no_mangle]
    pub extern "C" fn surreal_update(_handle: *mut SurHandle, _table: *const c_char, _id: *const c_char, json_patch: *const c_char) -> c_int {
        if json_patch.is_null() { return -4; }
        // Succeed in stub mode to keep flows working
        0
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
    use tokio::task::JoinHandle;
    use tokio::time::{sleep, Duration};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use surrealdb::Surreal;
    use surrealdb::engine::remote::ws::{Client, Ws};
    use surrealdb::opt::auth::Root;

    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    static LAST_ERR: AtomicI32 = AtomicI32::new(0);
    static LOGGER: OnceLock<Mutex<Option<(SurLogCb, usize)>>> = OnceLock::new();

    fn trace_enabled() -> bool {
        match std::env::var("SURREALDB_FFI_TRACE") {
            Ok(v) => {
                let v = v.trim().to_ascii_lowercase();
                !(v == "0" || v == "false" || v == "off")
            }
            Err(_) => true, // default ON
        }
    }

    fn set_err(code: i32) {
        LAST_ERR.store(code, Ordering::Relaxed);
        if trace_enabled() {
            eprintln!("[surrealdb_ffi] error code {}", code);
        }
    }

    pub type SurCommandCb = Option<extern "C" fn(topic: *const c_char, json: *const c_char, user_data: *mut c_void)>;
    pub type SurLogCb = Option<extern "C" fn(msg: *const c_char, user_data: *mut c_void)>;

    enum Auth {
        UserPass { user: String, pass: String },
        Token(String),
    }

    #[repr(C)]
    pub struct SurHandle {
        last_error_code: AtomicI32,
        callback: Mutex<Option<(SurCommandCb, *mut c_void)>>,
        client: Option<Surreal<Client>>,
        url: String,
        ns: String,
        db: String,
        auth: Auth,
        subs: Mutex<HashMap<String, Sub>>, // table -> subscription
        last_error_msg: Mutex<String>,
    }

    struct Sub {
        stop: Arc<AtomicBool>,
        handle: JoinHandle<()>,
        cb: SurCommandCb,
        user: *mut c_void,
    }

    fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> { if ptr.is_null() { return None; } unsafe { CStr::from_ptr(ptr) }.to_str().ok() }

    #[no_mangle]
    pub extern "C" fn surreal_init_runtime() -> c_int {
        if RUNTIME.get().is_some() { return 0; }
        match Runtime::new() {
            Ok(rt) => { let _ = RUNTIME.set(rt); LAST_ERR.store(0, Ordering::Relaxed); 0 }
            Err(_) => { set_err(-100); -1 }
        }
    }

    fn log_info(msg: &str) {
        let (cb_opt, user_usize) = {
            let m = LOGGER.get_or_init(|| Mutex::new(None));
            match m.lock() {
                Ok(g) => g.clone().unwrap_or((None, 0usize)),
                Err(_) => (None, 0usize),
            }
        };
        if let Some(cb) = cb_opt {
            if let Ok(cmsg) = std::ffi::CString::new(msg) { cb(cmsg.as_ptr(), user_usize as *mut c_void); }
        } else if trace_enabled() {
            eprintln!("[surrealdb_ffi] {}", msg);
        }
    }

    fn normalize_ws_url(url: &str) -> String {
        let u = url.trim();
        for p in ["ws://", "wss://", "http://", "https://"] {
            if let Some(stripped) = u.strip_prefix(p) { return stripped.to_string(); }
        }
        u.to_string()
    }

    fn open_client(url: &str, ns: &str, db: &str, auth: &Auth) -> Result<Surreal<Client>, ()> {
        let url = normalize_ws_url(url);
        let rt = match RUNTIME.get() { Some(rt) => rt, None => { set_err(-101); return Err(()); } };
        rt.block_on(async move {
            let dbh = match Surreal::new::<Ws>(&url).await {
                Ok(v) => v,
                Err(_) => { set_err(-102); return Err(()); }
            };
            match auth {
                Auth::UserPass { user, pass } => {
                    if dbh.signin(Root { username: &user, password: &pass }).await.is_err() { set_err(-103); return Err(()); }
                }
                Auth::Token(token) => {
                    if dbh.authenticate(token).await.is_err() { set_err(-104); return Err(()); }
                }
            }
            if dbh.use_ns(ns).use_db(db).await.is_err() { set_err(-105); return Err(()); }
            Ok::<_, ()>(dbh)
        })
    }

    #[no_mangle]
    pub extern "C" fn surreal_connect(url: *const c_char, ns: *const c_char, db: *const c_char, user: *const c_char, pass: *const c_char) -> *mut SurHandle {
        let url_s = match cstr_to_str(url) { Some(s) => s.to_string(), None => return std::ptr::null_mut() };
        let ns_s = match cstr_to_str(ns) { Some(s) => s.to_string(), None => return std::ptr::null_mut() };
        let db_s = match cstr_to_str(db) { Some(s) => s.to_string(), None => return std::ptr::null_mut() };
        let user_s = match cstr_to_str(user) { Some(s) => s.to_string(), None => return std::ptr::null_mut() };
        let pass_s = match cstr_to_str(pass) { Some(s) => s.to_string(), None => return std::ptr::null_mut() };

        let auth = Auth::UserPass { user: user_s, pass: pass_s };
        match open_client(&url_s, &ns_s, &db_s, &auth) {
            Ok(client) => {
                let handle = Box::new(SurHandle {
                    last_error_code: AtomicI32::new(0),
                    callback: Mutex::new(None),
                    client: Some(client),
                    url: url_s,
                    ns: ns_s,
                    db: db_s,
                    auth,
                    subs: Mutex::new(HashMap::new()),
                    last_error_msg: Mutex::new(String::new()),
                });
                LAST_ERR.store(0, Ordering::Relaxed);
                Box::into_raw(handle)
            }
            Err(_) => std::ptr::null_mut(),
        }
    }

    #[no_mangle]
    pub extern "C" fn surreal_connect_with_token(url: *const c_char, ns: *const c_char, db: *const c_char, token: *const c_char) -> *mut SurHandle {
        let url_s = match cstr_to_str(url) { Some(s) => s.to_string(), None => return std::ptr::null_mut() };
        let ns_s = match cstr_to_str(ns) { Some(s) => s.to_string(), None => return std::ptr::null_mut() };
        let db_s = match cstr_to_str(db) { Some(s) => s.to_string(), None => return std::ptr::null_mut() };
        let token_s = match cstr_to_str(token) { Some(s) => s.to_string(), None => return std::ptr::null_mut() };

        let auth = Auth::Token(token_s);
        match open_client(&url_s, &ns_s, &db_s, &auth) {
            Ok(client) => {
                let handle = Box::new(SurHandle {
                    last_error_code: AtomicI32::new(0),
                    callback: Mutex::new(None),
                    client: Some(client),
                    url: url_s,
                    ns: ns_s,
                    db: db_s,
                    auth,
                    subs: Mutex::new(HashMap::new()),
                    last_error_msg: Mutex::new(String::new()),
                });
                LAST_ERR.store(0, Ordering::Relaxed);
                Box::into_raw(handle)
            }
            Err(_) => std::ptr::null_mut(),
        }
    }

    #[no_mangle]
    pub extern "C" fn surreal_publish(handle: *mut SurHandle, table_or_topic: *const c_char, json_payload: *const c_char) -> c_int {
        if handle.is_null() { return -1; }
        let table = match cstr_to_str(table_or_topic) { Some(s) => s, None => return -3 };
        let json = match cstr_to_str(json_payload) { Some(s) => s, None => return -4 };

        let value: JsonValue = match serde_json::from_str(json) { Ok(v) => v, Err(_) => return -5 };

        let h = unsafe { &mut *handle };
        if let Ok(mut m) = h.last_error_msg.lock() { m.clear(); }
        let client = match h.client.as_ref() {
            Some(c) => c,
            None => {
                // Try to reconnect lazily
                match open_client(&h.url, &h.ns, &h.db, &h.auth) {
                    Ok(c) => { h.client = Some(c); h.client.as_ref().unwrap() }
                    Err(_) => return -2,
                }
            }
        };

        let rt = match RUNTIME.get() { Some(rt) => rt, None => return -1 };

        let res: Result<(), String> = rt.block_on(async {
            // Safer path: use SQL with validated table identifier and canonical JSON string
            // Validate table name to be conservative
            if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' ) { return Err("invalid table identifier".to_string()); }
            let json_str = match serde_json::to_string(&value) { Ok(s) => s, Err(e) => return Err(format!("json stringify failed: {}", e)) };
            let sql = format!("CREATE {} CONTENT {}", table, json_str);
            match client.query(sql).await { Ok(_) => Ok(()), Err(e) => Err(format!("{}", e)) }
        });

        match res {
            Ok(()) => 0,
            Err(e) => {
                if let Ok(mut msg) = h.last_error_msg.lock() { *msg = e; }
                h.last_error_code.store(-6, Ordering::Relaxed);
                h.client = None;
                -6
            }
        }
    }

    #[no_mangle]
    pub extern "C" fn surreal_close(handle: *mut SurHandle) { if handle.is_null() { return; } let _ = unsafe { Box::from_raw(handle) }; }

    #[no_mangle]
    pub extern "C" fn surreal_last_error_code(handle: *mut SurHandle) -> c_int { if handle.is_null() { return -1; } let h = unsafe { &*handle }; h.last_error_code.load(Ordering::Relaxed) }

    #[no_mangle]
    pub extern "C" fn surreal_last_error_global() -> c_int { LAST_ERR.load(Ordering::Relaxed) }

    #[no_mangle]
    pub extern "C" fn surreal_last_error_text(handle: *mut SurHandle, buf: *mut c_char, len: u32) -> c_int {
        if handle.is_null() || buf.is_null() || len == 0 { return -1; }
        let h = unsafe { &*handle };
        let s = match h.last_error_msg.lock() { Ok(g) => g.clone(), Err(_) => String::new() };
        let bytes = s.as_bytes();
        let n = (bytes.len().min((len - 1) as usize)) as usize;
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, n) };
        unsafe { *(buf.wrapping_add(n as usize)) = 0 };
        n as c_int
    }

    #[no_mangle]
    pub extern "C" fn surreal_is_stub() -> c_int { 0 }

    #[no_mangle]
    pub extern "C" fn surreal_set_logger(cb: SurLogCb, user_data: *mut c_void) -> c_int {
        let m = LOGGER.get_or_init(|| Mutex::new(None));
        match m.lock() {
            Ok(mut g) => { *g = Some((cb, user_data as usize)); 0 }
            Err(_) => -1,
        }
    }

    #[no_mangle]
    pub extern "C" fn surreal_subscribe(handle: *mut SurHandle, _topic: *const c_char, cb: SurCommandCb, user_data: *mut c_void) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &*handle };
        let table = match cstr_to_str(_topic) { Some(s) => s.to_string(), None => return -3 };
        log_info(&format!("subscribe started on {}", table));

        // Resolve callback and user_data
        let mut cb_to_use: SurCommandCb = cb;
        let mut user_to_use: *mut c_void = user_data;
        if cb_to_use.is_none() {
            if let Ok(guard) = h.callback.lock() {
                if let Some((stored_cb, stored_user)) = *guard {
                    cb_to_use = stored_cb;
                    user_to_use = stored_user;
                }
            }
        }

        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let url = h.url.clone();
        let ns = h.ns.clone();
        let db = h.db.clone();
        let auth = match &h.auth { Auth::UserPass { user, pass } => Auth::UserPass { user: user.clone(), pass: pass.clone() }, Auth::Token(t) => Auth::Token(t.clone()) };

        // Use integer for user_data to satisfy Send in tokio::spawn
        let user_ptr_usize = user_to_use as usize;
        let table_stream = table.clone();
        let join = RUNTIME.get().unwrap().spawn(async move {
            use futures::StreamExt;
            use surrealdb::Action;
            use surrealdb::RecordId;
            use surrealdb::value::{from_value as from_sur_value, Value as SurValue};
            use std::convert::TryInto;

            log_info(&format!("subscribe loop starting on {} (LIVE)", table_stream));
            let mut client = open_client(&url, &ns, &db, &auth).ok();
            'outer: loop {
                if stop_clone.load(std::sync::atomic::Ordering::Relaxed) { break; }
                if client.is_none() {
                    client = open_client(&url, &ns, &db, &auth).ok();
                    if client.is_none() { log_info(&format!("live connect failed on {}; retrying", table_stream)); sleep(Duration::from_millis(1000)).await; continue; }
                }
                let dbh = client.as_ref().unwrap();
                log_info(&format!("live connected on {}", table_stream));
                // Start LIVE SELECT stream
                let mut stream = match dbh.select(&table_stream).live().await {
                    Ok(s) => s,
                    Err(e) => { log_info(&format!("live start failed on {}: {}", table_stream, e)); client = None; sleep(Duration::from_millis(1000)).await; continue; }
                };

                while let Some(item) = stream.next().await {
                    let notif: surrealdb::Notification<SurValue> = match item {
                        Ok(n) => n,
                        Err(e) => { log_info(&format!("live notification error on {}: {}", table_stream, e)); continue; }
                    };
                    if stop_clone.load(std::sync::atomic::Ordering::Relaxed) { break 'outer; }
                    // Only react to create/update; delete is irrelevant for commands
                    if notif.action != Action::Create && notif.action != Action::Update { continue; }

                    // Serialize to JSON for callback shaping
                    let sur_val: SurValue = notif.data.clone();
                    let mut json = match serde_json::to_value(&sur_val) { Ok(v) => v, Err(_) => serde_json::json!({}) };

                    // Extract id + status using typed conversion for reliability
                    #[derive(serde::Deserialize)]
                    struct IdOnly { id: RecordId, status: Option<String> }
                    let idonly: IdOnly = match from_sur_value::<IdOnly>(sur_val.clone()) {
                        Ok(v) => v,
                        Err(e) => { log_info(&format!("live could not parse id/status on {}: {}", table_stream, e)); continue; }
                    };
                    // gate on status == 'new'
                    if let Some(st) = idonly.status.as_deref() { if !st.eq_ignore_ascii_case("new") { continue; } } else { continue; }

                    // Build id string as table:key (bare key for claim/update API)
                    let tb = idonly.id.table().to_string();
                    let key: Result<String, _> = idonly.id.key().clone().try_into();
                    let key = match key { Ok(s) => s, Err(_) => {
                        log_info(&format!("live could not stringify id key for {}; skipping", table_stream));
                        continue;
                    }};
                    let id_str = key;
                    // Claim the record
                    let claim = format!("UPDATE {}:{} SET status = 'processing', claimed_at = time::now()", table_stream, id_str);
                    match dbh.query(claim).await {
                        Ok(_) => {
                            // Ensure id is a JSON string for the callback
                            if let Some(obj) = json.as_object_mut() { obj.insert("id".to_string(), serde_json::Value::String(format!("{}:{}", tb, id_str))); }
                            if let Ok(txt) = serde_json::to_string(&json) {
                                if let Ok(ctopic) = std::ffi::CString::new(table_stream.clone()) {
                                    if let Ok(cjson) = std::ffi::CString::new(txt) {
                                        if let Some(cb_fn) = cb_to_use {
                                            let user_ptr = user_ptr_usize as *mut c_void;
                                            cb_fn(ctopic.as_ptr(), cjson.as_ptr(), user_ptr);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => { log_info(&format!("live claim failed for {}:{}: {}", table_stream, id_str, e)); }
                    }
                }
                // stream ended; reconnect
                client = None;
                log_info(&format!("live stream ended on {}; reconnecting", table_stream));
                sleep(Duration::from_millis(500)).await;
            }
        });

        let mut subs = match h.subs.lock() { Ok(m) => m, Err(_) => return -4 };
        subs.insert(table, Sub { stop, handle: join, cb, user: user_data });
        0
    }

    #[no_mangle]
    pub extern "C" fn surreal_unsubscribe(handle: *mut SurHandle, _topic: *const c_char) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &*handle };
        let table = match cstr_to_str(_topic) { Some(s) => s, None => return -3 };
        let mut subs = match h.subs.lock() { Ok(m) => m, Err(_) => return -2 };
        if let Some(sub) = subs.remove(&*table) {
            sub.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            sub.handle.abort();
            return 0;
        }
        -4
    }

    #[no_mangle]
    pub extern "C" fn surreal_update(handle: *mut SurHandle, table: *const c_char, id: *const c_char, json_patch: *const c_char) -> c_int {
        if handle.is_null() { return -1; }
        let h = unsafe { &mut *handle };
        let table = match cstr_to_str(table) { Some(s) => s, None => return -2 };
        let mut id = match cstr_to_str(id) { Some(s) => s.to_string(), None => return -3 };
        // Normalize id: allow fully qualified "table:id" or bare id
        if let Some(stripped) = id.strip_prefix(&format!("{}:", table)) { id = stripped.to_string(); }
        if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' ) { return -2; }
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' ) { return -3; }
        let patch = match cstr_to_str(json_patch) { Some(s) => s, None => return -4 };
        let value: JsonValue = match serde_json::from_str(patch) { Ok(v) => v, Err(_) => return -5 };
        let client = match h.client.as_ref() {
            Some(c) => c,
            None => {
                match open_client(&h.url, &h.ns, &h.db, &h.auth) {
                    Ok(c) => { h.client = Some(c); h.client.as_ref().unwrap() }
                    Err(_) => return -6,
                }
            }
        };
        let rt = match RUNTIME.get() { Some(rt) => rt, None => return -7 };
        // Use SQL to avoid method-shape mismatches and capture error text
        let res: Result<(), String> = rt.block_on(async {
            let json_str = match serde_json::to_string(&value) { Ok(s) => s, Err(e) => return Err(format!("json stringify failed: {}", e)) };
            let sql = format!("UPDATE {}:{} MERGE {}", table, id, json_str);
            match client.query(sql).await {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("{}", e)),
            }
        });
        match res {
            Ok(()) => 0,
            Err(e) => {
                if let Ok(mut msg) = h.last_error_msg.lock() { *msg = e; }
                h.last_error_code.store(-8, Ordering::Relaxed);
                -8
            }
        }
    }

    #[no_mangle]
    pub extern "C" fn surreal_debug_emit(_handle: *mut SurHandle, _topic: *const c_char, _json: *const c_char) -> c_int { -1 }

    #[no_mangle]
    pub extern "C" fn surreal_select(handle: *mut SurHandle, table: *const c_char, limit: u32, out_json: *mut c_char, out_len: u32) -> c_int {
        if handle.is_null() || out_json.is_null() || out_len == 0 { return -1; }
        let h = unsafe { &mut *handle };
        let table = match cstr_to_str(table) { Some(s) => s, None => return -2 };
        if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' ) { return -3; }
        let limit = if limit == 0 { 100 } else { limit.min(10000) };
        let client = match h.client.as_ref() {
            Some(c) => c,
            None => {
                match open_client(&h.url, &h.ns, &h.db, &h.auth) {
                    Ok(c) => { h.client = Some(c); h.client.as_ref().unwrap() }
                    Err(_) => return -4,
                }
            }
        };
        let rt = match RUNTIME.get() { Some(rt) => rt, None => return -5 };
        // Project string-cast id last to override typed id in output JSON
        let sql = format!("SELECT *, type::string(id) AS id FROM {} LIMIT {}", table, limit);
        let res: Result<String, String> = rt.block_on(async {
            match client.query(sql).await {
                Ok(mut resp) => {
                    match resp.take::<Vec<serde_json::Value>>(0) {
                        Ok(list) => serde_json::to_string(&list).map_err(|e| format!("json encode failed: {}", e)),
                        Err(e) => Err(format!("decode failed: {}", e)),
                    }
                }
                Err(e) => Err(format!("{}", e))
            }
        });
        match res {
            Ok(s) => {
                let bytes = s.as_bytes();
                let n = (bytes.len().min((out_len - 1) as usize)) as usize;
                unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_json as *mut u8, n) };
                unsafe { *(out_json.wrapping_add(n as usize)) = 0 };
                0
            }
            Err(e) => {
                if let Ok(mut msg) = h.last_error_msg.lock() { *msg = e; }
                h.last_error_code.store(-9, Ordering::Relaxed);
                -6
            }
        }
    }

    #[no_mangle]
    pub extern "C" fn surreal_get(handle: *mut SurHandle, table: *const c_char, id: *const c_char, out_json: *mut c_char, out_len: u32) -> c_int {
        if handle.is_null() || out_json.is_null() || out_len == 0 { return -1; }
        let h = unsafe { &mut *handle };
        let table = match cstr_to_str(table) { Some(s) => s, None => return -2 };
        let mut id = match cstr_to_str(id) { Some(s) => s.to_string(), None => return -3 };
        if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' ) { return -4; }
        if let Some(stripped) = id.strip_prefix(&format!("{}:", table)) { id = stripped.to_string(); }
        let client = match h.client.as_ref() {
            Some(c) => c,
            None => {
                match open_client(&h.url, &h.ns, &h.db, &h.auth) {
                    Ok(c) => { h.client = Some(c); h.client.as_ref().unwrap() }
                    Err(_) => return -5,
                }
            }
        };
        let rt = match RUNTIME.get() { Some(rt) => rt, None => return -6 };
        let res: Result<String, String> = rt.block_on(async {
            let sql = format!("SELECT *, type::string(id) AS id FROM {} WHERE id = {}:{} LIMIT 1", table, table, id);
            match client.query(sql).await {
                Ok(mut resp) => {
                    match resp.take::<Vec<serde_json::Value>>(0) {
                        Ok(mut list) => {
                            if let Some(v) = list.pop() { serde_json::to_string(&v).map_err(|e| format!("json encode failed: {}", e)) } else { Ok("null".to_string()) }
                        }
                        Err(e) => Err(format!("decode failed: {}", e)),
                    }
                }
                Err(e) => Err(format!("{}", e))
            }
        });
        match res {
            Ok(s) => {
                let bytes = s.as_bytes();
                let n = (bytes.len().min((out_len - 1) as usize)) as usize;
                unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_json as *mut u8, n) };
                unsafe { *(out_json.wrapping_add(n as usize)) = 0 };
                0
            }
            Err(e) => {
                if let Ok(mut msg) = h.last_error_msg.lock() { *msg = e; }
                h.last_error_code.store(-10, Ordering::Relaxed);
                -6
            }
        }
    }
}

pub use api::*;
