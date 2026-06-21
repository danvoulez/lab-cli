#!/usr/bin/env bash
set -euo pipefail

STACK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_INTERVAL="300"
HOST_ARG=""
INTERVAL="$DEFAULT_INTERVAL"
CLEANUP=""
SKIP_RADAR=0
SKIP_MANHATTAN=0
NO_USR_LOCAL=0
VERIFY_AFTER=1
CLI_SOURCE="${LAB_CLI_SOURCE:-}"
MANHATTAN_SOURCE="${MANHATTAN_SOURCE:-}"

usage() {
  printf '%s\n' "usage: install.sh --host LAB_256|LAB_8GB|LAB_512 [--cleanup delete|review] [--interval 300] [--cli-source PATH] [--manhattan-source PATH] [--skip-radar] [--skip-manhattan] [--no-usr-local] [--no-verify]"
}

die() {
  printf 'lab-stack install: %s\n' "$*" >&2
  exit 1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --host) HOST_ARG="${2:-}"; shift 2;;
    --interval) INTERVAL="${2:-}"; shift 2;;
    --cleanup) CLEANUP="${2:-}"; shift 2;;
    --cli-source) CLI_SOURCE="${2:-}"; shift 2;;
    --manhattan-source) MANHATTAN_SOURCE="${2:-}"; shift 2;;
    --skip-radar) SKIP_RADAR=1; shift;;
    --skip-manhattan) SKIP_MANHATTAN=1; shift;;
    --no-usr-local) NO_USR_LOCAL=1; shift;;
    --no-verify) VERIFY_AFTER=0; shift;;
    -h|--help) usage; exit 0;;
    *) usage; die "unknown argument: $1";;
  esac
done

infer_host() {
  local name
  name="$(/usr/sbin/scutil --get LocalHostName 2>/dev/null || hostname -s || true)"
  case "$(printf '%s' "$name" | tr '[:upper:]' '[:lower:]')" in
    *lab*256*|*256*) printf '%s\n' LAB_256;;
    *lab*8gb*|*8gb*) printf '%s\n' LAB_8GB;;
    *lab*512*|*512*) printf '%s\n' LAB_512;;
    *) return 1;;
  esac
}

if [ -z "$HOST_ARG" ]; then
  HOST_ARG="$(infer_host || true)"
fi

case "$HOST_ARG" in
  LAB_256|LAB_8GB|LAB_512) ;;
  "") usage; die "missing --host and could not infer this machine";;
  *) usage; die "invalid --host: $HOST_ARG";;
esac

case "$INTERVAL" in
  ''|*[!0-9]*) die "--interval must be seconds";;
esac

resolve_cli_source() {
  if [ -n "$CLI_SOURCE" ]; then printf '%s\n' "$CLI_SOURCE"; return 0; fi
  if [ -f "$STACK_ROOT/../Cargo.toml" ]; then cd "$STACK_ROOT/.." && pwd; return 0; fi
  die "cannot find CLI source; pass --cli-source"
}

resolve_manhattan_source() {
  local candidates
  if [ -n "$MANHATTAN_SOURCE" ]; then printf '%s\n' "$MANHATTAN_SOURCE"; return 0; fi
  candidates=(
    "$STACK_ROOT/../../manhattan"
    "$STACK_ROOT/manhattan"
    "$HOME/manhattan/project-manhattan-v2"
    "$HOME/MANHATTAN/project-manhattan-v2"
    "/Users/ubl-ops/MANHATTAN/project-manhattan-v2"
  )
  local p
  for p in "${candidates[@]}"; do
    if [ -f "$p/install/install.sh" ] && [ -f "$p/src/manhattan.py" ]; then
      cd "$p" && pwd
      return 0
    fi
  done
  die "cannot find Manhattan source; pass --manhattan-source or bundle it next to cli"
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

[ "$(uname -s)" = "Darwin" ] || die "this installer targets macOS"
require_cmd cargo
require_cmd python3
require_cmd launchctl

CLI_SOURCE="$(resolve_cli_source)"
[ -f "$CLI_SOURCE/Cargo.toml" ] || die "CLI source missing Cargo.toml: $CLI_SOURCE"
[ -f "$CLI_SOURCE/src/main.rs" ] || die "CLI source missing src/main.rs: $CLI_SOURCE"

printf 'lab-stack install\n'
printf '  host: %s\n' "$HOST_ARG"
printf '  cli: %s\n' "$CLI_SOURCE"

printf '\n[1/4] building lab CLI\n'
(cd "$CLI_SOURCE" && cargo build --release --locked)

LAB_BIN="$CLI_SOURCE/target/release/lab"
[ -x "$LAB_BIN" ] || die "lab binary was not built: $LAB_BIN"
mkdir -p "$HOME/.cargo/bin"
ln -sf "$LAB_BIN" "$HOME/.cargo/bin/lab"
printf 'linked %s -> %s\n' "$HOME/.cargo/bin/lab" "$LAB_BIN"

if [ "$NO_USR_LOCAL" -eq 0 ]; then
  if [ -w /usr/local/bin ]; then
    ln -sf "$LAB_BIN" /usr/local/bin/lab
    printf 'linked /usr/local/bin/lab -> %s\n' "$LAB_BIN"
  else
    sudo ln -sf "$LAB_BIN" /usr/local/bin/lab
    printf 'linked /usr/local/bin/lab -> %s\n' "$LAB_BIN"
  fi
fi

mkdir -p "$HOME/.radar"
if [ ! -f "$HOME/.radar/sync.env" ]; then
  printf 'notice: ~/.radar/sync.env is missing; copy lab-stack/templates/sync.env.example and fill it before ledger verification\n' >&2
fi
if [ ! -f "$HOME/.radar/.notify.env" ]; then
  printf 'notice: ~/.radar/.notify.env is missing; email notifications will not send until configured\n' >&2
fi

if [ "$SKIP_RADAR" -eq 0 ]; then
  printf '\n[2/4] installing Radar payload\n'
  cp "$STACK_ROOT/radar/install-lab-radar.sh" "$HOME/.radar/install-lab-radar.sh"
  cp "$STACK_ROOT/radar/radar-scan.sh" "$HOME/.radar/radar-scan.sh"
  cp "$STACK_ROOT/radar/radar-judge.py" "$HOME/.radar/radar-judge.py"
  cp "$STACK_ROOT/radar/radar-report.sh" "$HOME/.radar/radar-report.sh"
  cp "$STACK_ROOT/radar/radar-export.py" "$HOME/.radar/radar-export.py"
  cp "$STACK_ROOT/radar/radar-validate.py" "$HOME/.radar/radar-validate.py"
  cp "$STACK_ROOT/radar/success.json" "$HOME/.radar/success.json"
  chmod 755 "$HOME/.radar/install-lab-radar.sh" "$HOME/.radar/radar-scan.sh" "$HOME/.radar/radar-judge.py" "$HOME/.radar/radar-report.sh" "$HOME/.radar/radar-export.py" "$HOME/.radar/radar-validate.py"
  LAB_BIN="$LAB_BIN" "$HOME/.radar/install-lab-radar.sh" "$INTERVAL"
else
  printf '\n[2/4] skipped Radar\n'
fi

if [ "$SKIP_MANHATTAN" -eq 0 ]; then
  MANHATTAN_SOURCE="$(resolve_manhattan_source)"
  printf '\n[3/4] installing Manhattan from %s\n' "$MANHATTAN_SOURCE"
  [ -f "$MANHATTAN_SOURCE/install/install.sh" ] || die "Manhattan installer missing"
  args=(--host "$HOST_ARG" --allow-unresolved-machine-facts)
  if [ -n "$CLEANUP" ]; then args+=(--cleanup "$CLEANUP"); fi
  sudo "$MANHATTAN_SOURCE/install/install.sh" "${args[@]}"
else
  printf '\n[3/4] skipped Manhattan\n'
fi

printf '\n[4/4] install complete\n'
if [ "$VERIFY_AFTER" -eq 1 ]; then
  "$STACK_ROOT/verify.sh" --host "$HOST_ARG"
fi
