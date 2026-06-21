#!/usr/bin/env bash
set -euo pipefail

PURGE=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --purge) PURGE=1; shift;;
    -h|--help) printf '%s\n' "usage: uninstall.sh [--purge]"; exit 0;;
    *) printf 'uninstall: unknown argument: %s\n' "$1" >&2; exit 2;;
  esac
done

UID_N="$(id -u)"

launchctl bootout "gui/$UID_N/com.minilab.lab-radar" 2>/dev/null || true
rm -f "$HOME/Library/LaunchAgents/com.minilab.lab-radar.plist"

launchctl bootout "gui/$UID_N/com.project-manhattan.agent" 2>/dev/null || true
sudo launchctl bootout system/com.project-manhattan.daemon 2>/dev/null || true
sudo rm -f /Library/LaunchAgents/com.project-manhattan.agent.plist
sudo rm -f /Library/LaunchDaemons/com.project-manhattan.daemon.plist

if [ "$PURGE" -eq 1 ]; then
  sudo rm -rf /usr/local/project-manhattan
  rm -f "$HOME/.cargo/bin/lab"
  if [ -L /usr/local/bin/lab ]; then sudo rm -f /usr/local/bin/lab; fi
  printf 'purged installed stack files; ~/.radar data and credentials were left intact\n'
else
  printf 'unloaded launchd jobs and removed plists; installed files and ~/.radar data were left intact\n'
fi

