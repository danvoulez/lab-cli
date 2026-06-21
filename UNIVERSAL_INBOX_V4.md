# The Universal Inbox — v4 · The Abstraction Ladder

*A theory of **dynamic projections**: how a bounded model thinks over an unbounded ledger without drowning, by climbing levels of abstraction instead of swallowing rows — and why content-addressing under JCS-RFC8785 is what makes every level above the ledger safely disposable.*

**Status:** Design / theory, pre-implementation. Builds on v3 (the wake) — the body breathes; v4 is how it thinks.
**Fleet:** LAB 8GB (membrane, door) · LAB 512 (inference appliance) · LAB 256 (draft).
**Ledger:** Supabase `lab_log`, rows JCS-RFC8785-canonicalized + sha256 (`store_object`, `src/main.rs:255`).
**Date:** 2026-06-21

---

## 1. The one-sentence version

The context limit is not a token problem, it is a *level* problem; so the model never reads the ledger — it reads **projections**, each a hash-pinned, deterministically-rebuildable view one rung more abstract than the last, climbing 100,000 rows down to a handful of thinkable objects, and every rung it stands on can be thrown away because JCS-canonical content-hashes make it reconstructible from admitted memory alone.

---

## 2. The problem v4 names

v1–v3 gave the fleet existence, reachability, permitted behavior, audit, and the wake. What they did not give it was **cognition at scale**. The ledger only grows. Point any model at it directly and the failure is not subtle:

```
ledger rows → too many facts → context ceiling → shallow reasoning
```

The naive fix — "fit more rows in the window" — misreads the constraint. Seeing 100,000 rows is not 100,000× as useful as seeing one; it is *useless*, because no reasoning survives that much undifferentiated fact. The scarce resource is not tokens. It is **the right object at the right altitude.**

> **The v4 reframing:** a context window is not a budget to pack. It is a *ceiling*. You beat a ceiling by adding **floors beneath it**, not by compressing what you stuff against it.

---

## 3. What a projection *is* (the definition)

A **projection** is a pure, deterministic function from a set of admitted ledger rows to a derived view, where the view **cites** — by content-hash — exactly the rows (or lower projections) it was built from.

```
P : { h₁, h₂, …, hₙ }  →  V
        where each hᵢ is a JCS-content-hash of an admitted object,
        and V records the multiset { h₁…hₙ } it consumed.
```

Three clauses, each load-bearing:

1. **Pure & deterministic.** `P` is a function of its inputs and nothing else — no clock, no ambient state, no hidden source. Same input hashes ⇒ same output, on any box, on any day.
2. **Cites its inputs by hash.** A projection does not *copy* the rows it summarizes; it *points at* them. The view is a list of addresses plus a transform, never a second copy of the facts.
3. **Derives downward only.** A projection's inputs are always *lower* (closer to the ledger) than the projection itself. The ladder has a direction; nothing higher is ever an input to something lower.

A view that violates any clause is not a projection — it is a new assertion, and must go through admission (§7) before anything may stand on it.

---

## 4. The ladder (the levels, as types)

Each rung is a projection whose inputs are the rung below. The numbers are not decoration — they are the *altitude*, and altitude is what makes an object thinkable.

```
L0  admitted ledger rows          (existence — the only source of truth)
L1  entities / receipts            P over L0     — "who, what, what rang"
L2  clusters                       P over L1     — raw groupings, exact-dedup'd
L3  timelines / open cases         P over L2     — shape across time
L4  failure modes                  P over L3     — recurring structure
L5  candidate acts                 P over L4     — "3 proposed changes"
```

The compression is brutal and that is the point:

```
100,000 rows
  → 1,200 malformed attempts        (L2, after exact dedup)
  → 37 clusters                     (L3, fuzzy intent grouping)
  → 8 recurring failure modes       (L4)
  → 3 candidate doctrine changes    (L5)
```

`100,000` is unthinkable. `37` is thinkable. `3` is *governable*. The model did not get smarter and the window did not get bigger — the object got smaller while staying **faithful**, because every rung still points all the way back down to L0.

---

## 5. Two classes of projection (the critical split)

Not every rung is built the same way, and conflating them is the one mistake that collapses the theory.

### 5.1 Stable projections (deterministic transforms)

Plain code: filters, joins, group-bys, counts, dedup. Given the same input hashes, a stable projection re-derives **bit-identically** anywhere. These are the load-bearing floors — `entities`, `receipts`, `malformed_addresses`, `build_beats`. They are *safe to cache* precisely because caching them is indistinguishable from recomputing them.

### 5.2 Dynamic projections (inferred transforms)

The model conjures these on demand: *"cluster these malformed rows by probable intended entity,"* *"group failed wakes by missing capability."* This is the **scratch space** — temporary cognitive shapes, summoned for one thought and discarded.

A dynamic projection is **not deterministic** in general: an LLM doing intent-clustering can return a different shape on re-run. So it does **not** get the stable-projection guarantee — and must never be cached as if it did.

> **The discipline that saves it:** a dynamic projection is reproducible **iff** its transform is pinned — `(model_id, prompt, decode_params, seed)` recorded alongside the input hashes. Pin the transform and a dynamic projection re-derives like a stable one. Leave it unpinned and it is a *candidate*, never a cache: legitimate to reason from, illegitimate to stand on, must be re-derived (or admitted) before anything downstream depends on it.

This is not a new rule. It is **exactly** the v3 law applied one storey up: *the record governs, the message triggers.* Here: **admitted memory governs, inference triggers.** A dynamic projection is the model "triggering" a thought; nothing becomes real until L5 → admission.

---

## 6. The invariant (the whole theory in one line)

> **Every level above L0 must be disposable — i.e. reconstructible from admitted lower memory alone.**

Everything in v4 exists to make this invariant *hold* rather than *be hoped for*. Restated as an operational test:

> Delete every projection in the fleet. If the system cannot rebuild each one, byte-for-byte (stable) or candidate-for-candidate (dynamic, via its pinned transform), from L0 alone — then some projection had quietly become a **second source of truth**, and the invariant is already broken.

The invariant is what stops Mongo from becoming canon, stops the UI from becoming truth, stops a cached cluster from outliving the rows it summarized. It is the same discipline as v1's *"interpretation is separate and disposable"* — now generalized from one interpretation step to an entire ladder of them.

---

## 7. Why content-addressing — and why JCS specifically — is load-bearing

The invariant is a promise about *reconstruction*. A promise about reconstruction is only a *guarantee* if you can verify that a view was derived from specific, unchanged inputs. That verification is exactly what a content-hash provides, and exactly what a *canonical* content-hash provides **reliably**.

**1. It makes the ladder cheap — pointers, not payloads.** A cluster of 1,200 rows is 1,200 hashes, not 1,200 copies. Each rung compresses *addresses*; the data stays at L0. The whole stack is hash-linked, so altitude costs almost nothing.

**2. It makes the invariant *checkable*, not aspirational.** A projection citing its input hashes is falsifiable: re-run the transform over those hashes; a stable projection must reproduce exactly. If an input changed, its hash changed, and any view citing the old hash is *visibly* stale. Disposability becomes a property you can test, not a rule you ask people to follow.

**3. It makes exact dedup free — and this is where JCS earns its place.** The `100,000 → 1,200` collapse is exact-duplicate removal, and it is only exact if *semantic* sameness equals *byte* sameness. A naive hash breaks the instant two sources emit the same fact as `{"a":1,"b":2}` vs `{"b":2,"a":1}` — same meaning, different bytes, two hashes, and the dedup leaks silently. **JCS-RFC8785 closes that gap by construction**: keys sorted, numbers in canonical I-JSON/ECMAScript form, insignificant whitespace gone, UTF-8 settled. Semantic identity *is* byte identity *is* hash identity. The model does the *interesting* compression (intent); JCS already did the *boring* compression (exact dedup), correctly, for free.

**4. It makes reconstruction *fleet-wide and implementation-independent*.** "Re-derive anywhere and agree" requires 8GB, 512, and 256 to canonicalize identically — without coordinating. Because RFC 8785 is a *spec with published test vectors*, every box conforms to the same external standard rather than to whatever a hand-rolled `canonicalize()` did on the box that happened to write the row. A hand-rolled scheme makes the hash a property of *your binary*; JCS makes it a property of *the data*. That is the difference between "rebuildable if you have my exact code" and "rebuildable by anyone who implements 8785." The audit in §8 is therefore cross-implementation, not merely cross-run.

**5. It forecloses a class of *silent* corruption.** Canonicalization is the kind of code that is easy to get *almost* right and catastrophic to get *slightly* wrong, because the failure never raises — it just mints two hashes for one fact. Mishandle `-0` vs `0`, `1e3` vs `1000`, or a non-BMP codepoint, and dedup and derivation-integrity erode invisibly, months later, undebuggable because nothing ever errored. Conforming to 8785-with-test-vectors retires that whole class by *conformance* rather than by hope. For a system whose entire claim is "the higher floors rebuild from admitted memory," the canonicalizer being a *checkable standard* is structural, not a detail.

> **The upgrade, precisely:** content-addressing alone turns the ladder from a *lossy summary chain* into a *verifiable derivation chain*. JCS is what makes the leaves of that chain identical across sources, boxes, and implementations — so "verifiable" is a property anyone can re-check, not a property of our particular code.

---

## 8. The cognitive loop (where the ladder meets the gate)

The ladder is climbable in **both** directions, and that is what makes the model's thought legible to authority.

```
ascend (compress):   L0 → L1 → … → L5      the model climbs to a thinkable object
propose:             L5 → candidate row    a thought becomes a possible institutional act
descend (audit):     candidate → cited hashes → L0   the gate checks the thought to ground
admit:               gate accepts → admitted L0 row   only here does thought become real
```

When the model proposes at L5 — a new alias, a wake-spec update, a doctrine note — the candidate **cites the exact evidence it climbed**: *"this alias, because of these 37 clustered hashes."* The gate is never asked to trust the summary. It can descend the cited hashes to the leaf rows and verify before admitting anything. Content-addressing is what makes that descent exact; JCS is what makes it agree across boxes.

The model never becomes authority. It climbs to a thinkable altitude, proposes downward, and the gate — descending the same hash-links it climbed — decides. *The ledger gives existence; the projection gives a thinkable shape; the gate gives authority.* Mongo, throughout, is neither memory nor truth nor gate — it is **cognitive scaffolding over institutional memory**, disposable by construction.

---

## 9. What v4 adds (CLI-first, per the standing principle)

Everything new is a verb or a plugin; nothing reimplements the kernel.

| New thing | Kind | What it is |
|---|---|---|
| `lab project <spec>` | plugin | run a **stable** projection: pure transform over cited input hashes → a view that records its inputs |
| `lab think <spec>` | plugin | run a **dynamic** projection: POST inputs to the membrane, record the result *plus* the pinned `(model, prompt, params, seed)` transform |
| `lab rebuild <view-hash>` | plugin | re-derive a projection from its cited inputs; assert byte-identity (stable) or re-pin (dynamic) — **the invariant, as a command** |
| `lab cite <view-hash>` | plugin | descend a view to the leaf L0 rows it stands on — the gate's audit primitive |
| `projection-spec` did | convention | a registered, versioned transform — same `awaken-spec` discipline (§v3), now for views |

Deferred / unchanged: the ledger, JCS hashing, identity, the wake, the cage, the membrane — all already proved in v1–v3. Mongo is *only* the materialization target for `lab project`; it holds no authority and every Mongo document is rebuildable from L0 by `lab rebuild`.

---

## 10. Worked example — taming the malformed-address noise

The wild ledger is full of `if_ok: johnny` — addressed to nobody, kept but inert (v3 §3). Over months these accumulate. v4 turns that noise into governance:

1. **`lab project malformed_addresses`** (stable) — filter L0 for `if_ok` values that are not canonical hashes. Exact-dedup is free (JCS): `100,000 → 1,200`. The view cites all 1,200 row-hashes.
2. **`lab think "cluster by probable intended entity"`** (dynamic) — the model groups the 1,200 into `37` clusters by likely intent, against the live `awaken-spec` registry. The view records the 37 groupings *and* its pinned transform.
3. **`lab think "which missing aliases would collapse these"`** (dynamic) — `37 → 8` recurring failure modes → `3` candidate aliases.
4. **Propose (L5).** The model writes 3 candidate `alias` rows, each citing the cluster-hashes that justify it.
5. **Descend & admit.** The gate runs `lab cite` on each candidate, lands at the leaf rows, confirms the intent, and admits the aliases. Next cycle, those senders' `if_ok: johnny` resolves to a real frequency — the noise becomes delivery.

No floor here is canon. Delete every projection and `lab rebuild` reconstructs the stable ones byte-for-byte and re-pins the dynamic ones from L0 alone. The model thought; the ledger remembered; the gate decided. *Capability is data, not code* — now true of cognition, too.

---

## 11. Open decisions

1. **Dynamic-pin strictness:** record `(model, prompt, params, seed)` as a hard reproducibility contract, or treat all dynamic projections as candidates-only (never reproducible, always re-derived). Lean: pin, so a dynamic view *can* be promoted to stable when its transform stabilizes.
2. **Mongo as the only materialization target** vs. a pluggable view-store. Lean: Mongo for v1 (it is already the shaped shadow), kept strictly downstream of `lab rebuild`.
3. **Eager vs. lazy stable projections:** materialize on admission, or compute on `lab project` call. Lean: lazy first; the fleet is not a firehose.
4. **`lab rebuild` cadence:** an invariant-check on a slow timer (prove disposability continuously) vs. on-demand only. Lean: slow timer — the invariant is the doctrine; prove it lives.
5. **Altitude typing:** enforce "inputs strictly lower than output" in the `projection-spec`, or leave it a convention. Lean: enforce — it is what keeps the ladder a ladder.

---

## 12. Glossary

- **Projection** — a pure, deterministic view that cites (by JCS-hash) the lower rows it was derived from; disposable, rebuildable.
- **Stable projection** — a code transform; re-derives bit-identically anywhere. Safe to cache because caching equals recomputing.
- **Dynamic projection** — an inferred (LLM) transform; reproducible only if its `(model, prompt, params, seed)` is pinned. Otherwise a candidate, never a cache.
- **Altitude** — a rung's distance above L0; higher = more compressed, more thinkable, less granular. Inputs must always be lower than outputs.
- **The invariant** — every level above L0 is reconstructible from admitted lower memory alone. The whole theory in one line.
- **Scaffolding (Mongo)** — neither memory nor truth nor gate; the disposable materialization of projections over institutional memory.
- **Ascend / descend** — compress L0→L5 to think; audit L5→L0 to govern. The ladder is climbable both ways.

---

*v4 principle, in one line: you beat the ceiling by adding floors beneath it — and a floor is only safe to stand on if, under JCS, it can be rebuilt from the ground it points to.*
