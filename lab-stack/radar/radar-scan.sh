#!/bin/bash
# radar-scan.sh -- primordial whole-computer self-scanner.
#
# Principle: survives Supabase, the network, python, and the UI repo all being
# gone. Uses only the shell, coreutils, and macOS syscall tools. Each box scans
# ONLY itself, in phases (one subject per tick) so no single slow/failing pass
# can take the others down. Truth lives in local JSON under $RADAR_DIR. Cloud
# sync is a separate, best-effort, optional phase.
#
# "The whole computer" = a fixed set of SUBJECTS, each backed by an EXHAUSTIVE
# source (not a hand-picked list). Every finding is emitted in one uniform item
# shape {id,label,detail,state,size_bytes} so the UI can give each a button.
# Each slice carries a coverage manifest {source,count} = honesty law: we report
# what we looked at and how many, never just "OK".
#
# Usage:
#   radar-scan.sh next        run next subject in rotation, rebuild status, sync (launchd)
#   radar-scan.sh all         run every subject once (no sync, normal caps)
#   radar-scan.sh full        run every subject once (no sync, uncapped export scan)
#   radar-scan.sh <subject>   system|storage|services|processes|schedules|network|
#                             apps|packages|repos|secrets|config|junk|files|
#                             cloud|accounts|runtimes|unknown|sync

RADAR_DIR="${RADAR_DIR:-$HOME/.radar}"
SUBJECTS="system storage files services processes schedules network apps packages repos secrets config cloud accounts runtimes junk unknown"
FULL_SCAN="${RADAR_FULL_SCAN:-0}"
mkdir -p "$RADAR_DIR" 2>/dev/null

now()  { date -u +%Y-%m-%dT%H:%M:%SZ; }
now_ms() {
  python3 - <<'PY' 2>/dev/null || perl -MTime::HiRes=time -e 'printf "%.0f\n", time()*1000' 2>/dev/null || printf '%s000\n' "$(date +%s)"
import time
print(int(time.time() * 1000))
PY
}
host() { scutil --get LocalHostName 2>/dev/null || hostname -s; }
jstr() { printf '%s' "${1:-}" | tr -d '\n\r' | LC_ALL=C sed 's/\\/\\\\/g; s/"/\\"/g'; }
limit_lines() { local n="$1"; if [ "$FULL_SCAN" = "1" ]; then cat; else head -n "$n"; fi; }
base_name() { local x="${1:-}"; x="${x%/}"; printf '%s' "${x##*/}"; }
cap_json() { if [ "$FULL_SCAN" = "1" ]; then printf 'null'; else printf '%s' "$1"; fi; }
cap_hit_json() {
  local count="${1:-0}" cap="${2:-0}"
  if [ "$FULL_SCAN" = "1" ] || [ "$cap" = "0" ]; then printf 'false'; return; fi
  if [ "${count:-0}" -ge "$cap" ] 2>/dev/null; then printf 'true'; else printf 'false'; fi
}

write_slice() {
  local name="$1" tmp
  tmp="$(mktemp "$RADAR_DIR/.tmp.XXXXXX")" || return 1
  cat > "$tmp"; mv -f "$tmp" "$RADAR_DIR/$name.json"
}
mark() {
  printf '{"phase":"%s","status":"%s","note":"%s","at":"%s"}' \
    "$(jstr "$1")" "$(jstr "$2")" "$(jstr "${3:-}")" "$(now)" > "$RADAR_DIR/.last.$1"
}

# --- uniform item emitters (used inside each subject's slice subshell) ---
ITEMS_FIRST=1
SCAN_PHASE_STARTED_AT=""
SCAN_PHASE_START_MS=""
slice_open() { ITEMS_FIRST=1; printf '{"box":"%s","subject":"%s","scanned_at":"%s","items":[' "$(jstr "$(host)")" "$1" "$(now)"; }
item() {  # item <id> <label> <detail> <state> [size_bytes]
  [ "$ITEMS_FIRST" -eq 0 ] && printf ','
  printf '{"id":"%s","label":"%s","detail":"%s","state":"%s","size_bytes":%s}' \
    "$(jstr "$1")" "$(jstr "$2")" "$(jstr "$3")" "$(jstr "$4")" "${5:-null}"
  ITEMS_FIRST=0
}
slice_close() {
  local source="$1" count="${2:-0}" cap="${3:-null}" cap_hit="${4:-false}" ended_at ended_ms duration_ms
  ended_at="$(now)"
  ended_ms="$(now_ms)"
  duration_ms="null"
  if [ -n "$SCAN_PHASE_START_MS" ]; then
    duration_ms="$(( ended_ms - SCAN_PHASE_START_MS ))"
  fi
  printf '],"coverage":{"source":"%s","count":%s},"_meta":{"started_at":"%s","ended_at":"%s","duration_ms":%s,"source_command":"%s","argv":["%s"],"cap":%s,"cap_hit":%s}}\n' \
    "$(jstr "$source")" "$count" "$(jstr "$SCAN_PHASE_STARTED_AT")" "$(jstr "$ended_at")" "$duration_ms" \
    "$(jstr "$source")" "$(jstr "$source")" "$cap" "$cap_hit"
}

phase_system() {
  local model cores ram os boot line total used avail st
  model="$(sysctl -n hw.model 2>/dev/null)"
  cores="$(sysctl -n hw.ncpu 2>/dev/null)"
  ram="$(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1073741824 ))"
  os="$(sw_vers -productVersion 2>/dev/null) ($(sw_vers -buildVersion 2>/dev/null))"
  boot="$(uptime | sed 's/.*up //; s/,[^,]*user.*//')"
  line="$(df -g / 2>/dev/null | tail -1)"
  total="$(printf '%s' "$line" | awk '{print $2}')"; used="$(printf '%s' "$line" | awk '{print $3}')"; avail="$(printf '%s' "$line" | awk '{print $4}')"
  st=ok; [ "${avail:-100}" -lt 25 ] 2>/dev/null && st=warn; [ "${avail:-100}" -lt 10 ] 2>/dev/null && st=alert
  { slice_open system
    item "system:host"   "$(host)"  "$model · $cores cores · ${ram}GB" ok null
    item "system:os"     "macOS"    "$os" ok null
    item "system:uptime" "uptime"   "$boot" ok null
    item "system:disk"   "disk /"   "${avail}GB free of ${total}GB" "$st" "$(( ${avail:-0} * 1073741824 ))"
    slice_close "sysctl,sw_vers,df" 4 null false
  } | write_slice system; mark system ok
}

phase_storage() {
  local n=0 t
  { slice_open storage
    for t in "$HOME" /Applications /usr/local /opt; do
      [ -d "$t" ] || continue
      while IFS=$'\t' read -r sz path; do
        [ -z "$path" ] && continue
        item "storage:$path" "$(base_name "$path")" "$path" info "$(( ${sz:-0} * 1024 ))"; n=$((n+1))
      done < <(du -d 1 -k "$t" 2>/dev/null | sort -rn | limit_lines 12 | awk -F'\t' '{print $1"\t"$2}')
    done
    slice_close "du -d1 over home,/Applications,/usr/local,/opt (uncapped when full)" "$n" "$(cap_json 12)" "$(cap_hit_json "$n" 12)"
  } | write_slice storage; mark storage ok
}

phase_files() {
  local n=0 root depth
  { slice_open files
    for root in "$HOME" "$HOME/Desktop" "$HOME/Documents" "$HOME/Downloads" "$HOME/Library" /Applications /Users /Volumes /usr/local /opt /etc /var; do
      [ -e "$root" ] || continue
      depth=1
      case "$root" in "$HOME/Library"|/var) depth=1;; *) depth=1;; esac
      while IFS= read -r path; do
        [ -z "$path" ] && continue
        local kind="file"
        [ -d "$path" ] && kind="dir"
        [ -L "$path" ] && kind="symlink"
        item "files:$path" "$(base_name "$path")" "$path" "$kind" null; n=$((n+1))
      done < <(find "$root" -maxdepth "$depth" -mindepth 1 2>/dev/null | sort | limit_lines 80)
    done
    slice_close "find selected roots -maxdepth 1 (uncapped when full)" "$n" "$(cap_json 80)" "$(cap_hit_json "$n" 80)"
  } | write_slice files; mark files ok
}

phase_services() {
  local n=0 plists=0 d f
  { slice_open services
    while IFS=$'\t' read -r code label; do
      [ -z "$label" ] && continue
      local st=ok; [ "$code" != "0" ] && [ "$code" != "-" ] && st=fail
      item "services:$label" "$label" "exit $code" "$st" null; n=$((n+1))
    done < <(launchctl list 2>/dev/null | awk 'NR>1 && $3!="" {print $2"\t"$3}' | { if [ "$FULL_SCAN" = "1" ]; then cat; else grep -Ei 'minilab|voulezvous|manhattan|ciclo|actgraph|cloudflared|tv\.|work\.'; fi; } )
    for d in "$HOME/Library/LaunchAgents" /Library/LaunchAgents /Library/LaunchDaemons; do
      [ -d "$d" ] || continue
      plists=$(( plists + $(ls -1 "$d" 2>/dev/null | grep -c '\.plist$') ))
    done
    slice_close "launchctl list (minilab-scoped unless full) + $plists plists on disk" "$n" null false
  } | write_slice services; mark services ok
}

phase_processes() {
  local n=0
  { slice_open processes
    while read -r pcpu pmem comm; do
      [ -z "$comm" ] && continue
      item "processes:$comm" "$(base_name "$comm")" "cpu ${pcpu}% mem ${pmem}%" info null; n=$((n+1))
    done < <(ps -axo pcpu,pmem,comm 2>/dev/null | sort -rn | limit_lines 15)
    slice_close "ps -axo (top cpu; uncapped when full)" "$n" "$(cap_json 15)" "$(cap_hit_json "$n" 15)"
  } | write_slice processes; mark processes ok
}

phase_schedules() {
  local n=0 ghosts=0 line cmd
  { slice_open schedules
    while read -r line; do
      [ -z "$line" ] && continue
      case "$line" in \#*) continue;; esac
      local st=ok note="cron"
      cmd="$(printf '%s' "$line" | awk '{for(i=6;i<=NF;i++)printf $i" "}')"
      if printf '%s' "$line" | grep -qE 'ubl-core-forever|AI-NRF1|Projects/'; then st=ghost; ghosts=$((ghosts+1)); fi
      item "schedules:$(printf '%s' "$line" | md5 2>/dev/null || printf '%s' "$line")" "cron job" "$cmd" "$st" null; n=$((n+1))
    done < <(crontab -l 2>/dev/null)
    slice_close "crontab -l ($ghosts ghost)" "$n" null false
  } | write_slice schedules; mark schedules ok
}

phase_network() {
  local n=0
  { slice_open network
    while read -r cmd pid user fd type dev sz node name; do
      [ "$type" = "TCP" ] || [ -z "$name" ] && true
      case "$name" in *LISTEN*|*:*) : ;; *) continue;; esac
      item "network:$name" "$name" "$cmd (pid $pid)" listen null; n=$((n+1))
    done < <(lsof -nP -iTCP -sTCP:LISTEN 2>/dev/null | awk 'NR>1{print $1" "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9}' | limit_lines 30)
    slice_close "lsof -iTCP -sTCP:LISTEN" "$n" "$(cap_json 30)" "$(cap_hit_json "$n" 30)"
  } | write_slice network; mark network ok
}

phase_apps() {
  local n=0 d
  { slice_open apps
    for d in /Applications "$HOME/Applications"; do
      [ -d "$d" ] || continue
      while read -r app; do
        [ -z "$app" ] && continue
        item "apps:$d/$app" "${app%.app}" "$d" info null; n=$((n+1))
      done < <(ls -1 "$d" 2>/dev/null | grep '\.app$' | limit_lines 60)
    done
    slice_close "/Applications + ~/Applications" "$n" "$(cap_json 60)" "$(cap_hit_json "$n" 60)"
  } | write_slice apps; mark apps ok
}

phase_packages() {
  local n=0 p
  { slice_open packages
    if command -v brew >/dev/null 2>&1; then
      while read -r p; do [ -z "$p" ] && continue; item "packages:brew:$p" "$p" "homebrew leaf" info null; n=$((n+1)); done < <(brew leaves 2>/dev/null | limit_lines 60)
    fi
    for p in "$HOME/.cargo/bin" "$HOME/.local/bin"; do
      [ -d "$p" ] || continue
      while read -r b; do [ -z "$b" ] && continue; item "packages:$p/$b" "$b" "$p" info null; n=$((n+1)); done < <(ls -1 "$p" 2>/dev/null | limit_lines 40)
    done
    slice_close "brew leaves + cargo/.local bins" "$n" "$(cap_json 60)" "$(cap_hit_json "$n" 60)"
  } | write_slice packages; mark packages ok
}

phase_repos() {
  local n=0 g dir br dirty remote
  { slice_open repos
    while read -r g; do
      [ -z "$g" ] && continue
      dir="$(dirname "$g")"
      br="$(git -C "$dir" branch --show-current 2>/dev/null)"
      dirty="$(git -C "$dir" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
      remote="$(git -C "$dir" remote get-url origin 2>/dev/null)"
      local st=ok; [ "${dirty:-0}" -gt 0 ] 2>/dev/null && st=dirty
      item "repos:$dir" "$(base_name "$dir")" "branch $br · $dirty uncommitted · $remote" "$st" null; n=$((n+1))
    done < <(find "$HOME" -maxdepth 4 -name .git -type d 2>/dev/null | limit_lines 25)
    slice_close "find ~ -maxdepth 4 -name .git (uncapped when full)" "$n" "$(cap_json 25)" "$(cap_hit_json "$n" 25)"
  } | write_slice repos; mark repos ok
}

phase_secrets() {
  local n=0 f
  { slice_open secrets
    if [ -d "$HOME/.ssh" ]; then
      while read -r f; do [ -z "$f" ] && continue; item "secrets:$f" "$(base_name "$f")" "ssh key/material" info null; n=$((n+1)); done < <(ls -1 "$HOME/.ssh" 2>/dev/null | grep -Ev '\.pub$|known_hosts|config' | limit_lines 30)
    fi
    while read -r f; do [ -z "$f" ] && continue; item "secrets:$f" "$(base_name "$f")" "env file (name only)" info null; n=$((n+1)); done < <(find "$HOME" -maxdepth 5 \( -name '.env' -o -name '*.env' \) 2>/dev/null | limit_lines 20)
    slice_close "~/.ssh keys + env file names (never contents; uncapped when full)" "$n" "$(cap_json 30)" "$(cap_hit_json "$n" 30)"
  } | write_slice secrets; mark secrets ok
}

phase_config() {
  local n=0
  { slice_open config
    while IFS=$'\t' read -r sz path; do
      [ -z "$path" ] && continue
      item "config:$path" "$(base_name "$path")" "$path" info "$(( ${sz:-0} * 1024 ))"; n=$((n+1))
    done < <(du -d 0 -k "$HOME"/.[a-zA-Z]* 2>/dev/null | sort -rn | limit_lines 25 | awk -F'\t' '{print $1"\t"$2}')
    slice_close "du of ~/.* dotdirs (uncapped when full)" "$n" "$(cap_json 25)" "$(cap_hit_json "$n" 25)"
  } | write_slice config; mark config ok
}

phase_cloud() {
  local n=0 root
  { slice_open cloud
    for root in "$HOME/Library/CloudStorage" "$HOME/Library/Application Support/Google/DriveFS" "$HOME/Library/Mobile Documents" "$HOME/Library/Application Support/FileProvider"; do
      [ -e "$root" ] || continue
      while IFS=$'\t' read -r sz path; do
        [ -z "$path" ] && continue
        item "cloud:$path" "$(base_name "$path")" "$path" info "$(( ${sz:-0} * 1024 ))"; n=$((n+1))
      done < <(du -d 1 -k "$root" 2>/dev/null | sort -rn | limit_lines 40 | awk -F'\t' '{print $1"\t"$2}')
    done
    slice_close "CloudStorage, DriveFS, iCloud Mobile Documents, FileProvider (uncapped when full)" "$n" "$(cap_json 40)" "$(cap_hit_json "$n" 40)"
  } | write_slice cloud; mark cloud ok
}

phase_accounts() {
  local n=0
  { slice_open accounts
    while read -r user uid home shell; do
      [ -z "$user" ] && continue
      item "accounts:user:$user" "$user" "uid $uid · home $home · shell $shell" info null; n=$((n+1))
    done < <(dscl . -list /Users UniqueID 2>/dev/null | awk '$2 >= 500 {print $1" "$2}' | while read -r u id; do printf '%s %s %s %s\n' "$u" "$id" "$(dscl . -read "/Users/$u" NFSHomeDirectory 2>/dev/null | awk '{print $2}')" "$(dscl . -read "/Users/$u" UserShell 2>/dev/null | awk '{print $2}')"; done | limit_lines 50)
    slice_close "dscl local users uid>=500 (uncapped when full)" "$n" "$(cap_json 50)" "$(cap_hit_json "$n" 50)"
  } | write_slice accounts; mark accounts ok
}

phase_runtimes() {
  local n=0 name path ver
  { slice_open runtimes
    for name in lab python3 node npm cargo rustc brew git docker colima cloudflared; do
      path="$(command -v "$name" 2>/dev/null || true)"
      [ -z "$path" ] && continue
      ver="$("$path" --version 2>/dev/null | head -1)"
      item "runtimes:$name" "$name" "$path · $ver" info null; n=$((n+1))
    done
    for path in "$HOME/cli" "$HOME/manhattan" "$HOME/engine-park" "$HOME/app-park" /usr/local/project-manhattan "$HOME/.radar"; do
      [ -e "$path" ] || continue
      item "runtimes:path:$path" "$(base_name "$path")" "$path" info null; n=$((n+1))
    done
    slice_close "known runtime binaries and canonical lab paths" "$n" null false
  } | write_slice runtimes; mark runtimes ok
}

phase_junk() {
  local n=0 corpses d
  { slice_open junk
    for d in ZIPFILE from-LAB2566 .Trash quarantine .pm2 .disabled-launchagents "LOOSE FILES"; do
      [ -e "$HOME/$d" ] || continue
      local bytes human
      bytes="$(du -sk "$HOME/$d" 2>/dev/null | awk '{print $1*1024}')"
      human="$(du -sh "$HOME/$d" 2>/dev/null | cut -f1)"
      item "junk:$HOME/$d" "$d" "cold storage / leftovers ($human)" garbage "${bytes:-0}"; n=$((n+1))
    done
    corpses="$(find "$HOME" -maxdepth 3 \( -name '*.bak' -o -name '*.old' -o -name '*.orig' -o -name '*.backup' \) 2>/dev/null | wc -l | tr -d ' ')"
    item "junk:corpses" "corpse files" ".bak/.old/.orig/.backup ($corpses files)" garbage null; n=$((n+1))
    slice_close "known garbage dirs + corpse find" "$n" null false
  } | write_slice junk; mark junk ok
}

phase_unknown() {
  local n=0 name path state known
  { slice_open unknown
    while IFS= read -r path; do
      [ -z "$path" ] && continue
      name="$(base_name "$path")"
      known=0
      case "$name" in Applications|Desktop|Documents|Downloads|Library|Movies|Music|Pictures|Public|cli|manhattan|engine-park|app-park|.radar|.ssh|.config|.cargo|.local|.Trash) known=1;; esac
      [ "$known" -eq 1 ] && continue
      state="unknown"
      [ -d "$path" ] && state="unknown_dir"
      [ -f "$path" ] && state="unknown_file"
      item "unknown:$path" "$name" "$path" "$state" null; n=$((n+1))
    done < <(find "$HOME" -maxdepth 1 -mindepth 1 2>/dev/null | sort | limit_lines 200)
    slice_close "top-level home entries not in Radar's known buckets (uncapped when full)" "$n" "$(cap_json 200)" "$(cap_hit_json "$n" 200)"
  } | write_slice unknown; mark unknown ok
}

phase_sync() {
  local cfg="$RADAR_DIR/sync.env" url key
  if [ "${RADAR_DIRECT_SYNC:-0}" != "1" ]; then
    mark sync skip "direct scanner sync disabled; lab owns ledger writes"
    return 0
  fi
  [ -f "$cfg" ] && . "$cfg"
  url="${RADAR_SUPABASE_URL:-}"; key="${RADAR_SUPABASE_KEY:-}"
  if [ -z "$url" ] || [ -z "$key" ]; then mark sync skip "no sync.env (local-only, by design)"; return 0; fi
  local raw payload sub first=1
  raw="{"
  for sub in $SUBJECTS; do
    [ -f "$RADAR_DIR/$sub.json" ] || continue
    [ $first -eq 0 ] && raw="$raw,"
    raw="$raw\"$sub\":$(cat "$RADAR_DIR/$sub.json")"; first=0
  done
  raw="$raw}"
  payload="{\"box\":\"$(jstr "$(host)")\",\"scanned_at\":\"$(now)\",\"raw\":$raw}"
  if curl -s -m 20 -o /dev/null -w '%{http_code}' \
      -X POST "$url/rest/v1/radar_snapshots" \
      -H "apikey: $key" -H "Authorization: Bearer $key" \
      -H "Content-Type: application/json" -H "Prefer: return=minimal" \
      --data "$payload" | grep -q '^2'; then
    mark sync ok
  else
    mark sync error "post failed (local truth intact)"
  fi
}

build_status() {
  local first=1 p
  { printf '{"box":"%s","built_at":"%s","subjects":["%s"],"phases":[' "$(jstr "$(host)")" "$(now)" "$(echo $SUBJECTS | sed 's/ /","/g')"
    for p in $SUBJECTS sync; do
      [ -f "$RADAR_DIR/.last.$p" ] || continue
      [ $first -eq 0 ] && printf ','
      cat "$RADAR_DIR/.last.$p"; first=0
    done
    printf ']}\n'
  } | write_slice status
}

run_phase() {
  SCAN_PHASE_STARTED_AT="$(now)"
  SCAN_PHASE_START_MS="$(now_ms)"
  case "$1" in
    system) phase_system;; storage) phase_storage;; files) phase_files;; services) phase_services;;
    processes) phase_processes;; schedules) phase_schedules;; network) phase_network;;
    apps) phase_apps;; packages) phase_packages;; repos) phase_repos;;
    secrets) phase_secrets;; config) phase_config;; cloud) phase_cloud;;
    accounts) phase_accounts;; runtimes) phase_runtimes;; junk) phase_junk;;
    unknown) phase_unknown;;
    sync) phase_sync;;
    *) echo "radar-scan: unknown subject '$1'" >&2; return 2;;
  esac
}

run_next() {
  local cur arr n
  cur="$(cat "$RADAR_DIR/.cursor" 2>/dev/null)"
  case "$cur" in ''|*[!0-9]*) cur=0;; esac
  arr=($SUBJECTS); n=${#arr[@]}
  [ "$cur" -ge "$n" ] && cur=0
  run_phase "${arr[$cur]}"
  echo $(( (cur + 1) % n )) > "$RADAR_DIR/.cursor"
  build_status
  phase_sync
}

case "${1:-next}" in
  next) run_next;;
  all)  for p in $SUBJECTS; do run_phase "$p"; done; build_status;;
  full|export)
        FULL_SCAN=1
        i=1
        total="$(echo "$SUBJECTS" | wc -w | tr -d ' ')"
        for p in $SUBJECTS; do
          printf '[radar-scan] phase %s/%s: %s\n' "$i" "$total" "$p" >&2
          run_phase "$p"
          i=$((i+1))
        done
        build_status;;
  system|storage|files|services|processes|schedules|network|apps|packages|repos|secrets|config|cloud|accounts|runtimes|junk|unknown|sync)
        run_phase "$1"; build_status;;
  *) echo "usage: radar-scan.sh next|all|full|<subject>|sync" >&2; exit 2;;
esac
