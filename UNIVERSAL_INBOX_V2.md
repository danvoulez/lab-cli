# The Universal Inbox — v2

*A private-infrastructure ingress that turns any inbound text into a bounded, auditable action — using an LLM as the universal adapter, the CLI as the deterministic capability layer, and a grammar as the agent's cage. v2 folds in the cable-as-inference-bus topology, the policy membrane on 8GB, a three-layer cage built from features that already exist, and a Rust-native replacement for Docker.*

**Status:** Design, pre-implementation. CI stays paused; it returns as the first consumer.
**Fleet:** LAB 8GB · LAB 512 · LAB 256, all on build `39b08de`.
**Supersedes:** Universal Inbox v1.
**Date:** June 2026

---

## 0. What changed since v1

v1 was right about the spine and wrong about two placements. v2 corrects them and replaces hand-built machinery with native features we verified in source.

| | v1 | v2 |
|---|---|---|
| **Inference** | goblin runs the model locally on 512 | inference travels the **ethernet cable**: 512 is a pure model appliance, the agent lives on 8GB and reaches it over the wire |
| **The door** | a bespoke ~10-line HTTP receiver | the **Routing Middleware already on 8GB** — door, censored-CLI surface, and policy in one membrane |
| **The cage** | a grammar we author + a `lab-goblin` wrapper binary | **three native layers**: mistral.rs forced tool-grammar (Gemma-strict), OpenClaw's own sandbox, and the membrane's creds-denial |
| **Isolation** | unspecified | **Seatbelt-via-Rust (`lab cage`)**, not Docker |
| **Where logic lives** | implied across door + goblin + wrapper | **the CLI first** — every new capability is a `lab` verb or plugin; the membrane and goblin are thin shims over `lab` |

The throughline of all four corrections: **push the work down to things that already exist** — the cable, the middleware, the model's grammar engine, the agent's own sandbox, and above all the `lab` kernel.

---

## 1. The one-sentence version

Capture every inbound request as raw text in the append-only ledger, let a small grammar-constrained model on 512 read the raw text over the cable and pick one verb from the CLI we already have, and let `lab`'s tools do the work — so the agent's only job is to point, the toolbox is what makes it deterministic, and the one membrane on 8GB holds the keys.

---

## 2. Principles

Carried from v1, still load-bearing:

1. **Capture is sacred and dumb; interpretation is separate and disposable.** Raw is written before anything interprets it.
2. **The LLM reads the raw thing.** No envelope, no per-source parser. That property is the "universal."
3. **Determinism lives in the tools, not in the prompt.** The agent's reach is what it is *structurally allowed to emit*, not what we ask of it.
4. **Append-only, content-hashed, all the way down.** Every hop is a hashed receipt.
5. **A few lines forever.** Capability grows by adding a *tool*, never by growing the pipe.

Added in v2:

6. **New logic goes to the CLI first.** Before writing a receiver, a daemon, or a middleware handler, ask: *can this be a `lab` verb or a `commands/` plugin?* The kernel already owns identity, hashing, creds, and the ledger. The membrane and the goblin must stay thin shims that **call `lab`** — never reimplement it. This keeps every action testable, auditable, and hashed through the one path.
7. **The cage is upstream of the agent.** The strongest constraint is the one the agent cannot perceive: a grammar it cannot violate at decode time, a CLI that fails closed without keys it cannot read. Prefer structural impossibility to runtime rejection, and runtime rejection to prompt instruction.

---

## 3. Topology — the cable is the inference bus

```
                         the internet
                              │
                      (DNS + cf tunnel)
                              │
                   ┌──────────▼──────────┐
                   │       LAB 8GB       │  the MEMBRANE + the agent's home
                   │  Routing Middleware │  door · censored-lab surface · policy
                   │  + lab + creds      │  (only box with creds + a public face)
                   └──┬───────────────┬──┘
          reverse SSH │               │ ethernet cable 10.88.0.0/24
           (internet) │               │ 8GB=.9  512=.10   ← INFERENCE BUS
              ┌───────▼─┐           ┌─▼──────────┐
              │ LAB 256 │           │  LAB 512   │  the APPLIANCE
              │ (draft) │           │ mistral.rs │  OpenAI+Anthropic API on :1234
              └─────────┘           │ (no creds) │  grammar-constrained decode
                                    └────────────┘
```

The inversion: **512 holds no keys and runs no agent logic.** It serves a model on `:1234` and nothing else. The agent on 8GB speaks OpenAI to a local endpoint; the request travels `8GB → 10.88.0.10:1234 → back` over the cable. 512 is a model behind a wire — swappable, keyless, dumb.

8GB is the center because it is the only box that is all three of: publicly reachable (cf tunnel), cable-attached to 512, and able to reverse-reach 256. So it holds the membrane, the keys, and `lab`.

| Organ | Box | Role |
|---|---|---|
| **Membrane** | LAB 8GB | Routing Middleware: capture raw, run the censored `lab`, enforce policy |
| **Bus / ledger** | Supabase `lab_log` | append-only, content-hashed spine |
| **Appliance** | LAB 512 | mistral.rs inference over the cable; grammar-constrained; no creds |
| **Draft** | LAB 256 | where it's written and iterated |

---

## 4. The membrane (8GB) — one file, three jobs

The Vercel/Next **Routing Middleware** already running on 8GB (`next-server v16`) runs *before* any request is processed and returns a `Response`. That makes it the one place where putting rules actually *enforces* them. It plays three roles, kept distinct:

1. **Door.** Inbound HTTP (webhook, websocket upgrade, API, MCP) → capture verbatim → `lab emit inbox <source> "<raw>"` → `200`. No parsing. The door never knows what it carries.
2. **Censored-`lab` surface.** The *only* path by which anything causes an action is an HTTP call the membrane translates into a **validated, allowlisted `lab` invocation** (argv, never shell). The membrane holds the creds; callers never do.
3. **Policy.** Host-target checks, `notify` rate-limit, idempotency on the row hash, auth-by-path, `grammar_version` stamping.

Because the membrane is the only process on 8GB that can read creds, and `lab` **fails closed without creds** (`load_creds()` → exit 2, `src/main.rs:140`), every other principal's copy of `lab` is an inert stick. This is the founding rule — *one thing holds the keys* — applied to the agent.

> **CLI-first note:** the middleware must contain *no business logic*. It does exactly three things — `fetch` the model upstream, `lab emit` the capture, and `exec` an allowlisted `lab <verb>`. Everything substantive is a `lab` verb. If a handler grows past wiring, that logic belongs in a `lab` plugin instead.

Two settings make the membrane real (config, not code):
- **`runtime: 'nodejs'`** (not Edge) — Edge can't dial the `10.88.0.x` cable IP or `exec` a subprocess.
- **Streaming pass-through** — return the upstream `Response` body directly so SSE tokens flow live.

---

## 5. The machine, end to end — mostly verbs that already exist

Auditing `src/main.rs` (1899 lines) showed the inbox is ~80% built, never called an inbox. v2's job is to *not* rebuild it.

| Inbox piece | What it already is in `lab` | Evidence | New code? |
|---|---|---|---|
| Door (write raw first) | `lab emit inbox <src> "<raw>"` → `lab_log`, `did=inbox status=raw`, JCS-hashed | `main.rs:1673` | none |
| Ledger / bus | `lab_log`, append-only, content-hashed | `main.rs:184–336` | none |
| Toolbox | `lab commands` — builtins + `commands/` plugins, introspectable | `main.rs:1828` | none |
| Goblin reads work | `lab read lab_log "did=eq.inbox&status=eq.raw"` | `main.rs:1656` | none |
| Goblin acts | run a chosen `lab <verb> <args>` | `main.rs:1894` | none |
| Transitive leash | `run_external` injects `LAB_BIN` into every plugin's env | `main.rs:1608` | none |
| Grow a tool | `lab new-command <name>` (self-logging) | `main.rs:1874` | none |

**The goblin is a plugin, not a service.** `lab new-command goblin` scaffolds `commands/goblin` — a ~30-line bash loop:

```
lab read (unhandled inbox rows)
  → curl the membrane's OpenAI endpoint with {tools, tool_choice:"required"}
  → receive one tool call {verb, args}
  → run lab <verb> <args>            (through the cage)
  → lab emit inbox.routed --status handled
```

No Rust, no daemon stack. The kernel owns identity, hashing, creds, ledger; the plugin orchestrates `read → infer → emit`. New routing logic = a new `lab` verb the goblin is allowed to pick — the pipe never changes.

---

## 6. The cage — three native layers, each a different trust domain

The key v2 finding: we do not build the cage. We *assemble* it from features verified in source. Three independent layers, each enforced by a different component, so a failure in one is caught by the next.

### 6.1 Model layer — the grammar, native in mistral.rs

The cage's "decode-time impossibility" is real and already implemented. mistral.rs's constraint type (`mistralrs-core/src/request.rs:24`):

```rust
pub enum Constraint { Regex(String), Lark(String), JsonSchema(Value), Llguidance(...) }
```

all backed by **llguidance** (token-level enforcement). And tool-call grammars are **auto-built from the `tools` array and forced**: `build_tool_call_grammar(tools)`, `required_tool_call_grammar`, `maybe_force_required_grammar` (`tools/parsers/mod.rs`, `tools/state.rs:156`). There is a **Gemma-specific strict path** — `gemma4_strict.rs`: *"a branching Lark grammar where each tool gets its own branch."*

**Consequence:** pass the toolbox as OpenAI `tools`, set `tool_choice:"required"`, and the model can only emit one valid tool call from the closed set. The toolbox *is* the grammar, compiled for us — and Gemma is a first-class target. We only hand-author a grammar (Lark / JSON-schema, both supported) if we want a production beyond tool-calls, e.g. a bespoke `park <reason>`.

> **To verify on the 512 build (read-only):** `GET http://10.88.0.10:1234/api-doc/openapi.json` confirms *this* build exposes `grammar`/`tools`, and that the loaded model has a strict tool-call parser. Source says Gemma does; confirm the running binary matches.

### 6.2 Agent layer — different cages for two different brains

The fleet has two candidate "brains," with opposite risk profiles. They need different treatment:

- **Goblin (mistral.rs, single-verb).** It never runs arbitrary code — it picks a verb. Its cage is §6.1 (grammar) + the censored `lab` (§6.3). **It needs no container at all** — determinism is upstream of it.
- **OpenClaw (general agent), if/when used.** Open-source autonomous agent (Node.js Gateway, 100+ skills) that runs arbitrary shell, files, browser. CrowdStrike flags its top risks as **prompt injection (direct and indirect)** and **lateral movement via legitimate API access** — so its in-process limits are convenience, not containment. But it ships real isolation: `agents.defaults.sandbox.mode` = `off|non-main|all`, `tools.exec.host` = `sandbox|gateway`, and **fail-closed exec** when sandbox is required but absent (`src/agents/sandbox/`, audited in `src/security/audit.ts:781`). Plus per-agent `tools.deny` / profiles (`minimal|coding|messaging|full`) and a custom OpenAI-compatible provider (`api:"openai-completions"`, `baseUrl`, `apiKey`).

So "sandbox Open Claw" is largely flipping its own switches — **but we replace its Docker backend** (see §7).

### 6.3 OS / network layer — the membrane, creds-denial, single socket

The outermost layer is the founding rule plus the world's consensus (egress-allowlist + credential-isolation + least-privilege):

- The agent runs as a **dedicated user that cannot read the two creds files** (`<lab-root>/.env`, `~/.radar/sync.env`, already `600`) and has no `SUPABASE_*` in env. Its `lab` is inert by `exit 2`.
- The membrane is the **only** process holding creds; it hands them to the censored exec per-call via env, exactly as `run_external` already passes `LAB_SUPABASE_URL/KEY` (`main.rs:1606`).
- The agent's **only** network egress is the membrane socket — which carries *both* inference and action. One hole, trivially auditable: `allow localhost:<membrane>, deny all`.

| Layer | Mechanism | Enforced by |
|---|---|---|
| Model | `tools` + `tool_choice:"required"` → forced llguidance grammar (Gemma strict) | mistral.rs decode loop |
| Agent | grammar (goblin) **or** Seatbelt sandbox + `tools.deny` + fail-closed (OpenClaw) | the model / the agent |
| OS / net | membrane holds creds · single-socket egress · dedicated user · kernel `exit 2` | the 8GB host |

This is the layered cage the 2026 sandboxing consensus prescribes — assembled, not invented.

---

## 7. Substituting Docker with Rust

**Short answer: yes, and it belongs in the CLI.** On a Mac fleet, "Docker" means a Linux VM with a daemon — heavyweight, non-native, and not Rust. The Rust-native, macOS-native substitute is the **Apple Seatbelt** sandbox driven from Rust. Precedent: **OpenAI Codex sandboxes its agents on macOS with Seatbelt**, not containers.

### 7.1 The mechanism

- **`birdcage`** (phylum-dev) — a cross-platform embeddable Rust sandbox: **macOS Seatbelt**, **Linux Landlock + seccomp**. Restricts **filesystem and network** on child processes via native OS APIs. Originally built to contain untrusted package code in the Phylum CLI.
- Or drive `sandbox-exec` / `sandbox_init` with a generated `.sb` profile directly — no crate, no daemon.

Either way: no Docker daemon, no Linux VM, kernel-enforced, and it composes with the creds-denial layer.

### 7.2 CLI-first: expose it as `lab cage`

Per Principle 6, the sandbox is a **`lab` verb**, not a sidecar:

```
lab cage -- <argv...>     run argv under a Seatbelt profile:
                            · deny network except the membrane socket
                            · deny reads of the creds files
                            · writes confined to a scratch dir
                            · then exec the real command
```

`lab cage` becomes the `tools.exec.host` backend for OpenClaw (point its exec host at a `lab cage` shim instead of Docker) and a general-purpose confinement primitive for any plugin. Because it's a `lab` verb, it's hashed and auditable through the one path like everything else, and `LAB_BIN` transitivity (`main.rs:1608`) keeps the confinement following the process through any plugin it triggers.

### 7.3 Honest limits

- **Seatbelt/birdcage restrict filesystem + network, not every syscall.** They are not a microVM. For our **private, low-volume, trusted-ish** fleet that is the correct tier (the consensus "trusted internal automation" row), *especially* because the goblin path runs no arbitrary code at all.
- macOS `sandbox-exec` is officially deprecated-but-functional; `sandbox_init` (what birdcage uses) is the supported primitive. Codex's reliance on it is reassuring precedent.
- If we ever run genuinely untrusted code, escalate that path to a microVM (libkrun) — but that is not this design, and not this fleet.

**Net:** replace Docker with `lab cage` (Seatbelt via birdcage). The goblin needs no sandbox; OpenClaw points its exec host at `lab cage`; nothing requires a daemon or a VM.

---

## 8. New CLI surface (what v2 actually adds)

Everything new is a verb or a plugin — nothing reimplements the kernel.

| New thing | Kind | What it is |
|---|---|---|
| `commands/goblin` | bash plugin | the `read → infer → emit` loop; allowlist in bash; calls the membrane endpoint |
| `lab cage -- <argv>` | Rust builtin | Seatbelt/birdcage confinement; the Docker replacement; OpenClaw's exec backend |
| `inbox` / `inbox.routed` dids | convention | `lab emit inbox …` (exists) + the routed receipt the goblin writes |
| `build-relay` | bash plugin | CI as a tool (see §9) |
| membrane shim | ~10 lines TS | `fetch` upstream · `lab emit` capture · `exec lab <verb>` — no logic |

Deferred / unchanged: the ledger, hashing, identity, `read`/`emit`/`commands`/`new-command` — all already in `lab`.

---

## 9. Worked example: CI as the first consumer

No special CI path, no webhook receiver, no normalizer:

1. **Push to `main`.** GitHub fires a webhook.
2. **Door.** Hits 8GB's cf tunnel → membrane → `lab emit inbox github "<raw>"` → `lab_log`, `did=inbox status=raw`, hashed. The door has no idea it's CI.
3. **Goblin.** 512 reads the raw row over the cable, and — constrained by the forced tool-grammar — emits one tool call: `build-relay`.
4. **Cage.** Membrane validates `build-relay` is in the allowlisted set and runs it as `lab cage -- lab build-relay`.
5. **Relay (the payload that proves the point).** `build-relay` runs the heartbeat across the nervous system: 256 builds from the just-pushed code; 8GB and 512 `git pull` + build; preflight hop-by-hop, then fire — felt at the door (8GB), fanning to 256 (reverse SSH) and 512 (cable).
6. **Receipts.** Every hop writes append-only. The build is the excuse; **the reachability proof is the prize.**

`build-relay` is a `lab new-command` — a registered tool, blessed into the grammar by appearing in `tools`. The thing we paused returns as exactly one verb.

---

## 10. v2 scope

**In v1-of-the-build (first installable):**
- Membrane on 8GB: `runtime:nodejs`, three roles, streaming pass-through; upstream → `http://10.88.0.10:1234/v1` over the cable.
- `commands/goblin`: poll `did=inbox status=raw`, call the membrane with `tools`+`tool_choice:"required"`, run the verb, write `inbox.routed`.
- `lab cage`: Seatbelt/birdcage confinement verb (Docker replacement).
- Creds-denial: dedicated agent user; single-socket egress allowlist.
- `build-relay` as the first route verb (CI as consumer).
- `grammar_version` stamped on receipts.

**Deferred:**
- OpenClaw as a general second brain (the goblin covers v1; add OpenClaw behind `lab cage` later).
- Supabase realtime wake (poll-via-launchd first).
- Hand-authored grammar productions beyond tool-calls.
- Any pre-filtering (only if volume ever bites — it won't, on three minis).
- microVM escalation (only if we ever run untrusted code).

---

## 11. Open decisions

1. **Goblin dialect:** point it at mistral.rs's OpenAI endpoint (`/v1/chat/completions`) or Anthropic endpoint (`/v1/messages`) — both exist. Lean OpenAI for the widest tool-calling support.
2. **Goblin wake:** poll-via-launchd (lean) vs Supabase realtime.
3. **Sandbox crate:** `birdcage` (embeddable, cross-platform) vs hand-rolled `sandbox-exec` profile. Lean birdcage for Linux portability later.
4. **Where the agent user lives:** co-resident on 8GB under `lab cage` + creds-denial (lean, simplest) vs a separate segment.
5. **Seed verb set the goblin may pick:** `read`, `emit`, `build-relay`, `park`. Grow by blessing verbs into `tools`.

---

## 12. Verified-capabilities appendix (evidence vs. to-verify)

**Verified in source (`/tmp/research`):**
- mistral.rs constraint enum `Regex|Lark|JsonSchema|Llguidance` — `mistralrs-core/src/request.rs:24`.
- Forced tool-call grammar from `tools` — `tools/parsers/mod.rs`, `tools/state.rs:156`.
- Gemma strict branching Lark grammar — `tools/parsers/gemma4_strict.rs`.
- OpenAI + Anthropic endpoints — `mistralrs-server-core/src/openai.rs`, `anthropic.rs`.
- OpenClaw Docker sandbox + fail-closed exec — `src/agents/sandbox/`, `src/security/audit.ts:781`.
- OpenClaw custom OpenAI-compatible provider — `api:"openai-completions"`, `baseUrl`, `apiKey`.
- `lab` door/ledger/toolbox/leash — `src/main.rs:1673/184/1828/1608`.
- `birdcage` macOS Seatbelt + Linux Landlock/seccomp; OpenAI Codex uses Seatbelt on macOS.

**To verify on the running boxes (read-only, one curl/cmd each):**
- `GET http://10.88.0.10:1234/api-doc/openapi.json` — this build exposes `grammar`/`tools`; loaded model has a strict tool-call parser.
- Membrane upstream is settable to the cable IP, and `runtime:nodejs`.
- 8GB can `exec lab` and dial `10.88.0.10:1234` (cable confirmed up; OpenAI `/v1/models` answer not yet curled).

---

## 13. Glossary

- **Membrane** — the Routing Middleware on 8GB: door + censored-`lab` surface + policy; the only holder of creds.
- **Appliance** — LAB 512: keyless mistral.rs inference over the cable.
- **Inference bus** — the ethernet cable `10.88.0.0/24`; carries the agent's model calls.
- **Goblin** — the bounded brain: reads raw rows, emits one verb or parks; grammar-constrained, needs no container.
- **Cage** — the three-layer constraint stack (model grammar · agent sandbox · OS/net), not a single artifact.
- **`lab cage`** — the Rust/Seatbelt confinement verb that replaces Docker.
- **Toolbox** — the live set of `lab` verbs; capability is data, not branches.
- **Relay** — the hop-by-hop heartbeat across 8GB → 256 / 512 that proves reachability.
- **park** — the goblin's honesty valve: "I can't classify this; ask a human."

---

*v2 principle, in one line: push every constraint down to something that already exists — the cable, the membrane, the model's grammar, the kernel's keys — and put every new capability in the CLI first.*
