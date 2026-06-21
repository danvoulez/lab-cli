#!/usr/bin/env bash
set -euo pipefail

STACK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLI_SOURCE="$(cd "$STACK_ROOT/.." && pwd)"
MANHATTAN_SOURCE="${MANHATTAN_SOURCE:-}"

die() {
  printf 'lab-stack pack: %s\n' "$*" >&2
  exit 1
}

resolve_manhattan_source() {
  local candidates p
  if [ -n "$MANHATTAN_SOURCE" ]; then printf '%s\n' "$MANHATTAN_SOURCE"; return 0; fi
  candidates=(
    "$CLI_SOURCE/../MANHATTAN/project-manhattan-v2"
    "$HOME/MANHATTAN/project-manhattan-v2"
    "$HOME/manhattan/project-manhattan-v2"
    "/Users/ubl-ops/MANHATTAN/project-manhattan-v2"
  )
  for p in "${candidates[@]}"; do
    if [ -f "$p/install/install.sh" ] && [ -f "$p/src/manhattan.py" ]; then
      cd "$p" && pwd
      return 0
    fi
  done
  die "cannot find Manhattan source; set MANHATTAN_SOURCE"
}

MANHATTAN_SOURCE="$(resolve_manhattan_source)"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
DIST_DIR="$CLI_SOURCE/dist"
WORK_DIR="$DIST_DIR/lab-stack-$STAMP"
ARCHIVE="$DIST_DIR/lab-stack-$STAMP.tar.gz"

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR/cli" "$WORK_DIR/manhattan"

rsync -a --delete \
  --exclude '.git' \
  --exclude '.DS_Store' \
  --exclude '__pycache__' \
  --exclude '*.pyc' \
  --exclude 'target' \
  --exclude 'dist' \
  "$CLI_SOURCE/" "$WORK_DIR/cli/"

rsync -a --delete \
  --exclude '.git' \
  --exclude '.DS_Store' \
  --exclude '__pycache__' \
  --exclude '*.pyc' \
  "$MANHATTAN_SOURCE/" "$WORK_DIR/manhattan/"

(cd "$DIST_DIR" && tar -czf "$ARCHIVE" "lab-stack-$STAMP")
printf 'created %s\n' "$ARCHIVE"
printf 'install on target: cd lab-stack-%s/cli/lab-stack && ./install.sh --host LAB_8GB --cleanup delete\n' "$STAMP"
