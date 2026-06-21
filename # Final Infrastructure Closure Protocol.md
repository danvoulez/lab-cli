# Final Infrastructure Closure Protocol

## 0. Purpose

This protocol defines how the infrastructure phase ends.

After two years of coding, the goal is no longer to create more possibilities.
The goal is to close the system into a provable institution.

Infrastructure is finished only when:

1. the canon is explicit;
2. the authority chain is fixed;
3. the machines are installed and alive;
4. maintenance survives time;
5. every serious event passes through `lab`;
6. the ledger records admitted reality;
7. a human UI proves the state without reading code.

Code alone does not close infrastructure.

Installed, ledger-backed, human-visible operation closes infrastructure.

---

## 1. Canon

The following is canon.

### 1.1 Primitive

The primitive is local observation admitted into an operational record.

The machine sees first. A local scan, syscall, file read, probe, or receipt is the primary evidence of what is true now.

That evidence is not closed until `lab` records it durably.

Every serious fact must become a record with:

* who;
* did;
* this;
* when;
* status;
* data;
* hash;
* source machine;
* ledger custody.

### 1.2 Kernel

The Rust `lab` CLI is the operational kernel.

It is not just a convenience tool.
It is the custody boundary.

All serious writes must go through `lab`.

### 1.3 Ledger

Supabase is the shared ledger.

It is not merely a database.
It is the common institutional memory of the fleet.

It does not replace local observation.
It preserves admitted observation so it can be proven later, compared across machines, and shown without trusting memory.

### 1.4 Fleet

The fleet is composed of three machines:

* `lab-256`;
* `lab-512`;
* `lab-8gb`.

A machine is not in the fleet because it exists physically.
A machine is in the fleet only when it can identify itself, write to the ledger, and appear in the human UI.

### 1.5 Maintenance

Maintenance is part of the institution.

Radar, judge, notify, audit, converge, and Manhattan are not side scripts.
They are the immune system of the fleet.

Their ownership is not interchangeable.
Radar observes.
Manhattan maintains.
`lab` records.

### 1.6 UI Proof

A fact that cannot be shown in a human UI is not closed.

The UI is not the source of truth.
The local machine is the source of observation.
The ledger is the durable record of admitted observation.

But the UI is the proof that the truth can be seen, understood, and operated by a human.

---

## 2. Non-Canon

The following are not canon.

* code that exists but is not installed;
* services that exist but are not running;
* scripts that bypass `lab`;
* apps that write directly to Supabase;
* dashboards backed by mock data;
* logs without ledger records;
* memory of previous sessions;
* verbal claims of completion;
* “it worked once”;
* local state that was never admitted through `lab`;
* UI state not backed by the ledger;
* credentials copied into random services;
* future Engine Park or App Park assumptions before the kernel is closed.

---

## 3. Authority Chain

Authority descends in this order:

1. Canon doctrine
2. local observation
3. `lab` custody
4. admitted ledger records
5. installed services / LaunchAgents
6. human UI proof
7. logs
8. source code
9. notes, memory, intention

A repository does not prove deployment.

A script does not prove operation.

A service file does not prove the service is alive.

A dashboard does not prove truth unless it reads live ledger-backed state that traces back to local observation.

---

## 4. Boundaries

### 4.1 Kernel Boundary

`lab` owns operational custody.

Allowed through kernel:

* `lab emit`;
* `lab heartbeat`;
* `lab ping`;
* `lab tail`;
* `lab radar`;
* `lab scan`;
* `lab judge`;
* `lab audit`;
* `lab converge`;
* `lab manhattan-sync`;
* future `lab run`;
* future `lab inspect`.

Forbidden:

* direct app writes to Supabase;
* direct engine writes to Supabase;
* hidden service credentials;
* duplicated persistence layers;
* untracked maintenance scripts;
* silent mutation of operational truth.

### 4.2 Maintenance Boundary

Maintenance may:

* inspect;
* clean;
* report;
* repair;
* emit;
* notify;
* converge.

Maintenance may not:

* redefine canon;
* create hidden authority;
* bypass `lab`;
* spam humans;
* hide critical failure;
* run heavy scans by default.

Radar must be phased.

Email must remain life-and-death only.

Scheduled `lab radar` must remain observational:

```text
scan -> judge -> notify
```

It must not call `lab converge --apply` from a timer.

Scheduled mutation belongs to Manhattan.
Manual mutation remains explicit through:

```bash
lab converge --apply
```

### 4.3 Engine Boundary

Engines contain complex logic.

They may:

* infer;
* parse;
* validate;
* render;
* classify;
* transform;
* summarize;
* plan.

They may not:

* own truth;
* hold primary credentials;
* write directly to the ledger;
* define authority;
* replace `lab`.

An engine produces material.
`lab` records institutional reality.

Manhattan is an engine for maintenance and repair.
It may inspect the OS, decide drift, repair drift, and write local receipts.
It does not own ledger custody.

The bridge is `lab manhattan-sync`: full receipts remain local, compact hashed summaries enter the ledger.

### 4.3.1 Inference Boundary

Inference is an engine surface, not an authority surface.

The inference plane is:

* `lab-512`: model host and heavy inference machine;
* direct Ethernet cable from `lab-512` to `lab-8gb`;
* `lab-8gb`: treated inference middleware and policy boundary;
* `lab-256`, `lab-512`, and outside clients: consumers of the treated inference surface through local Wi-Fi or the `minilab.work` DNS/external URL path.

`lab-512` may serve raw model capability.
`lab-8gb` must expose the treated inference interface.

The rest of the fleet should consume inference from `lab-8gb`, not by reaching around it to raw model service on `lab-512`.

Inference may:

* explain a known problem;
* rewrite a known action into human language;
* summarize receipts;
* classify text when the source facts are already provided;
* improve email wording;
* produce operator-facing explanations.

Inference may not:

* decide what is broken;
* decide whether to notify Dan;
* decide whether to repair;
* invent operational facts;
* block notification delivery;
* hold ledger credentials;
* write directly to Supabase;
* become required for `lab`, Radar, Manhattan, or email to function.

The invariant is:

```text
deterministic fact first
optional inference polish second
send or record no matter what
```

For email, Manhattan or Radar must build a deterministic action-required payload first:

```json
{
  "kind": "action_required_email",
  "machine": "LAB_256",
  "problem": "Remote access privacy permissions are missing",
  "action": "Open System Settings > Privacy & Security and enable the required remote-access permissions.",
  "fallback_subject": "Action required: L-29 on LAB_256",
  "fallback_body": "Problem: Remote access privacy permissions are missing on LAB_256.\n\nDo this: Open System Settings > Privacy & Security..."
}
```

Only after that may `lab` ask inference to humanize the message.
If inference is unavailable, slow, or malformed, `lab notify` sends the deterministic fallback.

Future CLI commands may include:

```bash
lab infer --task email_alert
lab infer-health
lab explain --style email-alert
lab notify --polish
```

These commands are future surfaces.
They must not be required for closure of the kernel, maintenance, or notification path.

### 4.4 App Boundary

Apps compose engines.

They may:

* expose services;
* orchestrate workflows;
* present user-facing operations;
* call engines;
* request `lab` actions.

They may not:

* duplicate engine logic unnecessarily;
* write directly to the ledger;
* become their own runtime authority;
* hold private institutional truth outside the ledger.

### 4.5 Client Boundary

Clients are generated surfaces.

They may:

* display;
* request;
* trigger;
* explain;
* confirm;
* route.

They may not:

* become truth;
* hide ledger state;
* present mock state as real;
* perform invisible authority actions.

The final client layer should be manifest-built.

---

## 5. Permanent Rules

These rules must survive the closure.

1. One write path: `lab`.
2. One credentials boundary.
3. One shared ledger.
4. Machines are share-nothing.
5. Every box reports for itself.
6. Maintenance is installed, not optional.
7. Heavy scans are phased.
8. Email is critical-only.
9. UI proof is mandatory.
10. Apps and engines come after kernel closure.
11. Source code is not proof.
12. Human visibility is part of reality.
13. No hidden authority.
14. No duplicate persistence.
15. Manhattan receipts are local evidence until `lab` admits them.
16. No rewrite of basic CLI logic unless a test proves it is necessary.
17. Inference may humanize facts, but may not decide facts.
18. Notification must not depend on inference being available.
19. Radar observes; Manhattan repairs.

---

## 5.1 Current State — 2026-06-21

This section records current closure progress.
It is not final closure.
It prevents already-finished work from being rediscovered as mystery.

### Done On LAB-256

Verified locally on 2026-06-21:

* `lab whoami` returns `lab-256`;
* `lab ping` reaches the Supabase ledger;
* `lab tail` shows recent ledger-backed records;
* `lab` writes JCS-RFC8785 + SHA-256 content hashes;
* `lab radar` is observational: scan, judge, notify only;
* `lab radar` no longer runs `converge --apply`;
* `com.minilab.lab-radar` is installed with a 300 second interval;
* Manhattan daemon is installed and running;
* Manhattan agent is installed and running;
* Manhattan daemon and agent emit `lab heartbeat`;
* Manhattan daemon and agent repair ownership is split with no overlapping item sets;
* Manhattan repair cycles use non-blocking locks;
* scheduled Manhattan repair emits compact cycle summaries instead of per-item noise;
* `lab converge --apply` remains explicit manual mutation;
* `lab notify` sends deterministic action-required emails;
* critical email content is short and action-oriented;
* folder-name discovery is migration-safe for `lab` and Manhattan source checkouts;
* source and live Manhattan copies compile;
* Rust CLI release build succeeds.

### Not Done Yet

Fleet closure still requires:

* install and verify `lab` on `lab-8gb`;
* install and verify `lab` on `lab-512`;
* install and verify Manhattan on `lab-8gb`;
* install and verify Manhattan on `lab-512`;
* install and verify Radar on `lab-8gb`;
* install and verify Radar on `lab-512`;
* build fleet cards for all three machines;
* complete credential audit;
* retire or explicitly keep `ubl-ops`;
* install final canonical account/path shape under `danvoulez`;
* prove reboot recovery;
* prove remote access after reboot;
* prove backup/export;
* prove MCP/control runtime status;
* build live control UI backed by ledger data;
* create deployment matrix;
* create test plan;
* create final demo script;
* run closure court.

### Inference Not Done Yet

The inference direction is decided, but not closed.

Still required:

* keep Mistral.rs/model serving on `lab-512`;
* wire `lab-512` to `lab-8gb` over the direct Ethernet cable;
* build treated inference middleware on `lab-8gb`;
* expose treated inference on local Wi-Fi;
* expose treated inference through `minilab.work` DNS / external URL;
* add future `lab infer` / `lab infer-health` / `lab explain` only after the middleware is stable;
* optionally add `lab notify --polish`;
* guarantee deterministic fallback for every inference-polished command.

Inference is not required for current email safety.
It may improve wording later, but notification must already work without it.

---

## 6. What Must Be Done

## Phase 0 — Freeze Canon

Create a final canon document.

It must contain:

* primitive;
* kernel;
* ledger;
* fleet;
* authority order;
* boundaries;
* forbidden moves;
* closure definition.

Output artifact:

* `INFRA_CANON.md`

Acceptance test:

* a new operator can read it and explain what owns truth.

---

## Phase 1 — Close the Rust CLI Kernel

For each machine:

* source exists;
* binary builds;
* binary is on PATH;
* `lab whoami` works;
* `lab ping` works;
* `lab heartbeat setup` works;
* `lab tail` shows the write;
* hashes are visible;
* credentials are read from the correct file;
* no hardcoded user path exists.

Output artifacts:

* installed `lab` binary;
* CLI version record;
* heartbeat record;
* hash record.

Acceptance test:

```text
From every machine:
lab whoami
lab ping
lab heartbeat setup
lab tail 5
```

Pass condition:

Each machine writes and reads its own ledger-backed proof.

---

## Phase 2 — Close Credentials

Credential rules:

* one Supabase credentials file;
* correct permissions;
* no credentials in apps;
* no credentials in engines;
* no credentials in UI;
* no duplicate `.env` sprawl;
* no raw Supabase writes outside `lab`.

Output artifact:

* `CREDENTIALS_AUDIT.md`

Acceptance test:

```text
Find every Supabase key.
Prove only approved files contain it.
Prove only lab uses it for serious writes.
```

Pass condition:

No unauthorized credential holder exists.

---

## Phase 3 — Close Maintenance

Install and verify:

* Radar scan;
* Radar judge;
* Radar notify;
* Radar LaunchAgent;
* Manhattan/self-healer;
* audit;
* converge plan mode;
* converge apply mode where safe.
* Manhattan receipt sync.

Required behavior:

* phased scan only;
* no default full heavy scan;
* no background throttle that creates hung I/O;
* scheduled `lab radar` observes only and does not mutate;
* scheduled repair belongs to Manhattan daemon/agent;
* notify only on critical condition;
* notification is deterministic first, optional inference-polished later;
* all maintenance results emitted through `lab`.
* Manhattan receipts admitted through `lab manhattan-sync` or through `lab audit` / `lab converge` automatic sync.

Output artifacts:

* LaunchAgent installed proof;
* latest radar ledger row;
* latest judge ledger row;
* latest audit ledger row;
* latest Manhattan receipt ledger row;
* Manhattan status proof.

Acceptance test:

```text
lab radar
lab audit
lab converge
lab manhattan-sync 5
launchctl list | grep lab-radar
lab tail
```

Pass condition:

Maintenance produces visible ledger-backed state without human babysitting.

---

## Phase 4 — Close the Fleet

For each machine, create a fleet card:

```text
Machine:
Role:
Location:
User:
Network path:
Remote access:
lab installed:
heartbeat:
radar:
audit:
maintenance:
last seen:
UI visible:
known gaps:
```

Required machines:

* `lab-256`;
* `lab-512`;
* `lab-8gb`.

Output artifact:

* `FLEET_CARDS.md`

Acceptance test:

A human can answer:

* is the machine alive?
* what is its role?
* when did it last report?
* what is broken?
* what should I do?

Pass condition:

All three machines are visible as living members of one fleet.

---

## Phase 5 — Close Human UI Proof

The control UI must show live state.

Minimum screen:

* fleet list;
* machine name;
* role;
* last heartbeat;
* last radar status;
* last audit status;
* critical alerts;
* installed services;
* missing services;
* latest ledger records;
* action required;
* proof hash / record link.

The UI must distinguish:

* healthy;
* warning;
* critical;
* unknown;
* not installed;
* stale.

Output artifact:

* live control UI;
* screenshot evidence;
* UI route list.

Acceptance test:

Without opening a terminal, a human can see whether the fleet is alive.

Pass condition:

A human UI proves the system exists.

---

## Phase 6 — Close Deployment Reality

Deployment means installed and running.

For each component, classify:

```text
Component:
Exists in source:
Built:
Installed:
Running:
Writes to ledger:
Visible in UI:
Owner:
Restart behavior:
Recovery procedure:
```

Components:

* `lab`;
* radar;
* judge;
* notify;
* Manhattan;
* MCP/control runtime;
* Supabase schema;
* control UI;
* LaunchAgents;
* remote access;
* backup/export;
* manifest runtime if included.

Output artifact:

* `DEPLOYMENT_MATRIX.md`

Acceptance test:

No component may be marked complete unless it is installed and observable.

Pass condition:

There is no confusion between code existence and operational existence.

---

## Phase 7 — Close Tests

Create five classes of tests.

### 7.1 Kernel Tests

* hash stability;
* JCS canonicalization;
* heartbeat write;
* raw write;
* read;
* tail;
* ping;
* credential loading;
* portable home path.

### 7.2 Fleet Tests

* each box writes independently;
* each box can be seen in ledger;
* one box failure does not stop others;
* remote access works;
* reboot recovery works.

### 7.3 Maintenance Tests

* phased radar;
* judge verdict;
* critical notification;
* no notification for non-critical;
* audit emits;
* converge plans;
* converge applies safely.
* scheduled radar does not call converge apply;
* Manhattan receipts sync compactly and dedupe by receipt hash.

### 7.4 Authority Tests

* app cannot write directly;
* engine cannot write directly;
* UI cannot invent state;
* ledger record exists for every serious claim;
* hashes match content.

### 7.5 Human Proof Tests

* UI shows last heartbeat;
* UI shows stale machine;
* UI shows critical alert;
* UI shows recent ledger rows;
* UI explains action required.

Output artifact:

* `INFRA_TEST_PLAN.md`

Pass condition:

Every canonical claim has a test.

---

## Phase 8 — Close Demonstration

The final demo must show:

1. all three machines;
2. each machine emits heartbeat;
3. ledger receives records;
4. records have hashes;
5. radar runs;
6. Manhattan receipts are admitted by `lab`;
7. maintenance state appears;
8. one safe warning or critical simulation;
9. UI updates;
10. one machine restart or service restart survives;
11. no direct non-`lab` write path is used;
12. a human can understand the state;
13. final closure record is emitted.

Output artifact:

* `FINAL_DEMO_SCRIPT.md`

Pass condition:

The system can be demonstrated without explaining invisible assumptions.

---

## Phase 9 — Emit Closure Record

When all phases pass, emit a final closure record.

Example:

```bash
lab emit infra closure '{
  "phase": "end_of_coding",
  "fleet": ["lab-256", "lab-512", "lab-8gb"],
  "kernel": "lab",
  "ledger": "supabase",
  "status": "closed",
  "ui_proof": true,
  "maintenance": true,
  "direct_writes_forbidden": true
}'
```

This is the moment infrastructure becomes closed.

Output artifact:

* final ledger closure hash.

Pass condition:

The end of coding is itself recorded by the system.

---

## 10. What Must Be Tested After Deployment

After deployment, repeat:

### After 5 minutes

* radar tick happened;
* no hung process;
* heartbeat visible;
* UI current.

### After reboot

* `lab` still works;
* LaunchAgents restart;
* machine emits;
* UI shows recovery.

### After network interruption

* failure is visible;
* recovery is visible;
* no duplicate authority emerges.

### After credential rotation

* only approved credential location changes;
* apps remain credential-free;
* `lab` continues to write.

### After code update

* old binary identifiable;
* new binary identifiable;
* ledger continuity preserved;
* rollback path known.

---

## 11. What Must Be Installed

Minimum installed set:

* Rust `lab` binary on all machines;
* `~/.radar/sync.env`;
* `~/.radar/.notify.env` where needed;
* radar scripts;
* radar LaunchAgent;
* Manhattan/self-healer;
* Supabase schema;
* control UI;
* remote access;
* backup/export script;
* operator runbook;
* recovery runbook.

Optional only after closure:

* Engine Park;
* App Park;
* inference middleware;
* `lab infer`;
* `lab explain`;
* `lab notify --polish`;
* manifest client generator;
* `lab run`;
* `lab inspect`.

---

## 12. What Must Be Shown

The final human UI must show:

* the fleet;
* liveness;
* maintenance;
* recent acts;
* warnings;
* criticals;
* missing installs;
* stale machines;
* hashes;
* source machine;
* action required.

A hidden working system is not closed.

A visible mock system is not closed.

Only a live ledger-backed UI closes the loop.

---

## 13. Open Gaps To Resolve

Before declaring closure, resolve or explicitly mark:

* Is `lab` installed under the final user account on every machine?
* Is `ubl-ops` fully retired or still needed?
* Is Manhattan actually deployed or only present?
* Is MCP/control runtime alive?
* Is control UI using live ledger data or demo data?
* Are all writes routed through `lab`?
* Are radar and judge fully wrapped through `lab emit`?
* Is Supabase schema final enough for closure?
* Are backups/export tested?
* Are reboot tests complete?
* Is there a human-readable operator manual?
* Is `lab-512` inference wired to `lab-8gb` over the Ethernet cable?
* Is `lab-8gb` exposing treated inference through local Wi-Fi and `minilab.work` DNS?
* Are inference-polished notifications optional with deterministic fallback?

---

## 14. Closure Court

Before final closure, hold one closure review.

Required evidence:

* canon document;
* deployment matrix;
* fleet cards;
* test plan;
* final demo script;
* UI screenshots;
* ledger hashes;
* known gaps list;
* recovery procedure.

Questions:

1. What is canon?
2. What owns truth?
3. What is forbidden?
4. What is installed?
5. What is running?
6. What survives reboot?
7. What writes to the ledger?
8. What is visible to a human?
9. What remains intentionally unfinished?
10. What comes after infrastructure?

If any answer depends on memory, intention, or hidden code, closure fails.

---

## 15. Final Definition of Done

Infrastructure is done when the following sentence is true:

**A human can open the control UI, see the three-machine fleet, verify live ledger-backed health, inspect maintenance state, understand failures, trigger or observe recovery, and prove that every serious operational fact passed through `lab`.**

Until this is true, the infrastructure may be coded, but it is not closed.
