use anyhow::{anyhow, Context, Result};
use clap::Parser;
use futures_util::stream::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::sleep;
use tracing::{error, info, warn};

use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use surrealdb::value::{from_value, Value as SurValue};
use surrealdb::{Action, RecordId, Surreal};

#[derive(Debug, Clone, Parser)]
#[command(about = "SurrealDB -> FreeSWITCH commands worker", version)]
struct Opts {
    #[arg(long, env = "SURREALDB_URL", default_value = "127.0.0.1:8000")] url: String,
    #[arg(long, env = "SURREALDB_NS", default_value = "test")] ns: String,
    #[arg(long, env = "SURREALDB_DB", default_value = "test")] db: String,
    #[arg(long, env = "SURREALDB_USER")] user: Option<String>,
    #[arg(long, env = "SURREALDB_PASS")] pass: Option<String>,
    #[arg(long, env = "SURREALDB_TOKEN")] token: Option<String>,
    #[arg(long, env = "COMMANDS_TABLE", default_value = "fs_commands")] table: String,
    #[arg(long, env = "FS_CLI", default_value = "fs_cli")] fs_cli: String,
    #[arg(long, env = "POLL_MS", default_value_t = 500u64)] poll_ms: u64,
}

#[derive(Debug, Deserialize)]
struct CmdRow {
    id: RecordId,
    action: Option<String>,
    cmd: Option<String>,
    args: Option<String>,
    uuid: Option<String>,
    cause: Option<String>,
    uuid_a: Option<String>,
    uuid_b: Option<String>,
    file: Option<String>,
    legs: Option<String>,
    status: Option<String>,
}

fn normalize_ws_hostport(url: &str) -> String {
    let u = url.trim();
    for p in ["ws://", "wss://", "http://", "https://"] {
        if let Some(stripped) = u.strip_prefix(p) {
            return stripped.to_string();
        }
    }
    u.to_string()
}

async fn connect(opts: &Opts) -> Result<Surreal<Client>> {
    let hostport = normalize_ws_hostport(&opts.url);
    let db = Surreal::new::<Ws>(&hostport)
        .await
        .with_context(|| format!("ws connect failed (to {})", hostport))?;
    if let Some(token) = &opts.token {
        db.authenticate(token).await.context("token auth failed")?;
    } else {
        let u = opts.user.clone().ok_or_else(|| anyhow!("missing user"))?;
        let p = opts.pass.clone().ok_or_else(|| anyhow!("missing pass"))?;
        db.signin(Root { username: &u, password: &p })
            .await
            .context("signin failed")?;
    }
    db.use_ns(&opts.ns).use_db(&opts.db).await.context("use ns/db failed")?;
    Ok(db)
}

async fn fs_exec(fs_cli: &str, cmd: &str, args: Option<&str>) -> Result<String> {
    let arg = match args { Some(a) if !a.is_empty() => format!("{} {}", cmd, a), _ => cmd.to_string() };
    let out = Command::new(fs_cli)
        .arg("-x")
        .arg(arg)
        .output()
        .await
        .context("spawn fs_cli")?;
    if out.status.success() {
        let mut s = String::new();
        let so = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !so.is_empty() { s.push_str(&so); }
        let se = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if !se.is_empty() {
            if !s.is_empty() { s.push_str(" | "); }
            s.push_str(&se);
        }
        Ok(s)
    } else {
        let so = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let se = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let msg = if !se.is_empty() { se } else { so };
        Err(anyhow!(if msg.is_empty() { "fs_cli failed (no output)".into() } else { msg }))
    }
}

async fn claim(db: &Surreal<Client>, table: &str, key: &str) -> Result<()> {
    let sql = format!("UPDATE {}:{} SET status = 'processing', claimed_at = time::now()", table, key);
    db.query(sql).await.context("claim update failed")?;
    Ok(())
}

async fn ack(db: &Surreal<Client>, table: &str, key: &str, ok: bool, result: &str) -> Result<()> {
    let safe = result.replace('\n', " ").replace('\r', " ");
    let patch = json!({
        "status": if ok { "done" } else { "failed" },
        "processed_at": chrono::Utc::now().timestamp(),
        "result": safe,
    });
    let sql = format!("UPDATE {}:{} MERGE {}", table, key, patch);
    db.query(sql).await.context("ack update failed")?;
    Ok(())
}

async fn handle_row(opts: &Opts, db: &Surreal<Client>, row: CmdRow) -> Result<()> {
    let tb = row.id.table().to_string();
    let key: String = row.id.key().clone().try_into().map_err(|_| anyhow!("id key not string-like"))?;
    claim(db, &tb, &key).await.ok();

    let action = row.action.unwrap_or_default().to_ascii_lowercase();
    let mut ok = false;
    let mut res = String::new();
    match action.as_str() {
        "api" => {
            let cmd = row.cmd.unwrap_or_default();
            let args = row.args.filter(|s| !s.is_empty());
            if cmd.is_empty() { res = "missing cmd".into(); }
            else {
                match fs_exec(&opts.fs_cli, &cmd, args.as_deref()).await { Ok(o) => { ok = true; res = o }, Err(e) => res = e.to_string() }
            }
        }
        "originate" => {
            let args = row.args.unwrap_or_default();
            if args.is_empty() { res = "missing args".into(); }
            else { match fs_exec(&opts.fs_cli, "originate", Some(&args)).await { Ok(o) => { ok = true; res = o }, Err(e) => res = e.to_string() } }
        }
        "hangup" => {
            let uuid = row.uuid.unwrap_or_default();
            let cause = row.cause.unwrap_or_default();
            if uuid.is_empty() { res = "missing uuid".into(); }
            else { let args = if cause.is_empty() { uuid } else { format!("{} {}", uuid, cause) };
                match fs_exec(&opts.fs_cli, "uuid_kill", Some(&args)).await { Ok(o) => { ok = true; res = o }, Err(e) => res = e.to_string() } }
        }
        "bridge" => {
            let a = row.uuid_a.unwrap_or_default();
            let b = row.uuid_b.unwrap_or_default();
            if a.is_empty() || b.is_empty() { res = "missing uuid_a/uuid_b".into(); }
            else { let args = format!("{} {}", a, b);
                match fs_exec(&opts.fs_cli, "uuid_bridge", Some(&args)).await { Ok(o) => { ok = true; res = o }, Err(e) => res = e.to_string() } }
        }
        "playback" => {
            let uuid = row.uuid.unwrap_or_default();
            let file = row.file.unwrap_or_default();
            let legs = row.legs.unwrap_or_default();
            if uuid.is_empty() || file.is_empty() { res = "missing uuid/file".into(); }
            else { let args = if legs.is_empty() { format!("{} {}", uuid, file) } else { format!("{} {} {}", uuid, file, legs) };
                match fs_exec(&opts.fs_cli, "uuid_broadcast", Some(&args)).await { Ok(o) => { ok = true; res = o }, Err(e) => res = e.to_string() } }
        }
        _ => res = format!("unknown action: {}", action),
    }

    ack(db, &tb, &key, ok, &res).await.ok();
    if ok {
        info!(url = %opts.url, ns = %opts.ns, db = %opts.db, table = %opts.table, "done: {}", res);
    } else {
        warn!(table = %tb, key = %key, "failed: {}", res);
    }
    Ok(())
}

async fn live_loop(opts: &Opts, db: &Surreal<Client>) -> Result<()> {
    info!(table = %opts.table, "starting LIVE feed");
    let mut stream = db
        .select::<Vec<CmdRow>>(&opts.table)
        .live()
        .await
        .context("live start failed")?;
    while let Some(item) = stream.next().await {
        let notif: surrealdb::Notification<CmdRow> = match item { Ok(n) => n, Err(e) => { warn!("live notif error: {}", e); continue; } };
        if notif.action != Action::Create && notif.action != Action::Update { continue; }
        let row: CmdRow = notif.data;
        if !matches!(row.status.as_deref(), Some(s) if s.eq_ignore_ascii_case("new")) { continue; }
        handle_row(opts, db, row).await.ok();
    }
    Err(anyhow!("live stream ended"))
}

async fn poll_loop(opts: &Opts, db: &Surreal<Client>) -> Result<()> {
    info!(table = %opts.table, every_ms = %opts.poll_ms, "starting POLL loop");
    loop {
        let sql = format!(
            "SELECT *, type::string(id) AS id FROM {} WHERE status = 'new' LIMIT 50",
            opts.table
        );
        match db.query(sql).await {
            Ok(mut resp) => match resp.take::<Vec<SurValue>>(0) {
                Ok(list) => {
                    if !list.is_empty() { info!(count=list.len(), table = %opts.table, "fetched new rows"); }
                    for v in list {
                        if let Ok(row) = from_value::<CmdRow>(v) { handle_row(opts, db, row).await.ok(); }
                    }
                }
                Err(e) => warn!("decode failed: {}", e),
            },
            Err(e) => warn!("poll query error: {}", e),
        }
        sleep(Duration::from_millis(opts.poll_ms)).await;
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let opts = Opts::parse();
    println!("surreal_commands_worker starting: url={} ns={} db={} table={}", opts.url, opts.ns, opts.db, opts.table);
    info!(url = %opts.url, ns = %opts.ns, db = %opts.db, table = %opts.table, "connecting");
    let db = loop {
        match connect(&opts).await {
            Ok(db) => break db,
            Err(e) => { warn!("connect failed: {}", e); sleep(Duration::from_secs(1)).await; }
        }
    };

    // Try LIVE, fallback to POLL
    match live_loop(&opts, &db).await {
        Ok(()) => {}
        Err(e) => { warn!("live failed: {} — falling back to poll", e); poll_loop(&opts, &db).await?; }
    }

    Ok(())
}
