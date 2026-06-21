# `lab` CLI — Founders Guide
> For Dan Voulez · minilab fleet · June 20, 2026  
> Written from the 10-hour Claude Code session in `/Users/ubl-ops/Downloads/ActGraph-packaging`

---

## What `lab` Is

One Rust binary. One rule: **everything in the fleet that touches Supabase goes through `lab`.**

No other process holds credentials. No other process writes to the ledger. Every write is JCS-hashed (RFC 8785, via `serde_jcs`) + sha256, so every row is referenceable by content. The CLI is `$HOME`-portable — it derives your home at runtime, so it runs identically under `ubl-ops` or `danvoulez`. No hardcoded paths.

```
~/cli/          ← the source (Rust, 1 file: src/main.rs)
~/.cargo/bin/lab ← the binary on PATH (symlink)
~/.radar/sync.env ← the normal creds file (RADAR_SUPABASE_URL, RADAR_SUPABASE_KEY)
~/cli/.env       ← optional local override for lab itself
~/cli/commands/ ← plugin scripts (auto-discovered)
```

Ledger: Supabase project `ipfbjwhnhgbxoynqmfxz`, table `public.lab_log`.

---

## Installing on the `danvoulez` Account

### Prerequisites

You need:
- Rust toolchain (`rustup` installed)
- `curl` (already on macOS)
- The creds file

### Step 1 — Copy the creds

From your current session (or any box that has them):

```bash
# Log in as danvoulez (or switch in terminal)
su - danvoulez   # or open a new terminal session as danvoulez

# Create the radar config dir
mkdir -p ~/.radar

# Copy the creds (from ubl-ops, adjust path as needed)
cp /Users/ubl-ops/.radar/sync.env ~/.radar/sync.env
cp /Users/ubl-ops/.radar/.notify.env ~/.radar/.notify.env
chmod 600 ~/.radar/sync.env ~/.radar/.notify.env
```

`sync.env` must contain:
```
RADAR_SUPABASE_URL=https://ipfbjwhnhgbxoynqmfxz.supabase.co
RADAR_SUPABASE_KEY=<anon key>
```

### Step 2 — Copy the CLI source

```bash
cp -r /Users/ubl-ops/cli ~/cli
cd ~/cli
```

### Step 3 — Build

```bash
cd ~/cli
cargo build --release
# Binary lands at ~/cli/target/release/lab
```

All crates are cached locally — builds offline.

### Step 4 — Put it on PATH

```bash
# Symlink to cargo bin (which is already on PATH via ~/.cargo/env)
ln -sf ~/cli/target/release/lab ~/.cargo/bin/lab

# Verify
lab whoami
lab ping
```

`whoami` reads `scutil --get LocalHostName` — it should print `lab-256` (or whatever box you're on).  
`ping` hits the ledger and prints latency. If it returns a number, you're wired.

### Step 5 — Send a heartbeat

```bash
lab heartbeat setup
```

This writes a canonical row to `lab_log` with `did=heartbeat this=setup`, JCS-hashed. Check Supabase or `lab tail` to confirm it landed.

---

## The Fleet Context

You run three Mac mini M1s. All of this applies to all three boxes.

| Box | Location | User | Role |
|-----|----------|------|------|
| lab-256 | Paris | `ubl-ops` → migrating to `danvoulez` | Workstation |
| lab-512 | Lisbon | `danvoulez` | Inference (mistral.rs `:1234`) |
| lab-8gb | Lisbon | `danvoulez` | Auth + control (actgraph `:7000`, passport `:4174`) |

512 and 8gb are on a direct ethernet `/30` cable (`10.88.0.9/10`). From Paris, reach 8gb via `ssh8gb.minilab.work` (Cloudflare SSH). Design is **share-nothing**: each box writes its own acts to the ledger; nobody calls anybody.

---

## What Was Built in This Session

All of this is in `~/cli/src/main.rs` (1684 lines) and proven running on lab-256:

### Core verbs

| Command | What it does |
|---------|-------------|
| `lab emit <did> <this> [json] [--status s]` | Canonical write — auto-stamps who+when, hashes, lands in lab_log |
| `lab write <table> <json>` | Raw write to any table, JCS-hashed |
| `lab read <table> [postgrest-query]` | Read from any table |
| `lab heartbeat [this]` | Writes a heartbeat row (health check for the ledger) |
| `lab tail [n]` | Last N rows from lab_log, newest last |
| `lab ping` | Ledger reachable + latency |
| `lab whoami` | Prints who the CLI stamps (from scutil) |
| `lab hash <json>` | Standalone JCS+sha256 tool |
| `lab conformance <json>` | Advisory verdict (always exit 0, never blocks) |
| `lab commands` | Lists builtins + ~/cli/commands plugins |
| `lab new-command <name>` | Scaffolds a new plugin script |

### Observability verbs (built this session)

| Command | What it does |
|---------|-------------|
| `lab scan [subject]` | Wraps `~/.radar/radar-scan.sh`, registers hashed `did=scan` summary |
| `lab judge` | Wraps `~/.radar/radar-judge.py`, registers hashed `did=judge` verdict |
| `lab radar` | `scan → judge → notify(only if critical)` — observational loop; scheduled repair belongs to Manhattan |
| `lab notify <subject> [body]` | Email to dan@minilab.work via Maileroo SMTP, 12h dedup, life-and-death only |

### Infrastructure verbs

| Command | What it does |
|---------|-------------|
| `lab audit` | Wraps Manhattan read-only audit (30 desired-state items → hashed act), then syncs compact receipts |
| `lab converge [--apply]` | Wraps Manhattan repair — PLAN by default, `--apply` acts, then syncs compact receipts |
| `lab manhattan-sync [n or --all]` | Admits Manhattan receipt files into `lab_log` as compact `did=manhattan.receipt` rows, deduped by receipt hash |

Manhattan still owns local inspection and repair. `lab` owns custody. Receipts stay full-fidelity under `/usr/local/project-manhattan/var/receipts`; the ledger gets compact, hashed summaries so local truth can be proven later without turning Supabase into the primary sensor.

### The hashing guarantee (v0.2.0)

Every write mints `content_hash = JCS-RFC8785 (serde_jcs, the audited crate) + sha256`. Hash covers content via `strip_meta` (drops `id/inserted_at/content_hash/json_canonicalization/hashes`). Three write tiers — printed as feedback, never an error:

```
Registered lab_log [conformant · jcs-rfc8785 + sha256]  <hash>   ← ideal
Registered lab_log [jcs-rfc8785 · hash uncommitted]      <hash>   ← table lacks canon columns
Registered lab_log [raw · wrapped...]                     <hash>   ← non-object, wrapped
```

The only true error is an unreachable ledger.

---

## The Timer (live on lab-256)

`com.minilab.lab-radar` LaunchAgent runs `lab radar` every 300 seconds.

```bash
# Reinstall (the installer is $HOME-portable — works on danvoulez)
~/.radar/install-lab-radar.sh          # default 300s
~/.radar/install-lab-radar.sh 600      # custom interval

# Logs
tail -f ~/.radar/logs/lab-radar.out
tail -f ~/.radar/logs/lab-radar.err

# Check it's running
launchctl list | grep lab-radar
```

**Critical lesson from today:** `lab radar` MUST use phased scan (`radar-scan.sh next`, one subject per tick), NOT `all`. The `storage` subject does `du` over 55 GB and under `ProcessType=Background` it hangs for minutes, piling up processes. The fix: phased + NO `ProcessType=Background`.

**What fires an email:** only a box emergency. Current code escalates when disk free is < 3 GB. Everything else is silent in the ledger. Email is life-and-death only. Extend `box_critical_reason()` over time (peer dark, won't-reboot).

**Retired today (reversible):**
- `com.minilab.radar-scan` → bootout (plist kept)
- `com.minilab.radar-judge` → bootout (plist kept)
- **Kept:** `com.project-manhattan.agent` (the self-healer)

---

## The Migration (ubl-ops → danvoulez on lab-256)

`danvoulez` already exists on lab-256 and is already admin. FileVault is OFF. This is low-risk.

### What to move (survivors only — don't drag 55 GB across)

```
~/cli/                    ← the CLI source
~/.radar/                 ← creds (.notify.env, sync.env) + scripts
~/MANHATTAN/project-manhattan-v2/   ← the self-healer
~/Desktop/mcp-openai-local-control/ ← the MCP host runtime (dead, needs revival)
```

### Ironclad rules

1. **Never delete the account you're logged into.** Remove `ubl-ops` LAST, from a `danvoulez` session.
2. The current Claude runs AS `ubl-ops` — do NOT touch the account from here. Do the danvoulez-side work in a danvoulez terminal.
3. Verify `danvoulez` login works and `lab whoami` returns correct before removing `ubl-ops`.

### Migration sequence

```bash
# 1. Open a terminal as danvoulez (System Settings > Users or su -)
# 2. Run the install steps above (copy creds, build, symlink, verify)
# 3. Copy Manhattan
cp -r /Users/ubl-ops/MANHATTAN ~/MANHATTAN
# 4. Copy radar scripts
cp -r /Users/ubl-ops/.radar ~/ # creds already there; overwrite scripts only
# 5. Install the LaunchAgent under danvoulez
~/.radar/install-lab-radar.sh
# 6. Verify: lab ping + lab tail + lab radar (manual test)
# 7. Only after everything green: delete ubl-ops from danvoulez session
```

---

## What's Next (smallest unblocked actions)

**P0 — Tighten the CLI**:
- CAS content-hash-PK idempotency from actgraph-cas (de-gated)
- Replace any remaining legacy maintenance writer with `lab` custody
- Add direct tests for `lab manhattan-sync` receipt compaction/dedup

**P1 — Complete the migration**:
- Build + verify `lab` under `danvoulez` on lab-256
- Install LaunchAgent under `danvoulez`
- Delete `ubl-ops` from a `danvoulez` session

**P2 — Spread to the fleet**:
- Install `lab` on lab-512 and lab-8gb (one-liner, builds offline)
- `lab radar` on each box writing to the same ledger (share-nothing)
- Revive `com.minilab.host-runtime` (dead since reboot): `cd ~/Desktop/mcp-openai-local-control && bash scripts/deploy-local.sh`

**P3 — Fleet aggregate view**:
- `lab tail --all` (reads from all boxes via ledger)
- Feed into `control.minilab.work` (the control UI already exists on 8gb)

---

## The Inventory of What Exists

| Component | Location | State |
|-----------|----------|-------|
| `lab` CLI (Rust, v0.2.0) | `~/cli/` | ✅ Built, running, on PATH |
| `lab radar` LaunchAgent | `com.minilab.lab-radar` | ✅ Running (300s cadence) |
| `com.minilab.radar-scan` | plist on disk | ⏸ Retired (reversible) |
| `com.minilab.radar-judge` | plist on disk | ⏸ Retired (reversible) |
| `com.project-manhattan.agent` | LaunchAgent | ✅ Running |
| Manhattan daemon | `~/MANHATTAN/project-manhattan-v2/` | ❌ Not deployed (needs sudo) |
| MCP host runtime | `~/Desktop/mcp-openai-local-control/` | ❌ Dead (exit 78) |
| Radar scan script | `~/.radar/radar-scan.sh` | ✅ Running via lab radar |
| Radar judge | `~/.radar/radar-judge.py` | ✅ Running via lab judge |
| Notify | Rust `lab notify` + `~/.radar/.notify.env` | ✅ Tested, working |
| Ledger (Supabase) | `ipfbjwhnhgbxoynqmfxz` | ✅ Live (~376 acts) |
| Control UI | `control.minilab.work` | ✅ Live (demo data) |
| `danvoulez` account on lab-256 | System Settings | ✅ Exists, admin |

---

## The Hard Rules (earned today)

1. **One credential path.** Normal source is `~/.radar/sync.env`; `~/cli/.env` and env vars exist for bootstrap and tests. Nothing else stores Supabase keys.
2. **One write path.** Lab's hashed write path. Plugins call `lab emit`; built-ins call the same Rust kernel, never raw curl outside `lab`.
3. **Local first, ledger second.** Radar scans and Manhattan receipts are local evidence first. `lab` admits compact, hashed records so the fact survives time.
4. **Email is life-and-death only.** Disk < 3GB or self-heal stuck for ~1h. Peer-dark / won't-reboot checks are future extensions, not current code.
5. **Phased scan.** Never `lab radar` with full `all` scan. One subject per tick, rotating.
6. **No Background throttle on I/O work.** LaunchAgent with `ProcessType=Background` + `du` over 55GB = hung processes. Lesson paid for with a stuck timer this session.
7. **`$HOME`-portable.** `~/cli` builds and runs identically on any box under any user. Never hardcode `/Users/ubl-ops/`.
8. **Conformance warns, never blocks.** A write with a bad or missing hash still lands. The system logs its own growth. A missing ledger never blocks the file work.

---

*Source: `~/.claude/projects/-Users-ubl-ops-Downloads-ActGraph-packaging/97eef6e3-f039-4c2a-8f08-adf497020783.jsonl` + memory files. Session ran 2026-06-20, last user prompt at 15:51: "first make a founder's guide starting from how I install the cli on danvoulez account".*
