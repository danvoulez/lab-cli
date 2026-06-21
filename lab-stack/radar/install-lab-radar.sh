#!/usr/bin/env bash
# install-lab-radar.sh — the box watches & records itself through `lab`, on a timer.
# Runs `lab radar` (scan -> judge -> notify-if-critical) every interval.
# $HOME-portable (no hardcoded user) — survives the move to danvoulez. Idempotent.
set -euo pipefail

LABEL="com.minilab.lab-radar"
INTERVAL="${1:-300}"                       # seconds between runs (default 5 min; phased scan)
UID_N="$(id -u)"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
PATH_VALUE="$HOME/.cargo/bin:/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"

resolve_lab() {
  if [ -n "${LAB_BIN:-}" ] && [ -x "$LAB_BIN" ]; then printf '%s\n' "$LAB_BIN"; return 0; fi
  if [ -x /usr/local/bin/lab ]; then printf '%s\n' /usr/local/bin/lab; return 0; fi
  if [ -x "$HOME/.cargo/bin/lab" ]; then printf '%s\n' "$HOME/.cargo/bin/lab"; return 0; fi
  if PATH="$PATH_VALUE" command -v lab >/dev/null 2>&1; then PATH="$PATH_VALUE" command -v lab; return 0; fi
  echo "install-lab-radar: lab binary not found; set LAB_BIN or install lab first" >&2
  return 1
}

LAB="$(resolve_lab)"

mkdir -p "$HOME/Library/LaunchAgents" "$HOME/.radar/logs"

cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$LAB</string>
    <string>radar</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>HOME</key><string>$HOME</string>
    <key>PATH</key><string>$PATH_VALUE</string>
  </dict>
  <key>StartInterval</key><integer>$INTERVAL</integer>
  <key>RunAtLoad</key><true/>
  <key>StandardOutPath</key><string>$HOME/.radar/logs/lab-radar.out</string>
  <key>StandardErrorPath</key><string>$HOME/.radar/logs/lab-radar.err</string>
</dict>
</plist>
EOF

launchctl bootout "gui/$UID_N/$LABEL" 2>/dev/null || true
launchctl bootstrap "gui/$UID_N" "$PLIST"
launchctl kickstart "gui/$UID_N/$LABEL"

echo "installed $LABEL -> '$LAB radar' every ${INTERVAL}s (RunAtLoad)"
echo "logs: ~/.radar/logs/lab-radar.{out,err}   reinstall: bash ~/.radar/install-lab-radar.sh [interval]"
