#!/usr/bin/env bash
set -u

STACK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOST_ARG=""
SKIP_MANHATTAN=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --host) HOST_ARG="${2:-}"; shift 2;;
    --skip-manhattan) SKIP_MANHATTAN=1; shift;;
    -h|--help) printf '%s\n' "usage: verify.sh --host LAB_256|LAB_8GB|LAB_512 [--skip-manhattan]"; exit 0;;
    *) printf 'verify: unknown argument: %s\n' "$1" >&2; exit 2;;
  esac
done

FAILURES=0
PASS_COUNT=0

pass() {
  PASS_COUNT=$((PASS_COUNT + 1))
  printf '[ok] %s\n' "$*"
}

fail() {
  FAILURES=$((FAILURES + 1))
  printf '[fail] %s\n' "$*" >&2
}

check_cmd() {
  local label="$1"; shift
  if "$@" >/tmp/lab-stack-verify.out 2>/tmp/lab-stack-verify.err; then
    pass "$label"
  else
    fail "$label ($(tr '\n' ' ' </tmp/lab-stack-verify.err | sed 's/[[:space:]]*$//'))"
  fi
}

resolve_lab() {
  if [ -n "${LAB_BIN:-}" ] && [ -x "$LAB_BIN" ]; then printf '%s\n' "$LAB_BIN"; return 0; fi
  if [ -x /usr/local/bin/lab ]; then printf '%s\n' /usr/local/bin/lab; return 0; fi
  if [ -x "$HOME/.cargo/bin/lab" ]; then printf '%s\n' "$HOME/.cargo/bin/lab"; return 0; fi
  if command -v lab >/dev/null 2>&1; then command -v lab; return 0; fi
  return 1
}

LAB="$(resolve_lab || true)"
if [ -z "$LAB" ]; then
  fail "lab binary is installed"
else
  pass "lab binary: $LAB"
  check_cmd "lab whoami" "$LAB" whoami
  check_cmd "lab ping reaches ledger" "$LAB" ping
  check_cmd "lab heartbeat setup" "$LAB" heartbeat setup
fi

if [ -x "$HOME/.radar/radar-scan.sh" ]; then
  pass "Radar scanner script installed"
else
  fail "Radar scanner script installed"
fi

if [ -x "$HOME/.radar/radar-judge.py" ]; then
  pass "Radar judge script installed"
  check_cmd "Radar judge syntax" python3 -m py_compile "$HOME/.radar/radar-judge.py"
else
  fail "Radar judge script installed"
fi

if [ -f "$HOME/.radar/success.json" ]; then
  pass "Radar success list installed"
  check_cmd "Radar success list parses" python3 -m json.tool "$HOME/.radar/success.json"
else
  fail "Radar success list installed"
fi

if launchctl print "gui/$(id -u)/com.minilab.lab-radar" >/tmp/lab-stack-verify.out 2>/tmp/lab-stack-verify.err; then
  pass "lab-radar launchd job loaded"
else
  fail "lab-radar launchd job loaded ($(tr '\n' ' ' </tmp/lab-stack-verify.err | sed 's/[[:space:]]*$//'))"
fi

if [ -n "$LAB" ]; then
  check_cmd "lab scan system" "$LAB" scan system
  check_cmd "lab judge" "$LAB" judge
fi

if [ "$SKIP_MANHATTAN" -eq 0 ]; then
  if [ -x /usr/local/project-manhattan/bin/manhattan ]; then
    pass "Manhattan binary installed"
    check_cmd "Manhattan CLI self-check" /usr/local/project-manhattan/bin/manhattan --help
  else
    fail "Manhattan binary installed"
  fi

  if [ -f /usr/local/project-manhattan/src/manhattan.py ]; then
    pass "Manhattan engine installed"
    check_cmd "Manhattan engine syntax" python3 -m py_compile /usr/local/project-manhattan/src/manhattan.py
  else
    fail "Manhattan engine installed"
  fi

  if launchctl print system/com.project-manhattan.daemon >/tmp/lab-stack-verify.out 2>/tmp/lab-stack-verify.err; then
    pass "Manhattan daemon loaded"
  else
    fail "Manhattan daemon loaded ($(tr '\n' ' ' </tmp/lab-stack-verify.err | sed 's/[[:space:]]*$//'))"
  fi

  if launchctl print "gui/$(id -u)/com.project-manhattan.agent" >/tmp/lab-stack-verify.out 2>/tmp/lab-stack-verify.err; then
    pass "Manhattan agent loaded"
  else
    fail "Manhattan agent loaded ($(tr '\n' ' ' </tmp/lab-stack-verify.err | sed 's/[[:space:]]*$//'))"
  fi
fi

printf '\nverify summary: %s passed, %s failed\n' "$PASS_COUNT" "$FAILURES"
[ "$FAILURES" -eq 0 ]

