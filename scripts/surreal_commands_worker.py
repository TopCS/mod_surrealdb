#!/usr/bin/env python3
"""
Simple SurrealDB -> FreeSWITCH commands worker.

- Polls fs_commands for rows with status='new'
- Claims each row (status='processing') to avoid duplicates
- Executes the action by shelling fs_cli -x ...
- Writes result/status back to SurrealDB

Configuration via env vars:
  SURREALDB_HTTP   e.g. http://127.0.0.1:8000
  SURREALDB_NS     e.g. test
  SURREALDB_DB     e.g. test
  SURREALDB_USER   (optional; used if no token)
  SURREALDB_PASS   (optional; used if no token)
  SURREALDB_TOKEN  (optional; takes precedence over user/pass)
  COMMANDS_TABLE   default: fs_commands
  POLL_MS          default: 500
  FS_CLI           default: fs_cli

Notes:
- Requires SurrealDB HTTP API enabled.
- No external Python deps; uses urllib and subprocess.
- Uses SQL queries with projection: "*, type::string(id) AS id" to make id a plain string.
"""

import json
import os
import sys
import time
import subprocess
from urllib import request, parse, error


HTTP = os.environ.get("SURREALDB_HTTP", "http://127.0.0.1:8000").rstrip("/")
NS = os.environ.get("SURREALDB_NS", "test")
DB = os.environ.get("SURREALDB_DB", "test")
USER = os.environ.get("SURREALDB_USER")
PASS = os.environ.get("SURREALDB_PASS")
TOKEN = os.environ.get("SURREALDB_TOKEN")
TABLE = os.environ.get("COMMANDS_TABLE", "fs_commands")
POLL_MS = int(os.environ.get("POLL_MS", "500"))
FS_CLI = os.environ.get("FS_CLI", "fs_cli")


def log(msg: str) -> None:
    ts = time.strftime("%Y-%m-%d %H:%M:%S")
    print(f"[{ts}] worker: {msg}", flush=True)


def http_post_json(path: str, body: dict | str, headers: dict) -> tuple[int, str]:
    url = f"{HTTP}{path}"
    data = body if isinstance(body, (bytes, bytearray)) else json.dumps(body).encode("utf-8")
    req = request.Request(url=url, data=data, method="POST")
    for k, v in headers.items():
        req.add_header(k, v)
    try:
        with request.urlopen(req, timeout=10) as resp:
            return resp.getcode(), resp.read().decode("utf-8", "replace")
    except error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")
    except Exception as e:
        return 0, str(e)


def get_token() -> str | None:
    global TOKEN
    if TOKEN:
        return TOKEN
    if not (USER and PASS):
        return None
    code, txt = http_post_json("/signin", {"user": USER, "pass": PASS}, {})
    if code != 200:
        log(f"signin failed: {code} {txt}")
        return None
    try:
        obj = json.loads(txt)
        tok = obj.get("token") or obj.get("Token")
        TOKEN = tok
        return tok
    except Exception:
        log(f"invalid signin response: {txt[:200]}")
        return None


def sql(query: str) -> list:
    headers = {"NS": NS, "DB": DB, "Content-Type": "application/json"}
    tok = get_token()
    if tok:
        headers["Authorization"] = f"Bearer {tok}"
    code, txt = http_post_json("/sql", query, headers)
    if code != 200:
        log(f"sql error {code}: {txt[:200]}")
        return []
    try:
        # Expect SurrealDB HTTP SQL array of results; take first result set
        arr = json.loads(txt)
        # Newer servers return array of objects with result
        rows = []
        for item in arr:
            res = item.get("result") if isinstance(item, dict) else item
            if isinstance(res, list):
                rows.extend(res)
        return rows
    except Exception:
        log(f"sql decode failed: {txt[:200]}")
        return []


def fs_exec(cmd: str, args: str | None) -> tuple[bool, str]:
    arg = f"{cmd} {args}".strip()
    try:
        out = subprocess.check_output([FS_CLI, "-x", arg], stderr=subprocess.STDOUT, timeout=20)
        return True, out.decode("utf-8", "replace").strip()
    except subprocess.CalledProcessError as e:
        return False, e.output.decode("utf-8", "replace").strip()
    except Exception as e:
        return False, str(e)


def claim(id_key: str) -> bool:
    q = f"UPDATE {TABLE}:{id_key} SET status='processing', claimed_at=time::now()"
    rows = sql(q)
    return True if rows is not None else False


def ack(id_key: str, ok: bool, result: str) -> None:
    safe = result.replace("\n", " ").replace("\r", " ")
    patch = json.dumps({
        "status": "done" if ok else "failed",
        "processed_at": int(time.time()),
        "result": safe[:1000]
    })
    q = f"UPDATE {TABLE}:{id_key} MERGE {patch}"
    _ = sql(q)


def handle(row: dict) -> None:
    # Expect id as "table:key" from projection; extract key
    rid = str(row.get("id", ""))
    key = rid.split(":", 1)[1] if ":" in rid else rid
    if not key:
        log(f"skip row without string id: {row}")
        return
    if not claim(key):
        log(f"claim failed for {TABLE}:{key}")
        return

    action = (row.get("action") or "").lower()
    ok = False
    result = ""
    try:
        if action == "api":
            cmd = str(row.get("cmd") or "").strip()
            args = str(row.get("args") or "").strip() or None
            if not cmd:
                ok, result = False, "missing cmd"
            else:
                ok, result = fs_exec(cmd, args)
        elif action == "originate":
            args = str(row.get("args") or "").strip()
            if not args:
                ok, result = False, "missing args"
            else:
                ok, result = fs_exec("originate", args)
        elif action == "hangup":
            uuid = str(row.get("uuid") or "").strip()
            cause = str(row.get("cause") or "").strip()
            args = f"{uuid} {cause}".strip()
            if not uuid:
                ok, result = False, "missing uuid"
            else:
                ok, result = fs_exec("uuid_kill", args)
        elif action == "bridge":
            a = str(row.get("uuid_a") or "").strip()
            b = str(row.get("uuid_b") or "").strip()
            if not (a and b):
                ok, result = False, "missing uuid_a/uuid_b"
            else:
                ok, result = fs_exec("uuid_bridge", f"{a} {b}")
        elif action == "playback":
            uuid = str(row.get("uuid") or "").strip()
            file = str(row.get("file") or "").strip()
            legs = str(row.get("legs") or "").strip()
            if not (uuid and file):
                ok, result = False, "missing uuid/file"
            else:
                args = f"{uuid} {file} {legs}".strip()
                ok, result = fs_exec("uuid_broadcast", args)
        else:
            ok, result = False, f"unknown action: {action}"
    except Exception as e:
        ok, result = False, f"exception: {e}"

    ack(key, ok, result)
    log(f"processed {TABLE}:{key} -> {'OK' if ok else 'ERR'}: {result[:120]}")


def main() -> int:
    log(f"starting; table={TABLE} ns={NS} db={DB} http={HTTP}")
    # Sanity: test connectivity
    test = sql("SELECT 1")
    if not isinstance(test, list):
        log("warning: connectivity test failed")

    while True:
        try:
            # Project id to string and fetch only 'new'
            q = f"SELECT *, type::string(id) AS id FROM {TABLE} WHERE status = 'new' LIMIT 50"
            rows = sql(q)
            if rows:
                log(f"fetched {len(rows)} new rows")
                for row in rows:
                    handle(row)
        except KeyboardInterrupt:
            log("stopping (SIGINT)")
            return 0
        except Exception as e:
            log(f"loop error: {e}")
            time.sleep(1.0)
        time.sleep(POLL_MS / 1000.0)


if __name__ == "__main__":
    sys.exit(main())

