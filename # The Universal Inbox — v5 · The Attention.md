# The Universal Inbox — v5 · The Attention Field

*A doctrine of **situated presence**: how a Lab that remembers everything, wakes conformant things, and thinks through disposable projections avoids drowning its human, agents, and surfaces — by making attention itself ledgered, scarce, cited, temporary, and revocable.*

**Status:** Design / theory, pre-implementation. Builds on v3 (the wake) and v4 (the abstraction ladder).
**Fleet:** LAB 8GB (membrane, door, receiver) · LAB 512 (inference appliance) · LAB 256 (draft).
**Ledger:** Supabase `lab_log`, admitted rows JCS-RFC8785-canonicalized + sha256.
**Date:** 2026-06-21

---

## 1. The one-sentence version

The Lab may remember everything, wake only conformant things, and think through rebuildable projections — but only a small number of objects may enter the active field of attention, and that field must itself be ledgered, cited, budgeted, temporary, and revocable.

---

## 2. The problem v5 names

v1–v4 solved four different kinds of drowning.

v1 solved ingress: do not normalize the world before remembering it.
v2 solved containment: keep the brain caged and the membrane thin.
v3 solved wake: registered things can be rung by canonical address.
v4 solved cognition: the model climbs projections instead of swallowing rows.

But once all that works, a new failure appears:

```
memory is infinite
wake is automatic
projection is cheap
thought becomes abundant
attention collapses
```

The Lab can now see too much, derive too much, cluster too much, summarize too much, and produce too many valid objects. The danger is no longer blindness. The danger is **presence without discipline**.

A thing may exist.
A thing may be conformant.
A thing may have woken.
A thing may be true.
A thing may be brilliantly projected.

It still may not deserve attention right now.

That is the v5 boundary.

---

## 3. The inversion

The old UI model says:

```
if it exists, show it
if it changed, notify
if it is urgent-looking, interrupt
if the query returns it, render it
```

That model is unusable in a universal ledger.

The v5 model says:

```
existence is not presence
truth is not urgency
projection is not attention
notification is not entitlement
```

Attention is not a side effect of data. Attention is an admitted, budgeted, revocable institutional state.

The core inversion:

```
Memory is universal.
Wake is earned.
Thought is disposable.
Attention is scarce.
```

---

## 4. The attention field

The **attention field** is the small, active set of things the Lab is currently allowed to bring forward to a person, agent, workflow, or surface.

It is not the inbox.
It is not Mongo.
It is not the projection store.
It is not a notification queue.
It is not a feed.

It is a ledgered field of situated claims:

```
this deserves presence
for this owner
for this reason
until this time
at this priority
with this evidence
toward this next action
```

An attention object is therefore not merely “important.” It is **important somewhere, for someone, for a while, because of cited memory**.

---

## 5. The attention object

An attention object is an admitted row, not a frontend state.

Example:

```json
{
  "who": "lab",
  "did": "attention.entered",
  "this": "malformed Johnny addresses are recurring",
  "when": "2026-06-21T18:00:00Z",
  "status": "active",
  "data": {
    "owner": "dan",
    "surface": "lab.dashboard",
    "priority": "medium",
    "reason": "37 clustered malformed address rows suggest one missing alias",
    "evidence": [
      "<projection_hash>",
      "<cluster_hash>",
      "<leaf_row_hash>"
    ],
    "why_now": "third recurrence in 24h",
    "next_smallest_action": "<candidate_alias_card_hash>",
    "ttl": "48h",
    "suppression_key": "malformed-address:johnny",
    "cooldown": "24h"
  }
}
```

The object does not say: “this is true because I say so.”

It says:

```
this deserves attention because it stands on these hashes
```

As in v4, the gate can descend.

---

## 6. Principles

1. **Existence is not presence.**
   A row can exist forever without ever entering anyone’s active field.

2. **Attention must cite evidence.**
   Nothing may ask for presence without pointing to the rows, receipts, projections, or candidates that justify it.

3. **Attention is owner-relative.**
   “Important” is incomplete. Important to whom? Dan, a receiver, a workflow, an agent, a surface, a contract?

4. **Attention is temporary by default.**
   Every attention object has a TTL, expiry, dismissal, cooldown, or review point. Ambient permanence is a leak.

5. **Attention is budgeted.**
   A surface has finite slots. If a new object enters, something else may need to leave, collapse, defer, or summarize.

6. **Attention is revocable.**
   Suppressed, resolved, expired, dismissed, merged, delegated, or superseded attention must be recorded as rows.

7. **Attention is not authority.**
   Entering attention does not make a claim true. It only makes a claim present.

8. **Notification is a privileged form of attention.**
   A notification is not just display. It is interruption. It requires a higher burden than appearance in a surface.

---

## 7. The stack after v5

The Lab now has a clearer vertical order:

```
L0  admitted ledger rows          existence
L1  receipts / entities           observed institutional state
L2  stable projections            reusable legibility
L3  dynamic projections           disposable thought-shapes
L4  compressed findings           failure modes, clusters, trends
L5  candidate acts                possible institutional changes
L6  attention objects             situated presence
L7  notifications                 interruption
```

The important thing: L6 is not “more true” than L5. It is more situated.

It answers:

```
why this?
why now?
for whom?
for how long?
with what evidence?
what is the next smallest action?
what happens if ignored?
```

---

## 8. Budgets

Every active surface has a budget.

Examples:

```json
{
  "surface": "dan.morning",
  "slots": 7,
  "max_interruptions": 1,
  "priority_floor": "medium",
  "collapse_by": ["suppression_key", "workflow", "entity"],
  "quiet_hours": ["22:00", "08:00"]
}
```

```json
{
  "surface": "agent.operator",
  "slots": 20,
  "max_interruptions": 0,
  "priority_floor": "low",
  "collapse_by": ["work_order", "failure_mode"]
}
```

```json
{
  "surface": "lab.wall",
  "slots": 12,
  "max_interruptions": 0,
  "priority_floor": "ambient",
  "collapse_by": ["domain", "machine", "workflow"]
}
```

A budget prevents projection abundance from becoming attentional spam.

The law:

```
No projection gets to bother Dan merely because it exists.
```

---

## 9. Attention states

Attention should not be boolean. It has lifecycle.

```
candidate_attention
  → entered
  → active
  → acknowledged
  → delegated
  → snoozed
  → suppressed
  → resolved
  → expired
  → superseded
```

Each transition is a row.

A dismissal is not deletion. It is memory:

```json
{
  "did": "attention.dismissed",
  "this": "<attention_hash>",
  "status": "closed",
  "data": {
    "by": "dan",
    "reason": "not relevant this week",
    "cooldown": "7d"
  }
}
```

This lets the Lab learn Dan’s attention boundaries without turning UI clicks into hidden state.

---

## 10. The ranking function

Ranking is not canon. It is a projection over candidate attention objects.

A ranker may consider:

```
urgency
risk
novelty
blockedness
owner
recurrence
freshness
cost of delay
confidence
available next action
relationship to active mission
explicit human preference
cooldown / suppression history
```

But the output is still a derived object:

```json
{
  "did": "attention.ranked",
  "this": "dan.dashboard.active",
  "data": {
    "inputs": ["<attention_hash_1>", "<attention_hash_2>"],
    "ranker": "<projection_spec_hash>",
    "ordered": ["<attention_hash_2>", "<attention_hash_1>"],
    "budget": "<attention_budget_hash>"
  }
}
```

If the ranker is deterministic, it is rebuildable.
If the ranker is inferred, it must pin model, prompt, params, seed, and evidence.

Attention ranking is allowed to shape presence. It is not allowed to create truth.

---

## 11. Surfaces

A surface is not a database query. A surface is an admitted view over an attention field.

Examples:

```
dan.now
dan.morning
lab.dashboard
machine.8gb.operator
agent.goblin.work
workflow.release.active
contract.review.pending
```

Each surface has:

```
owner
slot budget
priority floor
allowed object kinds
collapse rules
quiet rules
notification rules
expiry rules
```

The UI reads the surface. It does not invent it.

Vercel is glass, not hands.

---

## 12. Notification as escalation

Notification is not the same as attention.

Attention means:

```
this may appear in the active field
```

Notification means:

```
this may interrupt someone
```

So notification has stricter requirements:

```
active attention object
owner known
reason cited
priority above threshold
cooldown clear
not suppressed
next action available or risk explicit
```

A notification without a next action is usually noise.

A valid notification should be able to say:

```
I am interrupting you because:
- this evidence changed
- this attention object is active
- this budget allows interruption
- this cooldown is clear
- this is the smallest next action
```

---

## 13. The mercy layer

v5 is not only about priority. It is about mercy.

A universal ledger can become cruel if every true thing screams.

The Lab must know how to say:

```
not now
not Dan
not this surface
not again today
summarize instead
collapse these ten into one
wait until there is an action
hold until recurrence crosses threshold
```

Silence is not ignorance.
Silence is governed attention.

This is the difference between a system that remembers everything and a system that burdens everyone with everything it remembers.

---

## 14. The CLI surface v5 adds

Everything new remains CLI-first.

| New thing                                    | Kind       | What it is                                                    |
| -------------------------------------------- | ---------- | ------------------------------------------------------------- |
| `lab attend <hash>`                          | plugin     | propose or enter an attention object citing evidence          |
| `lab attention list --surface <name>`        | plugin     | materialize the active attention field for a surface          |
| `lab attention rank --surface <name>`        | plugin     | rank active/candidate attention under a budget                |
| `lab attention dismiss <hash>`               | plugin     | close an attention object with reason/cooldown                |
| `lab attention snooze <hash> --until <time>` | plugin     | temporarily remove from active presence                       |
| `lab attention resolve <hash>`               | plugin     | mark attention satisfied by cited receipt/candidate/admission |
| `attention-budget` did                       | convention | registered surface budget and interruption rules              |
| `attention.entered` did                      | convention | admitted claim that something deserves presence               |
| `attention.ranked` did                       | convention | rebuildable ordering of active attention objects              |
| `attention.notified` did                     | convention | interruption receipt                                          |

Mongo may materialize attention fields, but only from ledger rows. A Mongo collection like `active_attention` is a disposable surface cache, not the source of presence.

---

## 15. Worked example A — the malformed Johnny cluster

v4 finds a pattern:

```
1,200 malformed address rows
  → 37 clusters
  → 8 recurring failure modes
  → 3 candidate aliases
```

v5 asks a different question:

```
Does this deserve Dan’s attention right now?
```

The answer may be yes only if it passes the attention threshold:

```
recurs often
blocks delivery
has a clear candidate alias
affects a known sender or workflow
has a small next action
is not already suppressed
```

Then the Lab writes:

```json
{
  "did": "attention.entered",
  "this": "Possible alias needed for Johnny",
  "status": "active",
  "data": {
    "owner": "dan",
    "priority": "medium",
    "why_now": "37 malformed rows clustered to the same probable entity",
    "evidence": ["<cluster_projection_hash>"],
    "next_smallest_action": "<candidate_alias_hash>",
    "ttl": "48h"
  }
}
```

The dashboard shows one card, not 37 rows.

If Dan dismisses it, that dismissal is remembered.
If Dan admits the alias, the attention resolves.
If more rows arrive after cooldown, a new attention object may be justified.

---

## 16. Worked example B — build pulse across machines

A CI row wakes three machines. Each writes a receipt.

v4 can project:

```
8GB beat: ok
512 beat: ok
256 beat: missing
```

v5 decides:

```
Does this enter attention?
```

If all three are ok, maybe it becomes ambient history only.

If 256 misses two beats, it becomes active attention:

```json
{
  "did": "attention.entered",
  "this": "LAB 256 missed build relay twice",
  "status": "active",
  "data": {
    "owner": "dan",
    "surface": "lab.dashboard",
    "priority": "high",
    "why_now": "two consecutive missing relay receipts",
    "evidence": [
      "<relay_projection_hash>",
      "<missing_receipt_case_hash>"
    ],
    "next_smallest_action": "check receiver on LAB 256",
    "ttl": "12h"
  }
}
```

If it affects release, it may notify.
If it is merely historical, it stays in the surface.

---

## 17. Worked example C — agent spam

An agent produces many parked rows.

The naive surface shows all parks.
The v5 surface collapses them.

```
42 parked rows
  → 5 clusters
  → 1 repeated missing tool
  → 1 attention object
```

The attention card says:

```
Goblin repeatedly needs a route for GitHub release webhook.
```

It cites the parked rows and offers one next action:

```
create or admit a release-webhook route verb
```

Dan sees the missing capability, not the spam.

---

## 18. Relationship to Mongo

Mongo remains invisible scaffolding.

It may hold:

```
active_attention
attention_by_surface
attention_rankings
suppression_keys
cooldowns
surface_budgets
attention_history
```

But Mongo never creates attention.

Mongo changes only because ledger rows changed.

If Mongo is deleted, attention can be rebuilt.
If the ledger is deleted, Mongo must be blank.
If a Mongo attention document cannot cite ledger ancestry, it is a leak.

Mongo is the mirror.
The ledger is the world.
Attention is an admitted claim about what may become present.

---

## 19. The invariant

The v5 invariant:

> Attention must be derived, cited, temporary, budgeted, and revocable.

Operational test:

```
Why is this visible?
Why is it visible here?
Why is it visible now?
Who is it for?
What evidence does it cite?
When does it expire?
What suppresses it?
What resolves it?
What is the smallest next action?
```

If the system cannot answer those questions, the object does not belong in the active field.

---

## 20. Scope & sequencing

First installable v5:

1. `attention.entered` row convention.
2. `attention-budget` row convention.
3. One surface: `dan.now`.
4. One materialized Mongo collection: `active_attention`.
5. `lab attend`, `lab attention list`, `lab attention dismiss`.
6. First ranker: deterministic priority + TTL + cooldown.
7. First producers:

   * malformed-address clusters
   * missed relay beats
   * parked goblin rows
8. First hard rule:

   * no notification without an active attention object.

Deferred:

* inferred rankers
* multi-owner attention
* agent-specific attention fields
* calendar-aware quiet hours
* rich delegation
* notification channels beyond dashboard
* learned suppression policies

---

## 21. Open decisions

1. **Attention admission:** should `attention.entered` require gate admission, or may trusted projectors write it directly as admitted operational state? Lean: trusted deterministic projectors may enter low/medium attention; high/interruption requires stricter policy.

2. **Budget enforcement:** hard cap vs soft ranking. Lean: hard cap per surface, with overflow summarized.

3. **Notification threshold:** priority-only vs priority plus next action. Lean: require next action unless risk is explicit.

4. **Suppression keys:** generated deterministically vs proposed dynamically. Lean: deterministic where possible; inferred suppression keys must cite their transform.

5. **Human dismissals:** should dismissals train future rankers automatically? Lean: no hidden training. Dismissals become evidence; new suppression policies must be proposed and admitted.

---

## 22. Glossary

* **Attention field** — the active, budgeted set of objects allowed to become present for an owner or surface.
* **Attention object** — a ledgered claim that something deserves presence, citing evidence, owner, reason, priority, TTL, and next action.
* **Surface** — a named view over an attention field, such as `dan.now` or `lab.dashboard`.
* **Budget** — the slot, priority, interruption, quiet-hour, and collapse rules for a surface.
* **Presence** — the state of being actively shown or considered.
* **Notification** — interruption; a privileged escalation of attention.
* **Suppression key** — a stable grouping key that prevents repeated attention spam.
* **Cooldown** — a period during which similar attention should not re-enter unless conditions change.
* **Mercy layer** — the discipline that prevents true things from becoming burdens merely because they are true.

---

*v5 principle, in one line: the Lab may remember everything, wake the conformant, and think through projections — but only cited, budgeted, temporary claims may become present.*
