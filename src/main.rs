//! lab — the minilab CLI. The ONE thing that reads/writes Supabase.
//!
//! Bootstrap kernel, by design:
//!   - reads/writes Supabase (PostgREST over curl -- "supabase is everywhere")
//!   - every write is HASHED with JCS-RFC8785 (audited `serde_jcs`) + sha256,
//!     so each row is referenceable by its content_hash. The hash is minted,
//!     never enforced: a wrong/absent hash still lands (register first).
//!   - `conformance` can be POINTED at any row and gives a verdict -- a WARNING,
//!     never a blockage. The LAB is Dan's; LogLine is a tool, not a warden.
//!   - `new-command` creates a new command (self-extending)
//!   - any other word runs <lab-root>/commands/<word> (auto-discovered plugins)
//!
//! $HOME-portable (no hardcoded user). Nothing else in the fleet should hold
//! Supabase creds or speak to the ledger -- everything calls `lab`.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use std::time::UNIX_EPOCH;

use serde_json::Value;
use sha2::{Digest, Sha256};

fn home() -> String {
    env::var("HOME").unwrap_or_else(|_| "/".to_string())
}

fn is_lab_cli_root(path: &Path) -> bool {
    let cargo = path.join("Cargo.toml");
    let main = path.join("src/main.rs");
    if !cargo.exists() || !main.exists() {
        return false;
    }
    fs::read_to_string(cargo)
        .map(|text| text.lines().any(|line| line.trim() == "name = \"lab\""))
        .unwrap_or(false)
}

fn first_lab_cli_root_from(mut path: PathBuf) -> Option<PathBuf> {
    if path.is_file() {
        path.pop();
    }
    for candidate in path.ancestors() {
        if is_lab_cli_root(candidate) {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

fn cli_dir() -> PathBuf {
    if let Ok(path) = env::var("LAB_CLI_DIR") {
        let p = PathBuf::from(path);
        if is_lab_cli_root(&p) {
            return p;
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(root) = first_lab_cli_root_from(exe) {
            return root;
        }
    }
    if let Ok(cwd) = env::current_dir() {
        if let Some(root) = first_lab_cli_root_from(cwd) {
            return root;
        }
    }
    let home_dir = PathBuf::from(home());
    for base in [
        home_dir.clone(),
        home_dir.join("app-park"),
        home_dir.join("engine-park"),
    ] {
        if let Ok(rd) = fs::read_dir(base) {
            for entry in rd.flatten() {
                let p = entry.path();
                if is_lab_cli_root(&p) {
                    return p;
                }
            }
        }
    }
    home_dir.join("cli")
}

fn commands_dir() -> PathBuf {
    env::var("LAB_COMMANDS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| cli_dir().join("commands"))
}

/// One creds source: <lab-root>/.env first, then the existing ~/.radar/sync.env.
/// Returns None if creds are absent (for best-effort callers); see load_creds().
fn try_creds() -> Option<(String, String)> {
    let mut map: HashMap<String, String> = HashMap::new();
    let candidates = [
        cli_dir().join(".env"),
        PathBuf::from(home()).join(".radar/sync.env"),
    ];
    for path in candidates.iter() {
        if let Ok(text) = fs::read_to_string(path) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    map.entry(k.trim().to_string())
                        .or_insert_with(|| v.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    let pick = |keys: &[&str]| -> String {
        for k in keys {
            if let Some(v) = map.get(*k) {
                if !v.is_empty() {
                    return v.clone();
                }
            }
            if let Ok(v) = env::var(k) {
                if !v.is_empty() {
                    return v;
                }
            }
        }
        String::new()
    };
    let url = pick(&["SUPABASE_URL", "RADAR_SUPABASE_URL", "LAB_SUPABASE_URL"]);
    let key = pick(&["SUPABASE_KEY", "RADAR_SUPABASE_KEY", "LAB_SUPABASE_KEY"]);
    if url.is_empty() || key.is_empty() {
        return None;
    }
    Some((url.trim_end_matches('/').to_string(), key))
}

/// Strict creds for the I/O paths -- exits if absent.
fn load_creds() -> (String, String) {
    try_creds().unwrap_or_else(|| {
        eprintln!("lab: no Supabase creds. Set SUPABASE_URL/SUPABASE_KEY (or RADAR_*) in <lab-root>/.env or ~/.radar/sync.env");
        exit(2);
    })
}

fn curl(args: &[String]) -> String {
    match Command::new("curl").args(args).output() {
        Ok(o) => {
            if !o.status.success() && o.stdout.is_empty() {
                eprintln!(
                    "lab: curl error: {}",
                    String::from_utf8_lossy(&o.stderr).trim()
                );
            }
            String::from_utf8_lossy(&o.stdout).to_string()
        }
        Err(e) => {
            eprintln!("lab: cannot run curl: {}", e);
            exit(1);
        }
    }
}

fn rest_read(url: &str, key: &str, table: &str, query: &str) -> String {
    let q = if query.is_empty() {
        "select=*&limit=20".to_string()
    } else {
        query.to_string()
    };
    let full = format!("{}/rest/v1/{}?{}", url, table, q);
    curl(&[
        "-sS".into(),
        "-H".into(),
        format!("apikey: {}", key),
        "-H".into(),
        format!("Authorization: Bearer {}", key),
        full,
    ])
}

/// Raw POST. Returns (body, http_status). Never exits on a non-2xx -- the caller
/// decides what to do, because in the LAB a rejected write is a warning, not death.
fn rest_write_raw(url: &str, key: &str, table: &str, json: &str) -> (String, i64) {
    let full = format!("{}/rest/v1/{}", url, table);
    let out = curl(&[
        "-sS".into(),
        "-X".into(),
        "POST".into(),
        "-H".into(),
        format!("apikey: {}", key),
        "-H".into(),
        format!("Authorization: Bearer {}", key),
        "-H".into(),
        "Content-Type: application/json".into(),
        "-H".into(),
        "Prefer: return=representation".into(),
        "-w".into(),
        "\n__CODE__%{http_code}".into(),
        "-d".into(),
        json.to_string(),
        full,
    ]);
    if let Some(idx) = out.rfind("\n__CODE__") {
        let (body, codepart) = out.split_at(idx);
        let code = codepart
            .trim_start_matches("\n__CODE__")
            .trim()
            .parse::<i64>()
            .unwrap_or(0);
        (body.to_string(), code)
    } else {
        (out, 0)
    }
}

/// Canonical content hash: JCS-RFC8785 (audited `serde_jcs`) + sha256, lowercase hex.
/// This is the real thing -- not hand-rolled. Same recipe as actgraph-canon.
fn content_hash(value: &Value) -> String {
    let bytes = serde_jcs::to_vec(value).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(&bytes);
    hex::encode(h.finalize())
}

/// Strip minting/transport meta so the hash covers the *content*, reproducibly.
/// Also drops null-valued keys (server-default columns like `data` read back as
/// null), so a hash survives the round-trip through PostgREST's projection.
fn strip_meta(obj: &serde_json::Map<String, Value>) -> Value {
    let mut m = obj.clone();
    for k in [
        "content_hash",
        "json_canonicalization",
        "hashes",
        "id",
        "inserted_at",
    ] {
        m.remove(k);
    }
    m.retain(|_, v| !v.is_null());
    Value::Object(m)
}

/// The one canonical table. Everything in LogLine form lands here.
const LEDGER: &str = "logline_acts";

/// Build a canonical `logline.receipt.v0` from the nine slots (all strings).
/// Returns (act, content_hash, tuple_hash). Recipe (conformance-exact):
///   tuple_hash   = sha256(jcs({the 9 slots}))
///   content_hash = sha256(jcs(receipt − {id, hashes}))
///   id           = content_hash
/// jcs-rfc8785 sorts keys, so insertion order is irrelevant.
#[allow(clippy::too_many_arguments)]
fn canonical_receipt(
    who: &str,
    did: &str,
    this: &str,
    when: &str,
    confirmed_by: &str,
    if_ok: &str,
    if_doubt: &str,
    if_not: &str,
    status: &str,
    extra: Option<serde_json::Map<String, Value>>,
) -> (Value, String, String) {
    let slots = |m: &mut serde_json::Map<String, Value>| {
        for (k, v) in [
            ("who", who),
            ("did", did),
            ("this", this),
            ("when", when),
            ("confirmed_by", confirmed_by),
            ("if_ok", if_ok),
            ("if_doubt", if_doubt),
            ("if_not", if_not),
            ("status", status),
        ] {
            m.insert(k.into(), Value::String(v.into()));
        }
    };
    // tuple_hash over the nine slots alone
    let mut tuple = serde_json::Map::new();
    slots(&mut tuple);
    let tuple_hash = content_hash(&Value::Object(tuple));
    // The receipt body, sans id/hashes. Canonical fields, then AUX (any extra
    // top-level keys -- the DB projects `aux` = act minus the canonical fields).
    // content_hash MUST cover the aux, so it is folded in BEFORE hashing.
    let mut r = serde_json::Map::new();
    r.insert(
        "receipt_version".into(),
        Value::String("logline.receipt.v0".into()),
    );
    slots(&mut r);
    r.insert(
        "json_canonicalization".into(),
        Value::String("jcs-rfc8785".into()),
    );
    if let Some(ex) = extra {
        // never let aux shadow a canonical/reserved/forbidden field
        let reserved = [
            "who",
            "did",
            "this",
            "when",
            "confirmed_by",
            "if_ok",
            "if_doubt",
            "if_not",
            "status",
            "id",
            "hashes",
            "receipt_version",
            "json_canonicalization",
            "result",
            "evidence",
            "transport",
        ];
        for (k, v) in ex {
            if !reserved.contains(&k.as_str()) {
                r.insert(k, v);
            }
        }
    }
    let c_hash = content_hash(&Value::Object(r.clone()));
    // mint id + hashes onto the full receipt
    r.insert("id".into(), Value::String(c_hash.clone()));
    let mut h = serde_json::Map::new();
    h.insert("tuple_hash".into(), Value::String(tuple_hash.clone()));
    h.insert("content_hash".into(), Value::String(c_hash.clone()));
    h.insert("algorithm".into(), Value::String("sha256".into()));
    r.insert("hashes".into(), Value::Object(h));
    (Value::Object(r), c_hash, tuple_hash)
}

/// Assemble the `logline_acts` row: the `act` (truth) plus the three
/// non-generated projection columns the CHECK constraints pin to it. The slots
/// and `aux` are GENERATED from `act` by the database -- we never set them.
fn act_row(receipt: &Value, content: &str, tuple: &str) -> Value {
    let mut row = serde_json::Map::new();
    row.insert("act".into(), receipt.clone());
    row.insert("content_hash".into(), Value::String(content.into()));
    row.insert("tuple_hash".into(), Value::String(tuple.into()));
    row.insert(
        "receipt_version".into(),
        Value::String("logline.receipt.v0".into()),
    );
    Value::Object(row)
}

/// Post a fully-minted canonical row to the one ledger. No re-hashing, no
/// meta-stamping -- the receipt is already canonical. Returns (body, code).
fn write_act_row(url: &str, key: &str, row: &Value) -> (String, i64) {
    let body = serde_json::to_string(row).unwrap_or_default();
    rest_write_raw(url, key, LEDGER, &body)
}

/// Rust-compiler-style status. A write never "fails" -- like `cargo` it always
/// reports Registered, with a [tier] tag saying how conformant it managed to be.
/// (The one genuine error is an unreachable ledger -- we don't fake that green.)
fn registered(table: &str, tier: &str, hash: Option<&str>) {
    match hash {
        Some(h) => eprintln!("   Registered {} [{}] {}", table, tier, h),
        None => eprintln!("   Registered {} [{}]", table, tier),
    }
}

/// Mint the JCS hash + canon tag onto an object and POST it. Returns (body, code, hash).
fn store_object(
    url: &str,
    key: &str,
    table: &str,
    map: &serde_json::Map<String, Value>,
) -> (String, i64, String) {
    let h = content_hash(&strip_meta(map));
    let mut out = map.clone();
    out.insert("content_hash".into(), Value::String(h.clone()));
    out.insert(
        "json_canonicalization".into(),
        Value::String("jcs-rfc8785".into()),
    );
    let body = serde_json::to_string(&Value::Object(out)).unwrap_or_default();
    let (resp, code) = rest_write_raw(url, key, table, &body);
    (resp, code, h)
}

/// Last resort so the act is NEVER lost: wrap the raw payload in a canon row and
/// land it in lab_log (the LAB's journal). Register first, always.
fn wrap_raw(table_attempted: &str, raw: &str) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("who".into(), Value::String(hostname()));
    m.insert("did".into(), Value::String("write".into()));
    m.insert("this".into(), Value::String(table_attempted.into()));
    m.insert("when".into(), Value::String(now_utc()));
    m.insert("status".into(), Value::String("raw".into()));
    let mut data = serde_json::Map::new();
    data.insert("raw".into(), Value::String(raw.into()));
    m.insert("data".into(), Value::Object(data));
    m
}

fn register_fallback(url: &str, key: &str, table_attempted: &str, raw: &str) -> String {
    let wrapped = wrap_raw(table_attempted, raw);
    let (resp, code, h) = store_object(url, key, "lab_log", &wrapped);
    if (200..300).contains(&code) {
        let tier = if table_attempted == "lab_log" {
            "raw · wrapped + hashed".to_string()
        } else {
            format!("raw · wrapped into lab_log (from {})", table_attempted)
        };
        registered("lab_log", &tier, Some(&h));
        return resp;
    }
    // The only true failure: the ledger itself wouldn't take it.
    eprintln!(
        "error: could not register — Supabase unreachable (HTTP {}): {}",
        code,
        resp.trim()
    );
    resp
}

/// The one write path. Always lands the act; the tier tag says how clean it was.
///   [conformant]            valid object -> JCS-hashed + canon-tagged + stored
///   [hash uncommitted]      table rejected canon columns -> stored raw, hash shown
///   [raw · wrapped...]      unparseable/non-object -> wrapped into lab_log, hashed
fn write_hashed(url: &str, key: &str, table: &str, json_str: &str) -> String {
    match serde_json::from_str::<Value>(json_str) {
        Ok(Value::Object(map)) => {
            let (resp, code, h) = store_object(url, key, table, &map);
            if (200..300).contains(&code) {
                registered(table, "conformant · jcs-rfc8785 + sha256", Some(&h));
                return resp;
            }
            // Canon columns not on this table -> store the bare object, hash still shown.
            let (resp2, code2) = rest_write_raw(url, key, table, json_str);
            if (200..300).contains(&code2) {
                registered(
                    table,
                    "jcs-rfc8785 · hash uncommitted (no canon columns here)",
                    Some(&h),
                );
                return resp2;
            }
            // Table won't take it at all -> never lose the act; wrap into lab_log.
            register_fallback(url, key, table, json_str)
        }
        _ => register_fallback(url, key, table, json_str),
    }
}

/// Advisory conformance. Point it at any row; it tells you right/wrong and WHY.
/// Always exits 0 -- it informs, it never blocks. (The LAB is not a prison.)
fn conformance(json_str: &str) {
    println!("lab conformance — advisory (warns, never blocks)\n");
    let v: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            println!("⚠  not valid JSON: {}", e);
            println!(
                "\nverdict: NON-CONFORMANT — but a write would still be accepted (register first)"
            );
            return;
        }
    };
    let obj = match v.as_object() {
        Some(o) => o,
        None => {
            println!("⚠  not a JSON object (canon rows are objects)");
            println!("\nverdict: NON-CONFORMANT — write still accepted");
            return;
        }
    };
    let mut warns = 0;
    for f in ["who", "did", "this", "when"] {
        match obj.get(f).and_then(|x| x.as_str()) {
            Some(s) if !s.is_empty() => println!("✓  {} present", f),
            _ => {
                println!("⚠  {} missing/empty", f);
                warns += 1;
            }
        }
    }
    match obj.get("content_hash").and_then(|x| x.as_str()) {
        Some(stored) => {
            let recomputed = content_hash(&strip_meta(obj));
            if recomputed == stored {
                println!("✓  content_hash reproduces (jcs-rfc8785 + sha256)");
            } else {
                println!("⚠  content_hash does NOT match recompute");
                println!("     stored:     {}", stored);
                println!("     recomputed: {}", recomputed);
                warns += 1;
            }
        }
        None => {
            println!("⚠  content_hash absent (a `lab write` would mint one)");
            warns += 1;
        }
    }
    match obj.get("json_canonicalization").and_then(|x| x.as_str()) {
        Some("jcs-rfc8785") => println!("✓  json_canonicalization = jcs-rfc8785"),
        Some(other) => {
            println!(
                "⚠  json_canonicalization = {} (expected jcs-rfc8785)",
                other
            );
            warns += 1;
        }
        None => println!("·  json_canonicalization tag absent (optional)"),
    }
    println!();
    if warns == 0 {
        println!("verdict: CONFORMANT ✓");
    } else {
        println!(
            "verdict: {} warning(s) — informational only, a write is still accepted",
            warns
        );
    }
}

/// Pretty `tail` of the journal: newest at the bottom, one act per line.
fn print_tail(url: &str, key: &str, n: usize) {
    let q = format!(
        "select=when,who,did,this,status&order=inserted_at.desc&limit={}",
        n
    );
    let body = rest_read(url, key, "lab_log", &q);
    match serde_json::from_str::<Value>(&body) {
        Ok(Value::Array(rows)) => {
            for r in rows.iter().rev() {
                let g = |k: &str| r.get(k).and_then(|x| x.as_str()).unwrap_or("");
                println!(
                    "{:<20}  {:<8}  {:<10}  {}  [{}]",
                    g("when"),
                    g("who"),
                    g("did"),
                    g("this"),
                    g("status")
                );
            }
        }
        _ => print!("{}", body),
    }
}

/// Is the ledger reachable, and how fast? Returns (ok, http_code, seconds).
fn ledger_ping(url: &str, key: &str) -> (bool, String, String) {
    let full = format!("{}/rest/v1/lab_log?select=who&limit=1", url);
    let out = curl(&[
        "-sS".into(),
        "-o".into(),
        "/dev/null".into(),
        "-H".into(),
        format!("apikey: {}", key),
        "-H".into(),
        format!("Authorization: Bearer {}", key),
        "-w".into(),
        "%{http_code} %{time_total}".into(),
        full,
    ]);
    let mut it = out.split_whitespace();
    let code = it.next().unwrap_or("0").to_string();
    let secs = it.next().unwrap_or("?").to_string();
    (code.starts_with('2'), code, secs)
}

/// Maileroo creds live in ~/.radar/.notify.env (reuse the one source).
fn load_notify_cfg() -> Option<HashMap<String, String>> {
    let path = env::var("LAB_NOTIFY_ENV")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(home()).join(".radar/.notify.env"));
    let text = fs::read_to_string(&path).ok()?;
    let mut m = HashMap::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            m.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
        }
    }
    Some(m)
}

/// Minimal percent-encode for a PostgREST query value.
fn pct(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                o.push(b as char)
            }
            _ => o.push_str(&format!("%{:02X}", b)),
        }
    }
    o
}

/// Reach-Dan: send an email via Maileroo (curl SMTP, no TLS-in-Rust), record the
/// act through the one path, dedup the same subject within 12h. Life-and-death only
/// is the CALLER's discipline; this is just the honest mechanism.
fn cmd_notify(subject_raw: &str, body: &str) {
    // strip CR/LF from the subject (no header injection)
    let subject: String = subject_raw
        .chars()
        .filter(|c| *c != '\r' && *c != '\n')
        .collect();
    let cfg = match load_notify_cfg() {
        Some(c) => c,
        None => {
            eprintln!("lab: no notify creds at ~/.radar/.notify.env");
            exit(2);
        }
    };
    let get = |k: &str, d: &str| {
        cfg.get(k)
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| d.to_string())
    };
    let host = get("MAILEROO_HOST", "smtp.maileroo.com");
    let port = get("MAILEROO_PORT", "587");
    let user = get("MAILEROO_USER", "");
    let pass = get("MAILEROO_PASS", "");
    let from = get("NOTIFY_FROM", &user);
    let to = get("NOTIFY_TO", "");
    if to.is_empty() || user.is_empty() {
        eprintln!("lab: NOTIFY_TO / MAILEROO_USER not set");
        exit(2);
    }

    let creds = try_creds();
    let mut status = "sent";

    // dedup: same subject already sent in the last 12h?
    if let Some((url, key)) = &creds {
        let cutoff = shell_value(&["date", "-u", "-v-12H", "+%Y-%m-%dT%H:%M:%SZ"], "");
        if !cutoff.is_empty() {
            let q = format!(
                "select=id&did=eq.notify&status=eq.sent&this=eq.{}&inserted_at=gt.{}&limit=1",
                pct(&subject),
                cutoff
            );
            let resp = rest_read(url, key, "lab_log", &q);
            if resp.contains("\"id\"") {
                status = "deduped";
                eprintln!("   Notify   [deduped · same subject < 12h] {}", subject);
            }
        }
    }

    if status == "sent" {
        let date = shell_value(&["date", "+%a, %d %b %Y %H:%M:%S %z"], "");
        let msg = format!(
            "From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nDate: {date}\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n"
        );
        let tmp = std::env::temp_dir().join(format!("lab-notify-{}.eml", std::process::id()));
        if fs::write(&tmp, &msg).is_err() {
            eprintln!("lab: cannot stage message");
            exit(1);
        }
        let out = curl(&[
            "-sS".into(),
            "--ssl-reqd".into(),
            format!("smtp://{host}:{port}"),
            "--mail-from".into(),
            from.clone(),
            "--mail-rcpt".into(),
            to.clone(),
            "--user".into(),
            format!("{user}:{pass}"),
            "-T".into(),
            tmp.to_string_lossy().to_string(),
            "-w".into(),
            "\n__CODE__%{response_code}".into(),
        ]);
        let _ = fs::remove_file(&tmp);
        let code = out.rsplit("__CODE__").next().unwrap_or("").trim();
        if code.starts_with('2') {
            eprintln!("   Notify   [sent · {host}:{port}] {to}");
        } else {
            status = "failed";
            eprintln!(
                "   Notify   [FAILED · smtp {}] {to}",
                if code.is_empty() { "no-reply" } else { code }
            );
        }
    }

    // record the act through the one path (best-effort, hashed)
    if let Some((url, key)) = &creds {
        let mut m = serde_json::Map::new();
        m.insert("who".into(), Value::String(hostname()));
        m.insert("did".into(), Value::String("notify".into()));
        m.insert("this".into(), Value::String(subject.clone()));
        m.insert("when".into(), Value::String(now_utc()));
        m.insert("status".into(), Value::String(status.into()));
        let mut data = serde_json::Map::new();
        data.insert("to".into(), Value::String(to));
        data.insert(
            "body".into(),
            Value::String(body.chars().take(180).collect()),
        );
        m.insert("data".into(), Value::Object(data));
        let json = serde_json::to_string(&Value::Object(m)).unwrap_or_default();
        let _ = write_hashed(url, key, "lab_log", &json);
    }
    if status == "failed" {
        exit(1);
    }
}

/// Wrap the (Python/bash) radar scanner: run it, read the local slices it writes,
/// and register a hashed SUMMARY act through the one path. Full detail stays local
/// in ~/.radar (share-nothing; local is source of truth). The shape Dan blessed:
/// don't rewrite the organ — lab owns identity + hash + canon, the script does macOS.
/// Run the radar scanner in a mode (all | next | <subject>). Refreshes local
/// ~/.radar slices only — no ledger write (share-nothing; local is source of truth).
fn run_scanner(mode: &str) -> bool {
    let script = PathBuf::from(home()).join(".radar/radar-scan.sh");
    if !script.exists() {
        eprintln!("lab: radar-scan.sh not found ({})", script.display());
        return false;
    }
    match Command::new("bash").arg(&script).arg(mode).status() {
        Ok(status) => status.success(),
        Err(e) => {
            eprintln!("lab: failed to run radar scanner: {}", e);
            false
        }
    }
}

fn cmd_scan(url: &str, key: &str, subject: Option<&str>) {
    let radar_dir = PathBuf::from(home()).join(".radar");
    let arg = subject.unwrap_or("all").to_string();
    eprintln!("   Scanning  {} ...", arg);
    if !run_scanner(&arg) {
        exit(1);
    }

    // read the slice files (auto-discover by shape: has subject + items)
    let files: Vec<PathBuf> = match subject {
        Some(s) => vec![radar_dir.join(format!("{}.json", s))],
        None => fs::read_dir(&radar_dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
                    .collect()
            })
            .unwrap_or_default(),
    };
    let mut summary = serde_json::Map::new();
    let mut total: u64 = 0;
    for f in files {
        if let Ok(text) = fs::read_to_string(&f) {
            if let Ok(Value::Object(o)) = serde_json::from_str::<Value>(&text) {
                if let (Some(Value::String(subj)), Some(items)) = (o.get("subject"), o.get("items"))
                {
                    let count = o
                        .get("coverage")
                        .and_then(|c| c.get("count"))
                        .and_then(|c| c.as_u64())
                        .unwrap_or_else(|| items.as_array().map(|a| a.len() as u64).unwrap_or(0));
                    summary.insert(subj.clone(), Value::from(count));
                    total += count;
                }
            }
        }
    }

    // human-readable breakdown to stdout
    let mut keys: Vec<String> = summary.keys().cloned().collect();
    keys.sort();
    for k in &keys {
        println!(
            "  {:<12} {}",
            k,
            summary.get(k).and_then(|v| v.as_u64()).unwrap_or(0)
        );
    }
    println!("  {:<12} {}", "TOTAL", total);

    // register a hashed summary act through the one path
    let mut m = serde_json::Map::new();
    m.insert("who".into(), Value::String(hostname()));
    m.insert("did".into(), Value::String("scan".into()));
    m.insert("this".into(), Value::String(hostname()));
    m.insert("when".into(), Value::String(now_utc()));
    m.insert("status".into(), Value::String("ok".into()));
    let mut data = serde_json::Map::new();
    data.insert("scanned".into(), Value::String(arg));
    data.insert("total".into(), Value::from(total));
    data.insert("subjects".into(), Value::Object(summary));
    m.insert("data".into(), Value::Object(data));
    let json = serde_json::to_string(&Value::Object(m)).unwrap_or_default();
    let _ = write_hashed(url, key, "lab_log", &json);
}

fn cmd_export(url: &str, key: &str) {
    let radar_dir = PathBuf::from(home()).join(".radar");
    let exporter = radar_dir.join("radar-export.py");
    if !exporter.exists() {
        eprintln!(
            "lab: radar-export.py not found ({}) — reinstall the Radar payload",
            exporter.display()
        );
        exit(2);
    }

    eprintln!("   Loading   Radar on {} ...", hostname());
    eprintln!("   Scanning  every Radar phase before export ...");
    if !run_scanner("full") {
        eprintln!("lab: export stopped because the full scan did not complete");
        exit(1);
    }

    eprintln!("   Exporting raw Radar files ...");
    let out = match Command::new("python3").arg(&exporter).output() {
        Ok(out) => out,
        Err(e) => {
            eprintln!("lab: failed to run radar exporter: {}", e);
            exit(1);
        }
    };
    if !out.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
    }
    if !out.status.success() {
        eprintln!("lab: radar exporter failed");
        exit(out.status.code().unwrap_or(1));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let summary: Value = match serde_json::from_str(text.trim()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("lab: radar exporter returned unreadable output: {}", e);
            eprintln!("{}", text.trim());
            exit(1);
        }
    };

    let md_path = summary
        .get("markdown_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let json_path = summary
        .get("json_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let txt_path = summary
        .get("text_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let sidecar_path = summary
        .get("sidecar_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let export_sha256 = summary
        .get("export_sha256")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let items_total = summary
        .get("items_total")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    println!("Radar export complete");
    println!("  markdown {}", md_path);
    println!("  json     {}", json_path);
    println!("  text     {}", txt_path);
    println!("  sidecar  {}", sidecar_path);
    println!("  sha256   {}", export_sha256);
    println!("  items    {}", items_total);

    let mut m = serde_json::Map::new();
    m.insert("who".into(), Value::String(hostname()));
    m.insert("did".into(), Value::String("export".into()));
    m.insert("this".into(), Value::String("radar".into()));
    m.insert("when".into(), Value::String(now_utc()));
    m.insert("status".into(), Value::String("ok".into()));
    let mut data = serde_json::Map::new();
    data.insert("json_path".into(), Value::String(json_path.into()));
    data.insert("markdown_path".into(), Value::String(md_path.into()));
    data.insert("text_path".into(), Value::String(txt_path.into()));
    data.insert("sidecar_path".into(), Value::String(sidecar_path.into()));
    data.insert("export_sha256".into(), Value::String(export_sha256.into()));
    data.insert("items_total".into(), Value::from(items_total));
    if let Some(subjects) = summary.get("subjects") {
        data.insert("subjects".into(), subjects.clone());
    }
    m.insert("data".into(), Value::Object(data));
    let json = serde_json::to_string(&Value::Object(m)).unwrap_or_default();
    let _ = write_hashed(url, key, "lab_log", &json);
}

fn cmd_validate(rest: &[String]) {
    let radar_dir = PathBuf::from(home()).join(".radar");
    let validator = radar_dir.join("radar-validate.py");
    if !validator.exists() {
        eprintln!(
            "lab: radar-validate.py not found ({}) — reinstall the Radar payload",
            validator.display()
        );
        exit(2);
    }
    let export_path = rest
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| radar_dir.join("exports/radar-full-latest.json"));
    let sidecar_path = rest
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| radar_dir.join("exports/radar-full-latest.json.sha256"));
    let mut cmd = Command::new("python3");
    cmd.arg(&validator).arg(&export_path);
    if sidecar_path.exists() {
        cmd.arg(&sidecar_path);
    }
    match cmd.status() {
        Ok(status) => {
            if !status.success() {
                exit(status.code().unwrap_or(1));
            }
        }
        Err(e) => {
            eprintln!("lab: failed to run radar validator: {}", e);
            exit(1);
        }
    }
}

/// Wrap the radar judge: run it, read verdict.json, register a hashed `did=judge`
/// summary act through the one path. Keeps radar's honest verdict vocabulary
/// (OK/DOWN/DEGRADED/UNKNOWN/UNDEFINED) verbatim — richer than NodeLiveness's 3
/// states because it preserves UNKNOWN (never green without evidence).
fn cmd_judge(url: &str, key: &str) {
    let radar_dir = PathBuf::from(home()).join(".radar");
    let script = radar_dir.join("radar-judge.py");
    if !script.exists() {
        eprintln!(
            "lab: radar-judge.py not found ({}) — judge needs the radar judge",
            script.display()
        );
        exit(2);
    }
    eprintln!("   Judging   {} ...", hostname());
    if let Err(e) = Command::new("python3").arg(&script).output() {
        eprintln!("lab: judge failed to run: {}", e);
        exit(1);
    }

    let vpath = radar_dir.join("verdict.json");
    let v: Value = match fs::read_to_string(&vpath)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
    {
        Some(v) => v,
        None => {
            eprintln!("lab: could not read verdict.json after judge");
            exit(1);
        }
    };

    // per-item lines + collect compact items for the act
    let mut items_out: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    if let Some(arr) = v.get("items").and_then(|x| x.as_array()) {
        for it in arr {
            let id = it.get("id").and_then(|x| x.as_str()).unwrap_or("?");
            let verdict = it
                .get("verdict")
                .and_then(|x| x.as_str())
                .unwrap_or("UNKNOWN");
            let action = it.get("action").and_then(|x| x.as_str()).unwrap_or("");
            println!("  {:<9} {}", verdict, id);
            seen.push(verdict.to_string());
            let mut o = serde_json::Map::new();
            o.insert("id".into(), Value::String(id.into()));
            o.insert("verdict".into(), Value::String(verdict.into()));
            o.insert("action".into(), Value::String(action.into()));
            items_out.push(Value::Object(o));
        }
    }
    // overall status: worst wins (down > degraded > unknown > ok)
    let any = |s: &str| seen.iter().any(|x| x == s);
    let overall = if any("DOWN") {
        "down"
    } else if any("DEGRADED") {
        "degraded"
    } else if any("UNKNOWN") || any("UNDEFINED") {
        "unknown"
    } else {
        "ok"
    };
    if let Some(s) = v.get("summary") {
        println!("  ── {}  → {}", s, overall);
    }

    let mut m = serde_json::Map::new();
    m.insert("who".into(), Value::String(hostname()));
    m.insert("did".into(), Value::String("judge".into()));
    m.insert("this".into(), Value::String(hostname()));
    m.insert("when".into(), Value::String(now_utc()));
    m.insert("status".into(), Value::String(overall.into()));
    let mut data = serde_json::Map::new();
    if let Some(s) = v.get("summary") {
        data.insert("summary".into(), s.clone());
    }
    data.insert("items".into(), Value::Array(items_out));
    m.insert("data".into(), Value::Object(data));
    let json = serde_json::to_string(&Value::Object(m)).unwrap_or_default();
    let _ = write_hashed(url, key, "lab_log", &json);
}

/// The EMAIL bar is far tighter than the judge's DOWN. The judge records every
/// red (disk < 10GB SLO, mcp down, …) in the ledger — silent. `lab radar` only
/// reaches Dan for a genuine box-emergency (about-to-wedge). Sacred-inbox rule.
const CRITICAL_DISK_GB: f64 = 3.0;

/// Is the box in a genuine life-and-death state right now? Returns the reason.
/// Tighter than any single judge verdict — extend with more predicates over time.
fn box_critical_reason() -> Option<String> {
    // disk about to wedge? (df -k / -> Available is the token before the NN% capacity)
    let out = shell_value(&["df", "-k", "/"], "");
    let toks: Vec<&str> = out.split_whitespace().collect();
    if let Some(cap_idx) = toks.iter().position(|t| t.ends_with('%') && t.len() > 1) {
        if cap_idx >= 1 {
            if let Ok(avail_kb) = toks[cap_idx - 1].parse::<f64>() {
                let gb = avail_kb / 1024.0 / 1024.0;
                if gb < CRITICAL_DISK_GB {
                    return Some(format!(
                        "Problem: disk is almost full on / ({:.1} GB free, floor {:.0} GB).\n\nDo this: free disk space on this machine now, then let Radar re-check.",
                        gb, CRITICAL_DISK_GB
                    ));
                }
            }
        }
    }
    None
}

fn manhattan_pair(root: &Path) -> Option<(PathBuf, PathBuf)> {
    let script = root.join("src/manhattan.py");
    let policy_package = root.join("PROJECT_MANHATTAN_POLICY_REVIEW.json");
    let policy_installed = root.join("etc/PROJECT_MANHATTAN_POLICY_REVIEW.json");
    if script.exists() && policy_installed.exists() {
        return Some((script, policy_installed));
    }
    if script.exists() && policy_package.exists() {
        return Some((script, policy_package));
    }
    None
}

fn push_manhattan_candidates(out: &mut Vec<PathBuf>, base: PathBuf) {
    if !base.exists() {
        return;
    }
    out.push(base.clone());
    if let Ok(rd) = fs::read_dir(&base) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.push(path);
            }
        }
    }
}

/// Locate the Manhattan engine + its policy. The installed root is canonical,
/// but source checkouts can have any folder name.
fn find_manhattan() -> Option<(PathBuf, PathBuf)> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(root) = env::var("MANHATTAN_ROOT") {
        roots.push(PathBuf::from(root));
    }
    roots.push(PathBuf::from("/usr/local/project-manhattan"));
    if let Ok(cwd) = env::current_dir() {
        for ancestor in cwd.ancestors() {
            roots.push(ancestor.to_path_buf());
        }
    }
    let home_dir = PathBuf::from(home());
    push_manhattan_candidates(&mut roots, home_dir.join("manhattan"));
    push_manhattan_candidates(&mut roots, home_dir.join("MANHATTAN"));
    push_manhattan_candidates(&mut roots, home_dir.join("app-park"));
    roots.push(home_dir);

    let mut seen = HashSet::new();
    for root in roots {
        let key = root.to_string_lossy().to_string();
        if !seen.insert(key) {
            continue;
        }
        if let Some(pair) = manhattan_pair(&root) {
            return Some(pair);
        }
    }
    None
}

fn manhattan_root_from_script(script: &Path) -> Option<PathBuf> {
    script
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
}

fn manhattan_sync_state_path() -> PathBuf {
    PathBuf::from(home()).join(".radar/manhattan-sync.seen")
}

fn read_manhattan_seen() -> HashSet<String> {
    fs::read_to_string(manhattan_sync_state_path())
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn write_manhattan_seen(seen: &HashSet<String>) {
    let path = manhattan_sync_state_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut rows: Vec<&String> = seen.iter().collect();
    rows.sort();
    let body = rows
        .into_iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::write(
        path,
        if body.is_empty() {
            body
        } else {
            format!("{}\n", body)
        },
    );
}

fn manhattan_receipt_status(receipt_type: &str, receipt: &Value) -> String {
    if let Some(s) = receipt
        .get("data")
        .and_then(|d| d.get("status"))
        .and_then(|s| s.as_str())
    {
        return s.to_string();
    }
    let r = receipt_type.to_ascii_uppercase();
    if r.contains("FAILED") {
        "failed".into()
    } else if r.contains("BLOCKED") || r.contains("HUMAN_REQUIRED") || r.contains("MISSING") {
        "blocked".into()
    } else if r.contains("AUDIT") {
        "audit".into()
    } else if r.contains("REPAIR") {
        "repair".into()
    } else {
        "ok".into()
    }
}

fn compact_manhattan_receipt(
    path: &Path,
    receipt: &Value,
    receipt_hash: &str,
) -> serde_json::Map<String, Value> {
    let receipt_type = receipt
        .get("receipt_type")
        .and_then(|v| v.as_str())
        .unwrap_or("PROJECT_MANHATTAN_RECEIPT");
    let lab_id = receipt.get("lab_id").and_then(|v| v.as_str()).unwrap_or("");
    let when = receipt
        .get("timestamp")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(now_utc);
    let status = manhattan_receipt_status(receipt_type, receipt);

    let mut data = serde_json::Map::new();
    data.insert("receipt_type".into(), Value::String(receipt_type.into()));
    data.insert("receipt_hash".into(), Value::String(receipt_hash.into()));
    data.insert(
        "receipt_path".into(),
        Value::String(path.to_string_lossy().to_string()),
    );
    if !lab_id.is_empty() {
        data.insert("manhattan_lab_id".into(), Value::String(lab_id.into()));
    }
    for k in ["project", "schema_version", "actor", "message", "timestamp"] {
        if let Some(v) = receipt.get(k) {
            data.insert(k.into(), v.clone());
        }
    }
    if let Some(obj) = receipt.get("data").and_then(|v| v.as_object()) {
        for k in [
            "item_id",
            "apply",
            "target_count",
            "targets",
            "drift_count_before",
            "drift_count_after",
            "status",
        ] {
            if let Some(v) = obj.get(k) {
                data.insert(k.into(), v.clone());
            }
        }
        if let Some(results) = obj.get("results").and_then(|v| v.as_array()) {
            let mut hist: HashMap<String, u64> = HashMap::new();
            for r in results {
                let s = r
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                *hist.entry(s).or_insert(0) += 1;
            }
            data.insert(
                "result_counts".into(),
                Value::Object(hist.into_iter().map(|(k, v)| (k, Value::from(v))).collect()),
            );
        }
    }

    let mut m = serde_json::Map::new();
    m.insert("who".into(), Value::String(hostname()));
    m.insert("did".into(), Value::String("manhattan.receipt".into()));
    m.insert(
        "this".into(),
        Value::String(if lab_id.is_empty() {
            receipt_type.into()
        } else {
            format!("{}:{}", lab_id, receipt_type)
        }),
    );
    m.insert("when".into(), Value::String(when));
    m.insert("status".into(), Value::String(status));
    m.insert("data".into(), Value::Object(data));
    m
}

fn sync_manhattan_receipts(
    url: &str,
    key: &str,
    limit: Option<usize>,
    include_seen: bool,
) -> usize {
    let (script, _) = match find_manhattan() {
        Some(x) => x,
        None => return 0,
    };
    let root = match manhattan_root_from_script(&script) {
        Some(r) => r,
        None => return 0,
    };
    let receipts = root.join("var/receipts");
    let mut files: Vec<(u64, PathBuf)> = fs::read_dir(&receipts)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
                .filter_map(|p| {
                    let ts = fs::metadata(&p)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    Some((ts, p))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|(ts, p)| (*ts, p.clone()));
    if let Some(n) = limit {
        if files.len() > n {
            files = files.split_off(files.len() - n);
        }
    }

    let mut seen = if include_seen {
        HashSet::new()
    } else {
        read_manhattan_seen()
    };
    let mut emitted = 0usize;
    for (_, path) in files {
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let receipt = match serde_json::from_str::<Value>(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let receipt_hash = content_hash(&receipt);
        if !include_seen && seen.contains(&receipt_hash) {
            continue;
        }
        let m = compact_manhattan_receipt(&path, &receipt, &receipt_hash);
        let json = serde_json::to_string(&Value::Object(m)).unwrap_or_default();
        let _ = write_hashed(url, key, "lab_log", &json);
        seen.insert(receipt_hash);
        emitted += 1;
    }
    if !include_seen {
        write_manhattan_seen(&seen);
    }
    emitted
}

fn cmd_manhattan_sync(url: &str, key: &str, rest: &[String]) {
    let mut limit = Some(50usize);
    let mut include_seen = false;
    for arg in rest {
        if arg == "--all" {
            limit = None;
            include_seen = true;
        } else if arg == "--replay" {
            include_seen = true;
        } else if let Ok(n) = arg.parse::<usize>() {
            limit = Some(n);
        }
    }
    let n = sync_manhattan_receipts(url, key, limit, include_seen);
    eprintln!("   Manhattan [synced receipts] {}", n);
}

fn run_manhattan(script: &Path, policy: &Path, sub: &str, apply: bool) -> Option<Value> {
    let mut args: Vec<String> = vec![
        script.to_string_lossy().into_owned(),
        "--policy".into(),
        policy.to_string_lossy().into_owned(),
        sub.into(),
    ];
    if apply {
        args.push("--apply".into());
    }
    let out = Command::new("python3").args(&args).output().ok()?;
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).ok()
}

/// Wrap Manhattan's read-only audit: walk the 30 desired-state items, register a
/// hashed `did=audit` summary. The Python keeps the OS knowledge; lab owns the act.
fn cmd_audit(url: &str, key: &str) {
    let (script, policy) = match find_manhattan() {
        Some(x) => x,
        None => {
            eprintln!(
                "lab: manhattan engine not found (set MANHATTAN_ROOT, install /usr/local/project-manhattan, or run from a Manhattan checkout)"
            );
            exit(2);
        }
    };
    eprintln!("   Auditing  {} (manhattan, read-only) ...", hostname());
    let j = match run_manhattan(&script, &policy, "audit", false) {
        Some(j) => j,
        None => {
            eprintln!("lab: audit produced no JSON");
            exit(1);
        }
    };
    let gi = |k: &str| j.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
    let (total, drift, auto, human) = (
        gi("item_count"),
        gi("drift_count"),
        gi("auto_repairable_drift_count"),
        gi("human_required_count"),
    );
    let lab = j.get("lab_id").and_then(|v| v.as_str()).unwrap_or("?");
    let drift_ids: Vec<String> = j
        .get("drift_items")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.get("id").and_then(|id| id.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    println!(
        "  {} items · {} drift ({} auto · {} human) · {}",
        total, drift, auto, human, lab
    );
    if !drift_ids.is_empty() {
        println!("  drift: {}", drift_ids.join(", "));
    }
    // per-item detail for human-required items
    if let Some(arr) = j.get("drift_items").and_then(|v| v.as_array()) {
        for it in arr {
            let id = it.get("id").and_then(|x| x.as_str()).unwrap_or("?");
            let name = it.get("name").and_then(|x| x.as_str()).unwrap_or("");
            let auto = it
                .get("auto_repairable")
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            let tag = if auto { "auto" } else { "human" };
            println!("    [{tag}] {id}  {name}");
        }
    }
    let status = if drift == 0 { "ok" } else { "drift" };
    let mut m = serde_json::Map::new();
    m.insert("who".into(), Value::String(hostname()));
    m.insert("did".into(), Value::String("audit".into()));
    m.insert("this".into(), Value::String(hostname()));
    m.insert("when".into(), Value::String(now_utc()));
    m.insert("status".into(), Value::String(status.into()));
    let mut data = serde_json::Map::new();
    for k in [
        "item_count",
        "drift_count",
        "auto_repairable_drift_count",
        "human_required_count",
        "lab_id",
    ] {
        if let Some(v) = j.get(k) {
            data.insert(k.into(), v.clone());
        }
    }
    // compact drift items only (id/name/status/auto) — full evidence stays local
    if let Some(arr) = j.get("drift_items").and_then(|v| v.as_array()) {
        let compact: Vec<Value> = arr
            .iter()
            .map(|it| {
                let mut o = serde_json::Map::new();
                for f in ["id", "name", "status", "auto_repairable"] {
                    if let Some(v) = it.get(f) {
                        o.insert(f.into(), v.clone());
                    }
                }
                Value::Object(o)
            })
            .collect();
        data.insert("drift_items".into(), Value::Array(compact));
    }
    m.insert("data".into(), Value::Object(data));
    let _ = write_hashed(
        url,
        key,
        "lab_log",
        &serde_json::to_string(&Value::Object(m)).unwrap_or_default(),
    );
    let synced = sync_manhattan_receipts(url, key, Some(100), false);
    if synced > 0 {
        eprintln!("   Manhattan [synced receipts] {}", synced);
    }
}

/// Wrap Manhattan's converge: PLAN by default (mutates nothing); `--apply` to act.
/// Root items need the armed daemon + sudo — those return blocked/failed, never crash.
/// Core converge logic — non-fatal, usable from both the CLI and cmd_radar.
/// Returns (drift_after, n_applied, n_failed) so callers can decide.
fn run_converge(url: &str, key: &str, apply: bool) -> Option<(u64, u64, u64)> {
    let (script, policy) = find_manhattan()?;
    eprintln!(
        "   Converging {} (manhattan{}) ...",
        hostname(),
        if apply { " --apply" } else { " plan" }
    );
    let j = run_manhattan(&script, &policy, "repair", apply)?;
    let targets: Vec<String> = j
        .get("targets")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let mut hist: HashMap<String, u64> = HashMap::new();
    let mut n_applied = 0u64;
    let mut n_failed = 0u64;
    if let Some(arr) = j.get("results").and_then(|v| v.as_array()) {
        for r in arr {
            let s = r
                .get("status")
                .and_then(|x| x.as_str())
                .unwrap_or("?")
                .to_string();
            if s == "applied" {
                n_applied += 1;
            }
            if s == "failed" {
                n_failed += 1;
            }
            *hist.entry(s).or_insert(0) += 1;
        }
    }
    let drift_after = j
        .get("drift_count_after")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    // per-item lines (for interactive use; radar suppresses stdout)
    println!("  targets [{}]: {}", targets.len(), targets.join(", "));
    if let Some(arr) = j.get("results").and_then(|v| v.as_array()) {
        for r in arr {
            let id = r.get("item_id").and_then(|x| x.as_str()).unwrap_or("?");
            let st = r.get("status").and_then(|x| x.as_str()).unwrap_or("?");
            let marker = match st {
                "planned" | "applied" => "·",
                "blocked" | "failed" => "!",
                _ => "?",
            };
            println!("  {marker} {id:<6}  {st}");
        }
    }
    let mut hist_sorted: Vec<(&String, &u64)> = hist.iter().collect();
    hist_sorted.sort_by_key(|(k, _)| k.as_str());
    let summary: Vec<String> = hist_sorted
        .iter()
        .map(|(k, v)| format!("{v} {k}"))
        .collect();
    println!("  ── {}", summary.join("  "));
    // register the act through the one path
    let cli_status = if apply {
        if n_applied > 0 {
            "healed"
        } else if n_failed > 0 {
            "partial"
        } else {
            "blocked"
        }
    } else {
        "planned"
    };
    let mut m = serde_json::Map::new();
    m.insert("who".into(), Value::String(hostname()));
    m.insert("did".into(), Value::String("converge".into()));
    m.insert("this".into(), Value::String(hostname()));
    m.insert("when".into(), Value::String(now_utc()));
    m.insert("status".into(), Value::String(cli_status.into()));
    let mut data = serde_json::Map::new();
    data.insert("apply".into(), Value::Bool(apply));
    for k in [
        "target_count",
        "targets",
        "drift_count_before",
        "drift_count_after",
        "lab_id",
    ] {
        if let Some(v) = j.get(k) {
            data.insert(k.into(), v.clone());
        }
    }
    let hist_obj: serde_json::Map<String, Value> =
        hist.into_iter().map(|(k, v)| (k, Value::from(v))).collect();
    data.insert("results".into(), Value::Object(hist_obj));
    m.insert("data".into(), Value::Object(data));
    let _ = write_hashed(
        url,
        key,
        "lab_log",
        &serde_json::to_string(&Value::Object(m)).unwrap_or_default(),
    );
    let synced = sync_manhattan_receipts(url, key, Some(100), false);
    if synced > 0 {
        eprintln!("   Manhattan [synced receipts] {}", synced);
    }
    Some((drift_after, n_applied, n_failed))
}

/// CLI-facing wrapper: exits on failure (expected from terminal invocations).
fn cmd_converge(url: &str, key: &str, apply: bool) {
    if find_manhattan().is_none() {
        eprintln!("lab: manhattan engine not found (set MANHATTAN_ROOT, install /usr/local/project-manhattan, or run from a Manhattan checkout)");
        exit(2);
    }
    if run_converge(url, key, apply).is_none() {
        eprintln!("lab: converge produced no output");
        exit(1);
    }
}

/// The per-box observation loop: scan → judge → notify(critical only).
/// Manhattan maintains. Radar observes and escalates only true emergencies.
fn cmd_radar(url: &str, key: &str) {
    eprintln!("── lab radar: scan → judge → notify(critical only) ──");

    // phase 1: refresh ONE rotating subject locally (cheap phased scan)
    run_scanner("next");

    // phase 2: judge — records hashed verdict, sets the frame
    cmd_judge(url, key);

    // phase 3: collect escalation reasons (inbox stays sacred)
    let mut reasons: Vec<String> = Vec::new();

    if let Some(r) = box_critical_reason() {
        reasons.push(r);
    }

    if reasons.is_empty() {
        eprintln!("   Radar     no emergency — verdict recorded, inbox left sacred");
    } else {
        let body = reasons.join("\n\n");
        eprintln!(
            "   Radar     CRITICAL → reaching Dan ({} reason(s))",
            reasons.len()
        );
        cmd_notify(&format!("Action required: {}", hostname()), &body);
    }
}

fn shell_value(args: &[&str], fallback: &str) -> String {
    Command::new(args[0])
        .args(&args[1..])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn now_utc() -> String {
    shell_value(&["date", "-u", "+%Y-%m-%dT%H:%M:%SZ"], "")
}

fn hostname() -> String {
    let h = shell_value(&["scutil", "--get", "LocalHostName"], "");
    if !h.is_empty() {
        return h;
    }
    shell_value(&["hostname", "-s"], "unknown")
}

fn json_escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            _ => o.push(c),
        }
    }
    o
}

fn scaffold_command(name: &str) {
    let dir = commands_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("lab: cannot create {}: {}", dir.display(), e);
        exit(1);
    }
    let path = dir.join(name);
    if path.exists() {
        eprintln!(
            "lab: command '{}' already exists ({})",
            name,
            path.display()
        );
        exit(1);
    }
    let tpl = format!(
        "#!/usr/bin/env bash\n\
         # lab command: {name}   (auto-discovered by `lab` -- run: lab {name})\n\
         # created {when} on {who}\n\
         #\n\
         # Golden rule of the LAB: do ALL Supabase I/O through the kernel ($LAB).\n\
         # Never hold creds, never reimplement the ledger -- call lab:\n\
         #   \"$LAB\" emit {name} <this> [data]      register an act (auto who/when, auto-hashed)\n\
         #   \"$LAB\" read lab_log \"did=eq.{name}\"    read your acts back\n\
         #   \"$LAB\" tail 10                        peek the journal\n\
         set -euo pipefail\n\
         LAB=\"${{LAB_BIN:-lab}}\"\n\
         \n\
         # --- your command starts here ------------------------------------------\n\
         this=\"${{1:-hello}}\"\n\
         echo \"lab {name}: $*\"\n\
         \"$LAB\" emit {name} \"$this\"\n",
        name = name,
        when = now_utc(),
        who = hostname(),
    );
    if let Err(e) = fs::write(&path, tpl) {
        eprintln!("lab: cannot write {}: {}", path.display(), e);
        exit(1);
    }
    let _ = Command::new("chmod")
        .args(["+x", &path.to_string_lossy()])
        .status();
    println!("created command: {}  (run: lab {})", path.display(), name);
}

fn run_external(name: &str, args: &[String], url: &str, key: &str) {
    let path = commands_dir().join(name);
    if !path.exists() {
        eprintln!(
            "lab: unknown command '{}' (no builtin, no plugin at {}/{})",
            name,
            commands_dir().display(),
            name
        );
        exit(127);
    }
    let self_bin = env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "lab".to_string());
    match Command::new(&path)
        .args(args)
        .env("LAB_SUPABASE_URL", url)
        .env("LAB_SUPABASE_KEY", key)
        .env("LAB_BIN", self_bin)
        .status()
    {
        Ok(s) => exit(s.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("lab: failed to run command '{}': {}", name, e);
            exit(1);
        }
    }
}

fn usage() {
    println!(
        "lab — minilab CLI (the one Supabase I/O path)\n\n\
         usage:\n\
         \x20 lab emit <did> <this> [data] [--status s]  register an act (auto who/when, auto-hashed)\n\
         \x20 lab read  <table> [query]     read rows (PostgREST query, e.g. 'who=eq.lab-256')\n\
         \x20 lab write <table> <json>      insert a row (auto-hashed, jcs-rfc8785)\n\
         \x20 lab heartbeat [this]          write a canon-shaped heartbeat to lab_log\n\
         \x20 lab scan [subject]            scan this box (wraps radar) -> hashed summary act\n\
         \x20 lab export                    scan every Radar phase, then write a raw full report\n\
         \x20 lab validate [export] [sha]   validate a Radar raw export + optional sidecar\n\
         \x20 lab judge                     judge this box vs success list -> hashed verdict act\n\
         \x20 lab radar                     scan + judge + reach Dan only if critical (the per-box loop)\n\
         \x20 lab audit                     audit the 30 desired-state items (manhattan, read-only)\n\
         \x20 lab converge [--apply]        converge drift to desired state (plan; --apply mutates)\n\
         \x20 lab manhattan-sync [n|--all]  admit Manhattan receipts into lab_log (compact, deduped)\n\
         \x20 lab tail [n]                  last n acts from the journal (default 20)\n\
         \x20 lab ping                      is the ledger reachable, and how fast\n\
         \x20 lab whoami                    this box's identity (the 'who' it stamps)\n\
         \x20 lab notify <subject> [body]   email NOTIFY_TO (life-and-death only, 12h dedup)\n\
         \x20 lab hash <json>               print the JCS-RFC8785 + sha256 content hash\n\
         \x20 lab conformance <json>        verdict on a row (warns, never blocks)\n\
         \x20 lab commands                  list builtins + discovered plugins\n\
         \x20 lab new-command <name>        scaffold <lab-root>/commands/<name>\n\
         \x20 lab <name> [args]             run <lab-root>/commands/<name>"
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        return;
    }
    let cmd = args[0].as_str();
    let rest = &args[1..];
    match cmd {
        "read" => {
            if rest.is_empty() {
                eprintln!("usage: lab read <table> [query]");
                exit(2);
            }
            let (url, key) = load_creds();
            let query = rest.get(1).cloned().unwrap_or_default();
            print!("{}", rest_read(&url, &key, &rest[0], &query));
        }
        "write" => {
            if rest.len() < 2 {
                eprintln!("usage: lab write <table> <json>");
                exit(2);
            }
            let (url, key) = load_creds();
            print!("{}", write_hashed(&url, &key, &rest[0], &rest[1]));
        }
        "emit" => {
            // lab emit <did> <this> [data-json] [--status <s>]
            // The canonical reusable write: auto-stamps who+when, auto-hashes.
            let (mut did, mut this, mut data_arg) = (None, None, None);
            let mut status = "ok".to_string();
            let mut i = 0;
            while i < rest.len() {
                match rest[i].as_str() {
                    "--status" => {
                        if i + 1 < rest.len() {
                            status = rest[i + 1].clone();
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                    s => {
                        if did.is_none() {
                            did = Some(s.to_string());
                        } else if this.is_none() {
                            this = Some(s.to_string());
                        } else if data_arg.is_none() {
                            data_arg = Some(s.to_string());
                        }
                        i += 1;
                    }
                }
            }
            let (did, this) = match (did, this) {
                (Some(d), Some(t)) => (d, t),
                _ => {
                    eprintln!("usage: lab emit <did> <this> [data-json] [--status <s>]");
                    exit(2);
                }
            };
            let (url, key) = load_creds();
            let mut m = serde_json::Map::new();
            m.insert("who".into(), Value::String(hostname()));
            m.insert("did".into(), Value::String(did));
            m.insert("this".into(), Value::String(this));
            m.insert("when".into(), Value::String(now_utc()));
            m.insert("status".into(), Value::String(status));
            if let Some(d) = data_arg {
                let val = serde_json::from_str::<Value>(&d).unwrap_or_else(|_| {
                    let mut dm = serde_json::Map::new();
                    dm.insert("raw".into(), Value::String(d.clone()));
                    Value::Object(dm)
                });
                m.insert("data".into(), val);
            }
            let json = serde_json::to_string(&Value::Object(m)).unwrap_or_default();
            print!("{}", write_hashed(&url, &key, "lab_log", &json));
        }
        "heartbeat" => {
            let (url, key) = load_creds();
            let this = rest.first().cloned().unwrap_or_else(|| "fleet".to_string());
            let json = format!(
                "{{\"who\":\"{}\",\"did\":\"heartbeat\",\"this\":\"{}\",\"when\":\"{}\",\"status\":\"claimed\"}}",
                json_escape(&hostname()),
                json_escape(&this),
                now_utc()
            );
            print!("{}", write_hashed(&url, &key, "lab_log", &json));
        }
        "hash" => {
            if rest.is_empty() {
                eprintln!("usage: lab hash <json>");
                exit(2);
            }
            match serde_json::from_str::<Value>(&rest[0]) {
                Ok(v) => println!("{}", content_hash(&v)),
                Err(e) => {
                    eprintln!("lab: invalid JSON: {}", e);
                    exit(2);
                }
            }
        }
        "conformance" => {
            if rest.is_empty() {
                eprintln!("usage: lab conformance <json>");
                exit(2);
            }
            conformance(&rest[0]);
        }
        "whoami" => {
            let who = hostname();
            println!("{}", who);
            eprintln!(
                "who={} · uid={} · home={}",
                who,
                shell_value(&["id", "-u"], "?"),
                home()
            );
        }
        "tail" => {
            let (url, key) = load_creds();
            let n = rest
                .first()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(20);
            print_tail(&url, &key, n);
        }
        "ping" => {
            let (url, key) = load_creds();
            let (ok, code, secs) = ledger_ping(&url, &key);
            if ok {
                println!("ledger OK  ({}s)  {}", secs, url);
            } else {
                println!("ledger DOWN  (HTTP {})  {}", code, url);
                exit(1);
            }
        }
        "scan" => {
            let (url, key) = load_creds();
            cmd_scan(&url, &key, rest.first().map(|s| s.as_str()));
        }
        "export" | "report" => {
            let (url, key) = load_creds();
            cmd_export(&url, &key);
        }
        "validate" => {
            cmd_validate(rest);
        }
        "judge" => {
            let (url, key) = load_creds();
            cmd_judge(&url, &key);
        }
        "radar" => {
            let (url, key) = load_creds();
            cmd_radar(&url, &key);
        }
        "audit" => {
            let (url, key) = load_creds();
            cmd_audit(&url, &key);
        }
        "converge" => {
            let (url, key) = load_creds();
            let apply = rest.iter().any(|a| a == "--apply");
            cmd_converge(&url, &key, apply);
        }
        "manhattan-sync" => {
            let (url, key) = load_creds();
            cmd_manhattan_sync(&url, &key, rest);
        }
        "notify" => {
            if rest.is_empty() {
                eprintln!(
                    "usage: lab notify <subject> [body]   (life-and-death only — emails NOTIFY_TO)"
                );
                exit(2);
            }
            let subject = &rest[0];
            let body = rest.get(1).cloned().unwrap_or_else(|| subject.clone());
            cmd_notify(subject, &body);
        }
        "send" => {
            // lab send <did> <this> --to <hash>[,<hash>...] [--data <json>]
            //          [--status <s>] [--as <who>] [--if-not <x>] [--if-doubt <x>]
            // The canonical *addressed* write: emit + data.if_ok = [frequencies].
            // A row whose if_ok carries real content-hashes is DELIVERED (the trigger
            // taps each named channel). "send to johnny" still registers, wakes no one.
            let (mut did, mut this) = (None, None);
            let (mut to, mut data_arg, mut who_override) = (None, None, None);
            let (mut if_not, mut if_doubt) = (None, None);
            let mut status = "sent".to_string();
            let mut i = 0;
            while i < rest.len() {
                match rest[i].as_str() {
                    "--to" => {
                        to = rest.get(i + 1).cloned();
                        i += 2;
                    }
                    "--data" => {
                        data_arg = rest.get(i + 1).cloned();
                        i += 2;
                    }
                    "--status" => {
                        status = rest.get(i + 1).cloned().unwrap_or(status);
                        i += 2;
                    }
                    "--as" => {
                        who_override = rest.get(i + 1).cloned();
                        i += 2;
                    }
                    "--if-not" => {
                        if_not = rest.get(i + 1).cloned();
                        i += 2;
                    }
                    "--if-doubt" => {
                        if_doubt = rest.get(i + 1).cloned();
                        i += 2;
                    }
                    s => {
                        if did.is_none() {
                            did = Some(s.to_string());
                        } else if this.is_none() {
                            this = Some(s.to_string());
                        }
                        i += 1;
                    }
                }
            }
            let (did, this, to) = match (did, this, to) {
                (Some(d), Some(t), Some(to)) => (d, t, to),
                _ => {
                    eprintln!("usage: lab send <did> <this> --to <hash>[,<hash>...] [--data <json>] [--as <who>] [--status <s>] [--if-not <x>] [--if-doubt <x>]");
                    exit(2);
                }
            };
            let (url, key) = load_creds();
            // Addressing rides in the canonical if_ok slot: comma-separated frequencies.
            let if_ok = to
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(",");
            let who = who_override.unwrap_or_else(hostname);
            let when = now_utc();
            let extra = data_arg.map(|d| {
                let payload = serde_json::from_str::<Value>(&d).unwrap_or(Value::String(d));
                let mut a = serde_json::Map::new();
                a.insert("payload".into(), payload);
                a
            });
            let (receipt, c_hash, t_hash) = canonical_receipt(
                &who,
                &did,
                &this,
                &when,
                "",
                &if_ok,
                &if_doubt.unwrap_or_default(),
                &if_not.unwrap_or_default(),
                &status,
                extra,
            );
            let row = act_row(&receipt, &c_hash, &t_hash);
            let (resp, code) = write_act_row(&url, &key, &row);
            if !(200..300).contains(&code) {
                eprintln!("lab send: ledger rejected (HTTP {}): {}", code, resp.trim());
                exit(1);
            }
            eprintln!("   sent → logline_acts [canonical] {}", c_hash);
            print!("{}", resp);
        }
        "register" => {
            // lab register <name> <data-json>   (data = {"kind":..,"wake":{..}})
            // Writes an awaken-spec row and prints the resulting FREQUENCY (its
            // content_hash) — the address others put in `--to` to reach this entity.
            if rest.len() < 2 {
                eprintln!("usage: lab register <name> <data-json>   (data = {{\"kind\":..,\"wake\":{{..}}}})");
                exit(2);
            }
            let name = rest[0].clone();
            let data_val = match serde_json::from_str::<Value>(&rest[1]) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("lab register: invalid data JSON: {}", e);
                    exit(2);
                }
            };
            let (url, key) = load_creds();
            let when = now_utc();
            // The wake-spec lives in AUX (extra info) -- folded into the act so the
            // DB projects it into the aux column; never part of the nine slots.
            let mut extra = serde_json::Map::new();
            extra.insert("spec".into(), data_val);
            let (receipt, c_hash, t_hash) = canonical_receipt(
                &name,
                "awaken-spec",
                &name,
                &when,
                "",
                "",
                "",
                "",
                "registered",
                Some(extra),
            );
            let row = act_row(&receipt, &c_hash, &t_hash);
            let (resp, code) = write_act_row(&url, &key, &row);
            if !(200..300).contains(&code) {
                eprintln!("lab register: ledger rejected (HTTP {}): {}", code, resp.trim());
                exit(1);
            }
            println!("frequency: {}", c_hash);
            eprintln!("   registered → logline_acts [canonical] {}", c_hash);
        }
        "mine" => {
            // lab mine <frequency> [limit]  -> rows addressed to this frequency
            // (data.if_ok contains it). The cold-start / missed-tap pull; the receiver
            // derives handled-ness from `awakened` receipts (the ledger is append-only).
            if rest.is_empty() {
                eprintln!("usage: lab mine <frequency-hash> [limit]");
                exit(2);
            }
            let freq = &rest[0];
            let limit = rest
                .get(1)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(50);
            let (url, key) = load_creds();
            // if_ok is the canonical addressing slot; a row names this frequency
            // when its if_ok string contains the hash.
            let query = format!(
                "if_ok=ilike.*{}*&order=inserted_at.desc&limit={}",
                pct(freq),
                limit
            );
            print!("{}", rest_read(&url, &key, LEDGER, &query));
        }
        "commands" | "list" => {
            let builtins = [
                "read",
                "write",
                "emit",
                "heartbeat",
                "scan",
                "export",
                "report",
                "validate",
                "judge",
                "radar",
                "audit",
                "converge",
                "manhattan-sync",
                "hash",
                "conformance",
                "tail",
                "ping",
                "whoami",
                "notify",
                "send",
                "register",
                "mine",
                "commands",
                "new-command",
            ];
            println!("builtins:");
            for b in builtins {
                println!("  {}", b);
            }
            let dir = commands_dir();
            let mut plugins = Vec::new();
            if let Ok(rd) = fs::read_dir(&dir) {
                for e in rd.flatten() {
                    if e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        plugins.push(e.file_name().to_string_lossy().to_string());
                    }
                }
            }
            plugins.sort();
            println!("\nplugins ({}):", dir.display());
            if plugins.is_empty() {
                println!("  (none yet — make one with: lab new-command <name>)");
            }
            for p in plugins {
                println!("  {}", p);
            }
        }
        "new-command" => {
            if rest.is_empty() {
                eprintln!("usage: lab new-command <name>");
                exit(2);
            }
            scaffold_command(&rest[0]);
            // The system logs its own growth, through the one path (best-effort:
            // the file is the real work; the ledger record never blocks creation).
            if let Some((url, key)) = try_creds() {
                let mut m = serde_json::Map::new();
                m.insert("who".into(), Value::String(hostname()));
                m.insert("did".into(), Value::String("new-command".into()));
                m.insert("this".into(), Value::String(rest[0].clone()));
                m.insert("when".into(), Value::String(now_utc()));
                m.insert("status".into(), Value::String("created".into()));
                let json = serde_json::to_string(&Value::Object(m)).unwrap_or_default();
                let _ = write_hashed(&url, &key, "lab_log", &json);
            }
        }
        "help" | "-h" | "--help" => usage(),
        other => {
            let (url, key) = load_creds();
            run_external(other, rest, &url, &key);
        }
    }
}
