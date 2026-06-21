# The Universal Inbox — v3 · The Awakening Layer

*The permanent piece v1 and v2 were missing: a way for a registered thing to be **woken** when the ledger names it. Coded once as three shared receivers; everything else — the Universal Inbox, CI, every future agent — is a row.*

**Status:** Design, pre-implementation. Builds on v2 (membrane + goblin + cable, all live).
**Fleet:** LAB 8GB (membrane, door) · LAB 512 (inference appliance, cable-only) · LAB 256 (draft).
**Ledger:** Supabase project `ipfbjwhnhgbxoynqmfxz`, table `public.lab_log`.
**Date:** 2026-06-21

---

## 1. The one-sentence version

The ledger accepts everything, but only **delivers** what is written correctly: a row that names a registered entity's content-hash taps that entity's private realtime channel; a small per-box receiver hears the tap, reads from the **ledger** (never from the message) *how* that entity is to be woken, and runs that wake through the cage. The Universal Inbox and CI are then not infrastructure — they are two registered entities riding the same three receivers.

---

## 2. What v3 adds (and why v1/v2 stalled)

v1 and v2 built the whole body — door, ledger, membrane, cable, goblin, the grammar-cage — and proved every piece. But the inbox never *breathed*: a message arrives, lands raw in the ledger, and **nothing wakes**. The goblin is a one-shot a human runs by hand. The relay (CI) had no path to the boxes at all.

The missing primitive was never "CI" or "the relay." It was **the wake** — and it is the *same* primitive everywhere:

- *What wakes the goblin when a message arrives?*
- *What wakes a box when a row names it?*

One problem. v3 is that one problem, solved once, generically, and then **everything addressable inherits it for free.**

---

## 3. The inversion: from "no need to register" to "the absolute right way"

The ledger is a wild world. Most of what is written will never be addressed to anyone — a heartbeat, a scan, a stray note, a badly-written line (`if_ok: send to johnny` — whoever johnny is). **All of it still registers.** Capture is sacred; registering is itself the thing that builds intelligence and pays the bills. The ledger never rejects.

But registering and *being delivered* are different things, and that gap is the whole engine:

| You write… | It is… | Does it wake anyone? |
|---|---|---|
| nothing | (not in the ledger) | no — invisible |
| an unaddressed row | a kept record | no — inert, but it counts |
| `if_ok: johnny` | a *badly-written* address | no — not a hash, taps nobody |
| `if_ok: <content-hash>` | the **canonical** address | **yes — delivered in realtime** |

So correctness is **pulled, not pushed.** Nobody forces hashes on you. But if you want your message to actually reach its destination's hands, you write it the right way — and the right way, canonicalized, comes from one place: the CLI. The reward for writing it correctly is **delivery**. The reward for registering at all is **memory**.

And symmetrically, on the receiving side: if you want to *be* reachable, you **register yourself the right way** — your kind, your fields, every path you can be triggered at — so that a tap on your frequency tells the receiver exactly how to wake you.

**Two halves of one discipline:** register yourself fully if you want to be woken; address canonically if you want to be heard. Everything in between still lives in the ledger — it just doesn't ring a bell.

---

## 4. Principles

1. **The ledger accepts everything; it delivers only the well-formed.** Permissive capture, earned delivery.
2. **The receiver reads the ledger, not the message.** *How* to wake an entity comes from that entity's own registration. The message only triggers; the registration governs.
3. **The sender can trigger, never define.** A row that names your hash can only cause you to wake *the way your own wake-spec says*. No message can smuggle in behavior.
4. **Deterministic by default; inference only when the spec declares it.** The wake-spec picks the mode; the receiver stays generic.
5. **Coded once, shared by all.** Three receivers, one per box, permanent. New capability = a new registered entity, never new infrastructure.
6. **Append-only, content-hashed, all the way down** — the tap, the wake, the receipt. Same discipline as the rest of the fleet, now applied to the act of waking.

---

## 5. The ledger substrate (verified, not assumed)

Confirmed against the live project on 2026-06-21:

- **Row shape:** `who · did · this · when · status · data(jsonb)` + minted `content_hash` (JCS-RFC8785 + sha256) + `json_canonicalization: "jcs-rfc8785"`. (`store_object`, `src/main.rs:255`.)
- **`lab write lab_log '<json>'`** writes a *fully custom* row (your own `who`, your own fields) and mints the hash. **`lab emit`** force-stamps `who = hostname()` — so cross-identity authors (GitHub, the membrane-on-behalf-of-a-sender) use `write`/`send`, not `emit`.
- **`lab_log` is NOT in the `postgres_changes` publication** — and stays out. That path can't filter on a hash inside `data` and would broadcast the whole ledger. We don't want a public shout.
- **`realtime.send(payload jsonb, event text, topic text, private boolean)` exists** on this project. Broadcast-from-database is available. **This is the keystone** — a row's arrival can tap *only the channels it names*, privately.

---

## 6. The three receivers (the permanent infrastructure)

Three small realtime servers — **one per box** — coded once. Each is a **ledger-driven wake dispatcher**, not a message processor. It knows exactly one trick:

```
tap on frequency X
  → read the wake-spec where frequency == X        (from the LEDGER, not the message)
  → deterministic?  run its registered verb/argv through the cage
     inference?      POST the source row to the membrane, act on the verb it returns
  → write an 'awakened' receipt citing the tap's content_hash   (append-only · idempotent)
```

Properties that keep this from sprawling into per-entity daemons:

- **One receiver per box, not per entity.** A single websocket multiplexes many channel subscriptions (Phoenix channels). The frequencies of every entity *resident on that box* ride one connection. Entities are **entries on a subscription list**; adding one is appending a hash, not standing up a server.
- **Identity ≠ infrastructure.** What scales is the *list of frequencies* (cheap, unlimited — just hashes), never the *count of listeners* (fixed at three).
- **Generic and permanent.** The receivers contain no knowledge of goblins or builds. Those are wake-specs they look up.

> **Cold-start / missed-tap safety:** realtime is the fast path, not the only path. On boot (or on a slow timer) a receiver also does a **filtered pull** (`lab mine`) of un-receipted rows naming its frequencies, so a tap missed while offline is still served. Delivery is at-least-once; the `awakened` receipt makes it idempotent.

---

## 7. The wake-spec — the contract

How an entity declares "this is how you wake me." A registered `lab_log` row:

```jsonc
{
  "who":  "<canonical entity id>",       // e.g. "goblin@lab-8gb", "github.com/danvoulez/lab-cli"
  "did":  "awaken-spec",                 // dedicated did → one cheap lookup, never a scan
  "this": "<human label>",
  "when": "<utc>",
  "status": "registered",
  "data": {
    "kind": "people|object|contract|app|engine|workflow|code",
    "wake": {
      "mode": "deterministic",           // or "inference"
      // deterministic:
      "verb": ["lab","build-relay"],     // a registered argv — run THROUGH THE CAGE, never bash -c
      "paths": ["<every path/endpoint where this can be triggered>"],
      "artifact_hash": "<hash of the binary/artifact, if any>",
      // inference:
      "via": "membrane",                 // POST the source row to the membrane (→ cable → 512)
      "schema": "<decision-grammar id>"  // the strict json_schema the model fills
    }
  }
}
```

- **Frequency = self-address.** The wake-spec's own `content_hash` **is** the entity's frequency. Register your spec → its hash is your address → hand that hash to whoever should reach you. No separate identity registry.
- **Versioning is append-only.** Re-register to evolve; the latest `awaken-spec` for an identity wins, and history is preserved (you can prove what an entity's wake procedure *was* on any day).

A **sender's addressed message** (the other half):

```jsonc
{
  "who":  "<sender>", "did": "<verb>", "this": "<subject>", "when": "<utc>",
  "data": {
    "if_ok":    ["<freq-hash>", "<freq-hash>", "<freq-hash>"],  // deliver to these frequencies
    "if_not":   null,                                           // fallback, or nothing
    "if_doubt": "retry",                                        // sender's stated intent on ambiguity
    "payload":  "<the actual content>"
  }
}
```

---

## 8. The trigger — the radio

One `AFTER INSERT` trigger on `lab_log` carries the whole delivery model:

```
for each value in NEW.data->'if_ok':
    if value matches the canonical hash shape:                  -- a real frequency
        realtime.send(
          payload := jsonb_build_object('source', NEW.content_hash, 'did', NEW.did, 'who', NEW.who),
          event   := 'tap',
          topic   := value,        -- the destination's own private channel
          private := true)
    else:                          -- "johnny" → not a hash → skip
        (no send; the row is still registered)
```

The trigger **is** the incentive engine of §3: canonical hash → tap; anything else → kept but inert. The payload is deliberately minimal — *"something names you; go look"* — never the contents. (Applied only on operator review; it touches the production DB.)

---

## 9. The CLI surface v3 adds

| New verb | What it does | Weight |
|---|---|---|
| `lab register …` | write an `awaken-spec`; print the resulting frequency (its `content_hash`) | trivial (a shaped `write`) |
| `lab send <did> <this> --to <hash>[,<hash>,<hash>] [--data …]` | write a canonical **addressed** row (`emit` + `data.if_ok`) | trivial |
| `lab mine` | filtered pull of un-receipted rows naming my frequencies (cold-start / catch-up) | trivial (a `read`) |
| `lab listen` | the receiver: hold the socket to my frequencies, dispatch each tap per its wake-spec | the one heavy piece — a **plugin**, not kernel Rust (a Supabase realtime / Phoenix-channels client), keeping the kernel clean |

Everything else — hashing, identity, the ledger, the cage, the membrane — already exists.

---

## 10. Security model

- **The record governs, the message triggers.** The receiver never derives behavior from the incoming message — only from the entity's own `awaken-spec`. A maliciously- or badly-written sender can at most *ring a bell that is already wired*.
- **Deterministic wakes run through the cage** (`lab cage -- <argv>`): argv arrays, validated, never `bash -c` from a ledger field. The wake-spec names a *registered* verb; the receiver re-validates before exec.
- **Inference wakes never execute** — they ask the membrane for a verb under a strict grammar, and that verb is itself re-validated against the cage. (This is exactly the goblin's proven path.)
- **Private channels.** `realtime.send(private := true)` — a subscriber must be authorized to hold a frequency's channel. (On the trusted three-box fleet, v1 may relax this; the field is there for when the ecosystem widens.)
- **Idempotency.** Every wake writes an `awakened` receipt citing the tap's `content_hash`; a tap already receipted is skipped. At-least-once delivery + derived handled-ness = exactly-once effect.

---

## 11. Worked example A — Universal Inbox v3 (the goblin)

The universality of the inbox comes from **the membrane writing correctly on behalf of the wild world.**

1. **Arrival.** Any inbound text hits the door (8GB cf tunnel → membrane). It is unaddressed, possibly garbled — the wild norm.
2. **The membrane stamps the goblin's frequency.** Instead of `lab emit inbox <raw>`, the door does `lab send inbox <source> --to <goblin-freq> --data <raw>`. The sender never addressed the goblin; **the membrane does it for them.** That stamping *is* the universality — every door message is forced to tap the goblin.
3. **Trigger → tap.** The row names `goblin-freq`; the trigger taps the goblin's channel.
4. **Receiver (on 8GB) wakes the goblin.** It reads the goblin's `awaken-spec`: `mode: inference, via: membrane`. It POSTs the raw row to the membrane (→ cable → 512), gets back one grammar-constrained verb (`route` / `park`), re-validates it, runs it.
5. **Receipt.** `inbox.routed` / `awakened`, append-only. The inbox finally rings — on its own.

The goblin's *will* stays bounded exactly as v2 designed; v3 only gives it a doorbell. (Note: addressing the goblin is the **inbox's strategy**, not a law — a heartbeat or scan still carries no hash and wakes no one.)

## 12. Worked example B — CI as a registered entity

CI was always "the example." Here it is, as nothing but rows:

1. **Push to `main`.** GitHub (a tiny action with a scoped key, or via the door) **writes correctly**: `who: github.com/danvoulez/lab-cli`, `did: merged`, `this: branched into main`, `data.if_ok: [<8gb-freq>, <512-freq>, <256-freq>]`. It addresses the three boxes because *that is its strategy* — every case is a case.
2. **Trigger → three taps.** One row, three private channels, fired simultaneously — the cinematic synchronized pulse. The story is "the infra is up," told as one beat.
3. **Each box's receiver wakes its build entity.** The box's `awaken-spec` is `mode: deterministic, verb: ["lab","build-relay"]`. Each box, in parallel, runs it through the cage and writes its **own** `relay.beat` receipt (`who: lab-256`, build sha, when) — each node signs its own proof. No box reaches another; the reverse-SSH problem dissolves entirely.
4. **The artifact becomes a fact.** The built binary is registered with its **hash + trigger path** on the ledger. "Build done" is no longer a side effect — it is a row pointing to a hash-verified, triggerable artifact, which a *future* deterministic wake can fire.

Same three receivers, same trigger, same row format as the inbox — a *different addressing strategy*. That is the whole claim of v3: capability is data, not code.

---

## 13. The ecosystem this generalizes to

Anything that can be **pointed at** registers on the ledger and inherits the wake for free. The entity kinds (`data.kind`):

**People · Object · Contract · App · Engine · Workflow · Code.**

A person has an inbox; a contract fires when its condition is named; an app exposes a triggerable endpoint; an engine wakes on work; a workflow advances a step; a piece of code is a built artifact with a hash and a path. All of them: *register the right way, be reachable; address the right way, be heard.* The three receivers don't grow — the list of frequencies does.

---

## 14. Verified vs. to-build

**Verified live (2026-06-21):**
- Ledger row shape + JCS/sha256 hashing (`src/main.rs`).
- `realtime.send` present; `lab_log` absent from `postgres_changes` (both as desired).
- Membrane live on 8GB (door + cable-proxy); goblin present but **manual / one-shot** (no wake) — the gap v3 closes.
- Cable 8GB↔512 up.

**To build:**
- `lab register`, `lab send --to`, `lab mine` (trivial CLI verbs).
- `lab listen` receiver **plugin** (the one heavy piece).
- The `AFTER INSERT` trigger on `lab_log` (operator-reviewed; touches prod DB).
- The membrane change: door stamps `--to <goblin-freq>`.
- Two seed `awaken-spec` rows: the goblin (inference) and `build-relay` (deterministic).

---

## 15. Scope & sequencing

**v3-of-the-build (first installable):**
- The trigger (delivery) + `lab register` + `lab send --to` + `lab mine`.
- One receiver on 8GB first, waking the goblin (the inbox finally breathes end-to-end).
- Goblin + `build-relay` registered as `awaken-spec`s.
- Then the same receiver dropped onto 256 and 512 → CI fires across all three.

**Sequencing choice (the one live decision):** the receiver can start as a **poll loop** (`lab mine` on a launchd timer — same outbound HTTPS the CLI already makes, breathing *today*, zero socket) and upgrade to the **realtime websocket** with no row-format change (the trigger's `realtime.send` is already firing). Lean: poll first, socket as the clean drop-in.

**Deferred:** private-channel authorization hardening; richer `if_not`/`if_doubt` semantics; non-fleet (external) receivers; the artifact-trigger wake (fire a registered binary by hash).

---

## 16. Open decisions

1. **Receiver wake cadence:** poll-loop-first (lean) vs straight to realtime websocket.
2. **Private channels:** enforce `private:true` authz now, or relax on the trusted fleet for v1.
3. **`build-relay` first form:** heartbeat-only (reachability proof, no real cargo build) — *chosen* — fired as a simultaneous fan-out.
4. **Goblin frequency:** declared `awaken-spec` content-hash (canonical) vs a pinned well-known constant for v1.
5. **`if_doubt` handling:** receiver-side retry policy vs leaving it advisory for now.

---

## 17. Glossary

- **Frequency** — an entity's address: the `content_hash` of its `awaken-spec`. To reach it, name it in `data.if_ok`.
- **Wake-spec** — a registered row declaring *how* an entity is woken (deterministic verb, or inference via membrane) and every path it can be triggered at.
- **Receiver** — one of three permanent per-box servers; a ledger-driven wake dispatcher. Reads the wake-spec, not the message.
- **Tap** — the minimal private realtime signal a trigger sends to a named frequency: *"go look, something names you."*
- **Addressed message** — a canonical row whose `data.if_ok` carries one or more frequencies; the "written correctly" form that earns delivery.
- **Cage** — `lab cage -- <argv>`; the seatbelt every deterministic wake runs through.
- **Entity** — anything pointable: People, Object, Contract, App, Engine, Workflow, Code.

---

*v3 principle, in one line: the ledger remembers everyone, but only wakes those who registered the right way for those who addressed them the right way — and the waking is three small servers, coded once, that read from the ledger how to ring each bell.*
