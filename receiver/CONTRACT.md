# v3 receiver — contract

**Release:** *v3 receiver — manual deterministic wake, single box, canonical receipts.*

One small, boring, trustworthy thing: a per-box listener that turns a canonical
tap into a recorded, bounded action. Nothing more.

## What it does

```
act enters logline_acts (if_ok names a frequency)
  → postgres_changes realtime taps this receiver   (one socket, all my frequencies)
  → receiver reads the entity's wake-spec FROM THE LEDGER  (content_hash == frequency → aux.spec)
  → deterministic verb runs                          (allowlisted, harmless)
  → an `awakened` receipt lands back in logline_acts (canonical, conformance-valid)
```

The receiver reads *how to wake* from the entity's own registration, never from
the incoming act. The act only triggers; the registration governs.

## Configuration

- **Frequencies this box owns:** `LAB_FREQUENCIES` (comma-sep) or `~/.lab/frequencies` (one per line).
- **Verb allowlist:** `LAB_WAKE_ALLOW` (comma-sep `lab` subcommands). Default `whoami,ping,heartbeat`.
- **Creds:** `RADAR_SUPABASE_*` env, else `~/.radar/sync.env`. **`LAB_BIN`** → the canonical `lab`.

## The only wake it performs

A wake-spec must be exactly:

```json
{ "wake": { "mode": "deterministic", "verb": ["lab", "<allowlisted-sub>", "..."] } }
```

`verb[0]` must be `lab` and `verb[1]` must be in the allowlist. Anything else is refused.

## Every tap leaves an honest receipt

`did = awakened`, `this = <source act content_hash>`, and a `status`:

| status | meaning |
|---|---|
| `closed` | the verb ran; `aux.result` holds its output |
| `refused` | the receiver would not act; `aux.reason` ∈ {`no-wake-spec`, `mode-out-of-scope`, `malformed-verb`, `verb-not-allowed`} |
| `failed` | the verb was allowed and ran, but errored; `aux.error` holds why |

A refusal is a **recorded non-action**, never silence. Refusals still mark the
source handled (idempotency), so a bad act is processed once and left alone.

## Guarantees

- **Idempotent.** Handled-ness is derived from `awakened` receipts in the
  append-only ledger. An in-process in-flight guard stops realtime/catch-up races.
  Delivery is at-least-once; the recorded effect is once.
- **Restart-safe.** On (re)subscribe it runs a catch-up pull (`if_ok ilike` per
  frequency) for taps missed while down; idempotency makes re-processing a no-op.
- **No outbound effects.** The allowlist is the structural boundary — the receiver
  can only run the harmless reads it was told to allow. A wake-spec naming any
  other verb is refused, not run.

## Explicitly NOT in this release

inference · the membrane · the goblin · notifications · launchd persistence ·
any mutating/outbound verb · "the fleet is alive". Those come later, on this rail.

## Run it

```bash
echo "<frequency>" > ~/.lab/frequencies
LAB_BIN=/path/to/canonical/lab node receiver/listen.mjs    # or: lab listen
```
