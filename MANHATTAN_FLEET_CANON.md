# Manhattan + Lab Fleet Canon

This file fixes the install shape before LAB-8GB and LAB-512 are touched.

## Roles

| Machine | Role | Priority |
|---|---|---|
| LAB-8GB | orchestration, auth, control, fleet UI, coordination | first |
| LAB-512 | inference, model serving, heavy jobs | second |
| LAB-256 | workstation, cockpit, local operator surface | third |

LAB-8GB closes first. LAB-512 follows because it is the inference plane. LAB-256 stays important, but it is not the control-plane source of fleet authority.

## Account

Canonical account on all boxes:

```text
danvoulez
```

`ubl-ops` is migration residue on LAB-256. Do not carry it to LAB-8GB or LAB-512 as a canonical path.

## User Home Layout

Under `/Users/danvoulez`:

```text
~/cli/                         recommended lab CLI source location
~/.cargo/bin/lab               user PATH symlink to ~/cli/target/release/lab
~/.radar/                      local Radar truth, credentials, logs, timers
~/.radar/sync.env              Supabase ledger credentials
~/.radar/.notify.env           Maileroo Reach-Dan credentials
~/manhattan/project-manhattan-v2/  Manhattan source package
~/engine-park/                 engines and inference runtimes
~/app-park/                    apps, control surfaces, MCP host runtime
```

## System Install Layout

These paths are system-level and identical on every box:

```text
/usr/local/bin/lab
/usr/local/project-manhattan/
/usr/local/project-manhattan/etc/PROJECT_MANHATTAN_POLICY_REVIEW.json
/usr/local/project-manhattan/src/manhattan.py
/usr/local/project-manhattan/var/receipts/
/usr/local/project-manhattan/var/logs/
/usr/local/project-manhattan/var/state/
/Library/LaunchDaemons/com.project-manhattan.daemon.plist
/Library/LaunchAgents/com.project-manhattan.agent.plist
~/Library/LaunchAgents/com.minilab.lab-radar.plist
```

`/usr/local/bin/lab` is the stable command path for launchd and Manhattan. The source checkout may have any folder name. From the checkout root, symlink the release binary:

```bash
sudo ln -sf "$PWD/target/release/lab" /usr/local/bin/lab
```

On LAB-256 today, that symlink could not be created from the current non-sudo session. Until the migration is complete, Manhattan falls back to `~/.cargo/bin/lab` or discovers a Cargo package named `lab` under the user home.

## Manhattan Loop And Heartbeat

Manhattan has two launchd surfaces:

```text
com.project-manhattan.daemon   root/system repair loop
com.project-manhattan.agent    user/login repair loop
```

Both loops run every 300 seconds. The first action in each pass is a ledger heartbeat through `lab`:

```bash
lab heartbeat manhattan-daemon
lab heartbeat manhattan-agent
```

This keeps the equilibrium clear: local inspection and repair remain the source of operational truth, and the ledger proves later that the loop was alive.

`lab radar` is observational. It scans, judges, records, and notifies only for true critical states. It must not call `lab converge --apply` from a timer. Scheduled mutation belongs to Manhattan's daemon/agent loops. Manual mutation remains explicit through:

```bash
lab converge --apply
```

Daemon and agent repair ownership must not overlap. If a repair item belongs to a system/root launchd domain, the daemon owns it. If it belongs to a `gui/<uid>` user session, the agent owns it. Repair cycles use non-blocking lock files and skip rather than stack if a previous pass is still running.

## Desired State That Must Reach Dan

These are explicit desired states, not vague human queues:

| Item | Desired state | Automatic action |
|---|---|---|
| L-03 FileVault | FileVault is OFF | probe, receipt, email Dan if not OFF |
| L-04 Keychain | login keychain is usable for Wi-Fi/cloudflared/remote surfaces | probe, receipt, email Dan if unusable |
| L-29 Privacy / TCC | remote-access Accessibility, Input Monitoring, and Screen Recording grants are ON | read-only probe, receipt, email Dan if missing |

Manhattan must not directly modify `TCC.db`, store passwords, or fake closure. Without MDM, these remain human/MDM-granted states. The difference now is that drift is visible and reaches Dan through `lab notify`.

## Daily 06:00-07:00 Rejuvenation

Every machine must perform the daily maintenance sequence in local time:

```text
06:00  force-restart TeamViewer system service and user agent
06:10  refresh cloudflared
06:20  verify tunnel health
06:30  verify remote surfaces/login survival
06:35  write pre-restart ready receipt
06:40  restart LAB-8GB
06:50  restart LAB-512
06:59  restart LAB-256
```

The TeamViewer bounce is mandatory even when TeamViewer looks healthy. The machine restart is `shutdown -r now` from Manhattan, never a separate calendar plist or `pmset` schedule.

## Install Order

1. Build `lab` under `/Users/danvoulez/cli`.
2. Install `/usr/local/bin/lab`.
3. Install `~/.radar/sync.env` and `~/.radar/.notify.env`.
4. Install Radar LaunchAgent.
5. Install Manhattan package to `/usr/local/project-manhattan`.
6. Bootstrap Manhattan daemon and agent.
7. Run:

```bash
lab whoami
lab ping
lab heartbeat setup
lab radar
lab audit
lab converge
lab manhattan-sync 5
```

8. Confirm the control UI reads ledger-backed state for that machine.
