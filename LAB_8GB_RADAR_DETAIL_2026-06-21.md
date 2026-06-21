# LAB 8GB Radar Detail

Generated from LAB 8GB over public SSH via `lab-8gb-cf`.

Source snapshot:

- Host: `lab-8gb`
- User: `danvoulez`
- Radar report generated at: `2026-06-20T23:39:45Z`
- Local Google Drive sync footprint after cleanup: `8.0K`
- Data volume disk state after cleanup: `460G total / 264G used / 160G available / 63%`

## Executive Summary

Radar reports:

```text
OK:        5
DEGRADED: 1
DOWN:      3
```

Important correction: the two Manhattan `DOWN` verdicts are false negatives from Radar's current launchd check. Direct `launchctl print` shows both Manhattan jobs are running.

Real problems:

1. MCP Host Runtime is down.
2. LAB 512 inference port is reachable over the cable, but the HTTP API is not answering.
3. Radar's launchd checker needs to be upgraded so it can correctly judge system daemons and running jobs whose previous exit was `-15`.

Fixed already:

1. LAB 8GB now has the new `lab` CLI installed.
2. LAB 8GB now has `lab-radar` installed and loaded.
3. Manhattan package was reinstalled and bootstrapped.
4. Package verification passed `18/18`.
5. Local Google Drive sync/cache was removed from LAB 8GB without touching Google Drive cloud data.

## Current Radar Verdicts

| System | Radar verdict | Interpretation |
|---|---:|---|
| MCP Host Runtime | DOWN | Real failure. Launchd exists, last exit `1`, no HTTP on `127.0.0.1:8788`, public `8gb.minilab.work/health` does not answer. |
| Manhattan agent | DOWN | False negative. Direct launchd check shows `state = running`, `pid = 81889`. |
| Manhattan daemon | DOWN | False negative. Direct launchd check shows `state = running`, `pid = 81887`. |
| Radar scanner | OK | Real OK. `com.minilab.lab-radar` is loaded and scan status is fresh. |
| Ledger heartbeat | OK | Real OK. `lab ping` reaches Supabase ledger. |
| Control UI | OK | Real OK. `https://control.minilab.work` returns HTTP 200. |
| SSH remote login | OK | Real OK. `127.0.0.1:22` is listening. |
| Inference | DEGRADED | Real degraded. TCP `10.88.0.10:1234` is open, but `/v1/models` gives no HTTP response. |
| Disk headroom | OK | Real OK. 160G free after Google Drive cleanup. |

## Evidence From Radar

### MCP Host Runtime

Radar says:

```text
Verdict: DOWN
Action: dead and not serving -> RESTART
```

Evidence:

```text
launchd job loaded and last exit 0: fail
evidence: loaded but last exit 1

answers on local port 8788: fail
evidence: no HTTP response (http://127.0.0.1:8788/health)

reachable through public Cloudflare tunnel: fail
evidence: no HTTP response (https://8gb.minilab.work/health)
```

Interpretation:

This is a real failure. The protected remote execution door is not serving on LAB 8GB.

Next action:

Inspect `/Users/danvoulez/Library/LaunchAgents/com.minilab.host-runtime.plist`, the target script under `/Users/danvoulez/minilab/runtimes/current/scripts/run-host-runtime.sh`, and logs under `/Users/danvoulez/minilab/logs/host-runtime.*.log`.

### Manhattan Agent

Radar says:

```text
Verdict: DOWN
Evidence: loaded but last exit -15
```

Cross-check says:

```text
gui/501/com.project-manhattan.agent
state = running
program = /usr/local/project-manhattan/bin/manhattan-agent
pid = 81889
runs = 2
last terminating signal = Terminated: 15
```

Interpretation:

This is a Radar false negative. The job is running. Radar is reading a stale previous termination signal as failure.

Next action:

Change Radar launchd checks to use `launchctl print gui/<uid>/<label>` and treat `state = running` as pass.

### Manhattan Daemon

Radar says:

```text
Verdict: DOWN
Evidence: not loaded
```

Cross-check says:

```text
system/com.project-manhattan.daemon
state = running
program = /usr/local/project-manhattan/bin/manhattan-daemon
pid = 81887
runs = 2
last terminating signal = Terminated: 15
```

Interpretation:

This is also a Radar false negative. Radar's current `launchctl list` check is user-domain oriented and does not correctly judge the system daemon.

Next action:

Add domain-aware launchd checks to `success.json`, for example:

```json
{
  "kind": "launchd",
  "domain": "system",
  "label": "com.project-manhattan.daemon"
}
```

and:

```json
{
  "kind": "launchd",
  "domain": "gui",
  "label": "com.project-manhattan.agent"
}
```

### Radar Scanner

Radar says:

```text
Verdict: OK
Evidence:
- com.minilab.lab-radar loaded, last exit 0, pid 81555
- ~/.radar/status.json updated within 30 minutes
```

Interpretation:

Radar is installed and running through the new `lab radar` loop.

### Ledger Heartbeat

Radar says:

```text
Verdict: OK
Evidence: lab ping exited 0
Ledger: https://ipfbjwhnhgbxoynqmfxz.supabase.co
```

Interpretation:

The new CLI can reach the ledger from LAB 8GB.

### Control UI

Radar says:

```text
Verdict: OK
Evidence: HTTP 200 from https://control.minilab.work
```

Interpretation:

The public control UI is serving. This does not mean every UI route is wired, only that the public surface is alive.

### SSH

Radar says:

```text
Verdict: OK
Evidence: tcp 127.0.0.1:22 open
```

Interpretation:

Remote login is alive locally, and public SSH over `ssh8gb.minilab.work` works via Cloudflare Access.

### Inference

Radar says:

```text
Verdict: DEGRADED
Evidence:
- tcp 10.88.0.10:1234 open
- http://10.88.0.10:1234/v1/models gives no HTTP response
```

Interpretation:

The cable path to LAB 512 reaches something on port `1234`, but the service is not speaking the expected OpenAI-compatible HTTP API.

Next action:

On LAB 512, inspect the mistral.rs service, its bind address, logs, and whether it is accepting TCP without completing HTTP responses.

### Disk

Radar says:

```text
Verdict: OK
Evidence: 160G free, minimum 10G
```

Interpretation:

The Google Drive local sync cleanup worked. Disk pressure is no longer the immediate issue.

## Storage After Google Drive Cleanup

Radar's largest storage entries after cleanup:

```text
170G  /Users/danvoulez
65G   /Users/danvoulez/Library
38G   /Users/danvoulez/.colima
14G   /Applications
12G   /Users/danvoulez/Pictures
11G   /Users/danvoulez/.docker
10G   /Users/danvoulez/Documents
8G    /opt
6.2G  /Users/danvoulez/from-LAB2566
```

Google Drive local sync state:

```text
8.0K  /Users/danvoulez/Library/CloudStorage/GoogleDrive-dan@danvoulez.com
```

## Install State

The new package install on LAB 8GB completed:

```text
lab CLI: built and linked
/Users/danvoulez/.cargo/bin/lab -> /Users/danvoulez/cli/target/release/lab
/usr/local/bin/lab -> /Users/danvoulez/cli/target/release/lab

Radar: installed com.minilab.lab-radar every 300s
Manhattan: installed from /Users/danvoulez/manhattan/project-manhattan-v2
Verifier: 18 passed, 0 failed
```

Notable installer output:

```text
item_level_ghosts:
- shared_values_requiring_daniel.cloudflared_tunnel_health_endpoint
- pair_8gb_512_values_requiring_daniel.peer_macs
```

These are not install blockers. They are still unresolved observational facts.

## Recommended Next Fix

First fix Radar's launchd checker.

Reason:

Radar currently produces two false DOWNs for Manhattan even though both Manhattan jobs are running. That reduces trust in the dashboard. The real service failures should be visible without false noise.

Implementation direction:

1. Extend `radar-judge.py` launchd checks to support `domain`.
2. For `domain = "system"`, call:

   ```bash
   launchctl print system/<label>
   ```

3. For `domain = "gui"`, call:

   ```bash
   launchctl print gui/$(id -u)/<label>
   ```

4. Treat `state = running` as pass.
5. Keep `launchctl list` only as a fallback for old checks with no domain.
6. Update LAB 8GB `~/.radar/success.json` and the install pack's `lab-stack/radar/success.json`.

After that:

1. Fix MCP Host Runtime on LAB 8GB.
2. Investigate mistral.rs HTTP readiness on LAB 512.
3. Continue UI wiring from a clean, trusted Radar baseline.

