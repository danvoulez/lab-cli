# The Universal Inbox

*A design for a private-infrastructure ingress that turns any inbound text into a bounded, auditable action — using an LLM as the universal adapter and a grammar as its cage.*

**Status:** Design, pre-implementation. CI is paused and returns later as the first consumer.
**Fleet:** LAB 8GB · LAB 512 · LAB 256, all currently in sync on build `39b08de`.
**Date:** June 2026

---

## 1. The one-sentence version

Capture every inbound request as raw text in an append-only ledger, let a small grammar-constrained model read the raw text and pick one verb from the CLI we already have, and let the CLI's tools do the work — so the agent's only job is to point, and the toolbox is what makes it deterministic.

Everything below is the elaboration of that sentence.

---

## 2. Why this exists (and why not a normalizer)

The first instinct for an inbox is to write a normalizer: a parser per source that turns webhooks, websockets, API calls, and MCP messages into a tidy structured envelope. That instinct is wrong here, and it's worth being precise about why.

The moment you write a parser per source, you have quietly conceded that the LLM can't read raw text — which kills the entire premise. The universality is not in the code. An LLM is *already* the universal adapter; they are all just text. Writing one in Rust is insulting it. So the design inverts the usual shape: capture raw, hand the model the toolbox, and get out of the way.

This is explicitly **not** an AWS-scale universal inbox. It is private, low-volume infrastructure running on three Mac minis. That fact is a design input, not a caveat — it is what licenses the elegance. We do not need to defend against a firehose, so we do not pre-filter, do not shard, and do not build schema machinery. We let the goblin read everything, even noise.

**Discipline to hold the line:** resist pre-filtering. The instant you add "skip if health-check," you are writing the parser again. Add a bypass only if volume ever actually bites — which, on three minis, it won't.

---

## 3. Principles

These are load-bearing. The whole machine is downstream of them.

1. **Capture is sacred and dumb; interpretation is separate and disposable.** The raw request is written to the ledger verbatim *before* anything interprets it. The thing that reads it can be wrong, replaced, or rerun without ever risking the record of what arrived.

2. **The LLM reads the raw thing.** No envelope, no per-source schema. Raw text in, one verb out. That single property is the source of the "universal" in Universal Inbox.

3. **Determinism lives in the tools, not in the prompt.** The agent's reach is bounded by what it is structurally allowed to emit — not by asking it nicely. "Less the will of the agent" is achieved by shrinking the agent's surface area to *pick a verb or park*, and letting predetermined, deterministic executors do everything real.

4. **Append-only, content-hashed, all the way down.** Every hop — capture, the goblin's choice, the tool's result — is a hashed receipt in the ledger. The interpretation is as auditable as the capture.

5. **A few lines forever.** Capability grows by adding a *tool*, never by growing the pipe. The system gets smarter without the code getting bigger.

---

## 4. The topology

The fleet already has a working nervous system, established (with real digging) over the build sessions:

```
                         the internet
                              │
                      (DNS + cf tunnel)
                              │
                       ┌──────▼──────┐
                       │   LAB 8GB   │   the DOOR — only box with a public face
                       │  (center)   │   holds the cable to 512, can reverse-reach 256
                       └──┬───────┬──┘
              reverse SSH │       │ ethernet cable (10.88.0.0/24)
               (internet) │       │ 8GB=.9  512=.10
                  ┌───────▼─┐   ┌─▼────────┐
                  │ LAB 256 │   │ LAB 512  │   the BRAIN — local inference
                  │ (draft) │   │ (goblin) │   (mistralrs-serve)
                  └─────────┘   └──────────┘
```

8GB is the center, and that is a topological fact, not a preference: it is the only box with a public face (cf tunnel), it holds the cable to 512, and it can reverse-reach 256. Making the NAT'd draft box (256) the ingress was being too literal about "felt first." **8GB-as-front-door, fanning out to 256 (reverse SSH) and 512 (cable), is the honest shape.**

Mapping the organs onto the boxes:

| Organ | Box | Role |
|---|---|---|
| **Door** | LAB 8GB | DNS + cf tunnel; receives any inbound HTTP, writes it raw |
| **Bus / ledger** | Supabase `lab_log` | append-only, content-hashed spine |
| **Goblin (brain)** | LAB 512 | local inference reads raw rows, emits one verb |
| **Draft** | LAB 256 | where it's all written and iterated |

---

## 5. The machine, end to end

### 5.1 Door — dumb, ~nothing

A `cloudflared` route on 8GB points at a ~10-line receiver. The receiver appends the request **verbatim** to the ledger — method + path + headers + body as one blob, content-hashed, source-tagged — returns `200`, and is done. No parsing, no schema. **The door never knows what it is carrying.**

The keystone realization: this already exists. `lab emit inbox <source> "<raw body>"` lands a row in `lab_log` with `did=inbox status=raw`, auto-stamped who/when, JCS-hashed. The door is not something to build; it is one existing verb behind a tunnel.

### 5.2 Ledger — the bus

`lab_log`: append-only, content-hashed, already the fleet's spine. Capture writes here. The goblin reads here. The goblin's decision and the tool's result are written back here. There is one source of truth and it only ever grows.

### 5.3 Goblin — the brain on 512

The goblin's entire existence is: **raw in → `{verb, args}` out.**

It watches the ledger for unhandled rows (`did=eq.inbox & status=eq.raw`), reads the raw text, sees the toolbox (the live list of `lab` commands), and emits one tool call. Its choice is appended back as a receipt — `lab emit inbox.routed --status handled` — which both marks the row done and is itself a hashed receipt. Append-only the whole way down.

512 already serves the model (`mistralrs-serve`). The prompt is just: *"here is raw text, here is the verb list, return one verb + args, or park."*

### 5.4 Tools — the determinism

The toolbox is **the CLI we already have.** `lab` is not a tool layer built *for* the goblin; it *is* the goblin's entire world. Auditing the source showed the inbox is ~80% already built — built without being called an inbox:

| Inbox piece | What it already is in `lab` | New code? |
|---|---|---|
| Door (write raw first) | `lab emit inbox <source> "<raw>"` → `lab_log`, `did=inbox status=raw`, hashed | none |
| Ledger / bus | `lab_log`, append-only, content-hashed | none |
| Toolbox | `lab commands` — the literal builtins + plugins, introspectable at runtime | none |
| Goblin reads work | `lab read lab_log "did=eq.inbox&status=eq.raw"` | none |
| Goblin acts | run a chosen `lab <verb> <args>` | none |
| Grow a missing tool | `lab new-command <name>` (self-extending, logs its own growth) | none |

A tool is a **registered thing, not a branch in a switch statement.** Grow capability = add a tool; the pipe never changes. That is why the system stays a few lines forever.

### 5.5 What is genuinely new

Only three small things:

1. **The door wiring** — a `cloudflared` route on 8GB → a ~10-line receiver that shoves the raw request into `lab emit inbox`.
2. **The goblin loop** — a goblin process on 512: pull unhandled `did=inbox` rows → hand raw text + the verb list to local inference → get back `{verb, args}` → run it through the cage → `lab emit inbox.routed --status handled`. ~40 lines around the model call.
3. **The inference call** — already served on 512; just a constrained-decode request.

Open lean: run the goblin as a **plugin under `commands/`** rather than a Rust builtin (keeps model/prompt churn out of the kernel, iterate without recompiling), woken by a **cheap poll loop via launchd** first (dead simple, survives realtime being down, three minis is not a firehose). Supabase realtime is a later optimization.

---

## 6. The grammar — the cage

The censorship that keeps the goblin deterministic is not an allowlist the model bumps into after the fact. It is a **grammar**: with grammar-constrained generation, the model can only emit tokens that keep the output on a valid production. The cage stops being a runtime rejection and becomes a **decode-time impossibility** — the model literally cannot form an illegal sentence. The wrapper then re-validates the same grammar as defense in depth.

The reason one artifact is so strong: **the grammar is the allowlist is the grant-ledger is the prompt menu is the decode constraint is the wrapper validator.** One thing, six jobs. Generate it from `lab commands` plus per-verb "goblin-allowed" annotations, and there is a single source of truth — bless a verb and it appears everywhere at once, no drift.

### 6.1 The model: Gemma 9B, not Opus

The grammar specialist is **Gemma 9B** — and it is the correct choice, not the budget one. A grammar collapses the goblin's job from "be an agent" down to "translate raw text into one production of a small grammar." That is slot-filling, not reasoning, and small models are excellent exactly there. A big model is actually slightly *worse* here — more tempted to be clever at the one spot that must be boringly reliable.

The design principle, stated cleanly: **smart if it doesn't have to pretend it knows more than it does.** The grammar never hands the model a question it can't answer. The worst it can honestly say is `park`.

Because the brain is bounded by the grammar, it is **swappable**: Gemma today, something else tomorrow — the cage and the audit don't change.

### 6.2 `park` is a first-class production

`park <reason>` must be a production in the grammar — the honesty valve. It is what converts a small model from "hallucinates under pressure" to "asks for help when it's unsure." When the goblin parks, it emits a row + a `lab notify` to the human. The human (or a session) adds the needed tool; now it is in the set next cycle. **The goblin requests capability; a human grants it.** Its will stays bounded to "pick a verb," forever — it never authors its own hands (`new-command` is never in the goblin's grammar).

### 6.3 Honest division of labor

The grammar guarantees well-formed and in-set; it does **not** guarantee wise. Gemma can still pick a valid-but-wrong verb. So:

- **Grammar** → can't be malformed, can't be out-of-set, can't reach a forbidden verb.
- **Wrapper** → can't be unwise: host checks, no `--apply` unless the row's target matches this box, `notify` rate-limited, idempotency on the row hash.
- **Menu design** → few, semantically distinct verbs. Don't give a 9B two routes that mean almost the same thing; bias to `park` on ambiguity.

### 6.4 Structural censorship (the attack surface)

The cage is only real if it is structural, not prompt-deep:

- **argv, never shell.** The wrapper must `exec` argv arrays, never `bash -c "$args"`, and validate each arg. Otherwise the goblin smuggles a second command (`ok; lab new-command evil`). The censorship is exactly as strong as the arg handling — full stop.
- **`LAB_BIN`-as-cage, transitive for free.** `run_external` already injects `LAB_BIN` into every plugin's env (source line ~1608). Set `LAB_BIN=lab-goblin` instead of `lab` when the goblin runs anything; then even a plugin the goblin triggers — which calls `$LAB` internally — gets routed back through the cage. The boundary follows the goblin all the way down; it can't escape by going through a tool that happens to have more reach.
- **Safe core vs dangerous surface.** Default the goblin to `read` + `emit` (observe + record) plus a couple of blessed route verbs. Every mutation — `new-command` (never), raw writes to arbitrary tables, `converge --apply`, `notify` (rate-limited at most) — is a deliberate per-verb blessing.

### 6.5 Version the leash

Version the grammar and stamp `grammar_version` into every receipt. Then the ledger records not just what the goblin *did*, but what it was *allowed to do* at that instant. **The capability envelope itself becomes append-only and auditable** — you can prove, historically, the exact reach the goblin had on any given day. Same hashed-receipt discipline as the rest of the fleet, now applied to the agent's own leash.

> **To confirm at wiring time (not asserted from memory):** what 512's `mistralrs-serve` exposes for constrained decoding — GBNF, a regex constraint, or JSON-schema structured output. That decides whether the grammar is literally a `.gbnf` or a schema compiled to one.

---

## 7. Worked example: CI as the first consumer

CI was paused on purpose. Building a bespoke webhook receiver now is throwaway work: the hardest part of CI was always "how does an event reach the fleet," and the inbox answers exactly that. So CI stops being infrastructure and becomes a **consumer** — one row in the toolbox.

The flow, with no special CI path, no webhook receiver, no normalizer:

1. **Push to `main`.** GitHub fires a webhook — an inbound HTTPS call.
2. **Door.** It hits 8GB's cf tunnel and is written raw: `lab emit inbox github "<raw webhook body>"` → `lab_log`, `did=inbox status=raw`, hashed. The door has no idea it's a CI event.
3. **Goblin.** 512 picks up the raw row, reads "push to main on lab-cli," and — constrained by the grammar — emits one verb: `build-relay`.
4. **Cage.** The wrapper validates `build-relay` is in the goblin's grammar and that args are well-formed argv; it runs it as `lab-goblin build-relay`.
5. **The relay (the payload that proves the point).** `build-relay` runs the heartbeat across the whole nervous system:
   - **256** builds from the just-pushed local code.
   - **8GB** and **512** `git pull` + build locally.
   - The relay preflights connectivity hop-by-hop, then fires: the event is felt at the door (8GB) → fans to 256 (reverse SSH) and 512 (cable).
6. **Receipts.** Every hop writes append-only to the ledger. The build is almost the excuse; **the reachability proof is the prize** — permanent and queryable.

`build-relay` is itself a `lab new-command` — a registered tool, added by a human, blessed into the grammar. The thing we paused returns as exactly one verb. That is the whole claim of the design demonstrated on a real workload: *the gating "if this works, they all build" is just a tool the goblin is allowed to pick.*

---

## 8. v1 scope

Lean recommendation for the first installable version:

**In v1**
- Door: cf tunnel route on 8GB → ~10-line receiver → `lab emit inbox`.
- Goblin loop on 512: poll `did=inbox status=raw`, constrained-decode, run through cage, write `inbox.routed`.
- Seed grammar productions: `read` (bounded queries), `emit` (record), a couple of blessed route verbs, and `park <reason>`. A goblin that can observe, record, route the obvious, and honestly tap out on everything else.
- `lab-goblin` cage: argv-only, `LAB_BIN` transitivity, `grammar_version` stamped on receipts.
- `build-relay` as the first registered route verb (CI as consumer).

**Deferred to v2**
- Supabase realtime wake (poll-via-launchd is fine to start).
- Richer grammar productions beyond the seed set.
- Any pre-filtering — only if volume ever bites.

---

## 9. Open decisions

Small and genuinely optional:

1. **Goblin wake:** poll-via-launchd (lean) vs Supabase realtime subscribe.
2. **Goblin housing:** plugin under `commands/` (lean) vs Rust builtin.
3. **Toolbox location:** read live from `lab commands` (lean) vs a config the goblin loads at start.
4. **Constrained-decode form:** confirm what `mistralrs-serve` on 512 supports (GBNF / regex / JSON-schema) before fixing the grammar's file format.
5. **Seed verb set:** is `read + emit + route + park` the right v1 shape, or grow productions from there.

---

## 10. Glossary

- **Door** — the public ingress on 8GB (DNS + cf tunnel); writes raw, knows nothing.
- **Ledger / bus** — `lab_log`; append-only, content-hashed record of everything.
- **Goblin** — the bounded agent on 512; reads raw rows, emits one verb or parks.
- **Cage** — `lab-goblin`, the grammar-enforcing wrapper that execs the real kernel.
- **Grammar** — the single artifact that is simultaneously allowlist, grant-ledger, prompt menu, decode constraint, and wrapper validator.
- **Toolbox** — the live set of `lab` verbs; capability is data, not branches.
- **Relay** — the hop-by-hop heartbeat across 8GB → 256 / 512 that proves reachability.
- **park** — the goblin's honesty valve: "I can't classify this; ask a human."
