# Lab Stack Install Pack

This directory wraps the current spine for another Mac:

- `lab` CLI: builds the Rust binary and installs it on the user PATH.
- Radar: installs the local scanner, judge, success list, and `lab radar` launchd timer.
- Manhattan: calls the Manhattan package installer so the daemon and agent run every 5 minutes and heartbeat through `lab`.

The pack keeps credentials out of the artifact. Put Supabase credentials in `~/.radar/sync.env` and Maileroo credentials in `~/.radar/.notify.env` on the target machine before verification.

## Install

From a bundled pack:

```bash
cd cli/lab-stack
./install.sh --host LAB_8GB --cleanup delete
```

Valid hosts are `LAB_256`, `LAB_8GB`, and `LAB_512`.

## Build A Transfer Bundle

From LAB-256:

```bash
./lab-stack/pack.sh
```

The bundle is written under `dist/` and includes this CLI repo, the Radar payload, and the Manhattan package source.

## Verify

```bash
./lab-stack/verify.sh --host LAB_8GB
```

Verification is intentionally practical: it checks `lab`, Supabase reachability, Radar scripts, `lab-radar` launchd, Manhattan installed files, and Manhattan launchd jobs.

