#!/usr/bin/env node
// =============================================================================
// v3 receiver -- manual deterministic wake, single box, canonical receipts.
// =============================================================================
// One per box. A ledger-driven WAKE DISPATCHER, not a message processor: on a
// realtime tap naming one of this box's frequencies, it reads HOW to wake that
// entity from the entity's own awaken-spec ON THE LEDGER (never from the
// message), runs the wake, and writes an append-only `awakened` receipt.
//
// SCOPE (deliberately small, so the same tap can be trusted):
//   * deterministic wake ONLY -- a verb from a tiny allowlist of harmless reads.
//   * every tap gets an honest receipt: closed (ran) / refused (won't) /
//     failed (tried, errored). A refusal is a recorded non-action, never silence.
//   * idempotent + restart-safe: handled-ness is derived from `awakened` receipts
//     in the append-only ledger; an in-flight guard stops same-process races.
//
// OUT OF SCOPE (on purpose): inference, the membrane, the goblin, notifications,
// any mutating/outbound verb. Those are refused, not attempted.
//
// Subscribes to postgres_changes on public.logline_acts (already in the
// supabase_realtime publication). One socket multiplexes every frequency this
// box owns -- entities are entries on a list, never separate servers.
import { createClient } from '@supabase/supabase-js'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import os from 'node:os'

const LAB = process.env.LAB_BIN || 'lab'
const log = (...a) => console.error('[receiver]', ...a)

// Only these `lab <sub>` verbs may run on a deterministic wake. Harmless reads.
// Widening this list is a deliberate, reviewed act -- never the message's choice.
const ALLOW = new Set(
  (process.env.LAB_WAKE_ALLOW || 'whoami,ping,heartbeat').split(',').map(s => s.trim()).filter(Boolean)
)

function loadCreds() {
  let url = process.env.RADAR_SUPABASE_URL || process.env.SUPABASE_URL || process.env.LAB_SUPABASE_URL
  let key = process.env.RADAR_SUPABASE_KEY || process.env.SUPABASE_KEY || process.env.LAB_SUPABASE_KEY
  if (!url || !key) {
    try {
      for (const line of readFileSync(`${os.homedir()}/.radar/sync.env`, 'utf8').split('\n')) {
        const m = line.match(/^\s*([A-Z_]+)\s*=\s*(.*)\s*$/)
        if (!m) continue
        const [, k, v] = m
        if (!url && /SUPABASE_URL$/.test(k)) url = v
        if (!key && /SUPABASE_KEY$/.test(k)) key = v
      }
    } catch {}
  }
  return { url, key }
}

function myFreqs() {
  const env = (process.env.LAB_FREQUENCIES || '').split(',').map(s => s.trim()).filter(Boolean)
  let file = []
  try {
    file = readFileSync(`${os.homedir()}/.lab/frequencies`, 'utf8')
      .split('\n').map(s => s.trim()).filter(s => s && !s.startsWith('#'))
  } catch {}
  return [...new Set([...env, ...file])]
}

const { url, key } = loadCreds()
if (!url || !key) { log('FATAL: no Supabase creds (set RADAR_SUPABASE_* or ~/.radar/sync.env)'); process.exit(2) }
const FREQS = myFreqs()
if (!FREQS.length) { log('FATAL: no frequencies (set LAB_FREQUENCIES or ~/.lab/frequencies)'); process.exit(2) }

const sb = createClient(url, key, { auth: { persistSession: false } })
const inFlight = new Set()   // same-process race guard (realtime vs catch-up)

async function alreadyAwakened(src) {
  const { data, error } = await sb.from('logline_acts')
    .select('content_hash').eq('did', 'awakened').eq('this', src).limit(1)
  if (error) { log('idempotency check failed, treating as unhandled:', error.message); return false }
  return !!(data && data.length)
}
async function resolveSpec(freq) {
  const { data, error } = await sb.from('logline_acts').select('aux').eq('content_hash', freq).limit(1)
  if (error || !data || !data[0]) return null
  return data[0].aux && data[0].aux.spec
}
// Every tap leaves an honest receipt. status: closed | refused | failed.
function receipt(src, status, detail) {
  try {
    execFileSync(LAB, ['send', 'awakened', src, '--status', status,
      '--data', JSON.stringify(detail)], { stdio: 'ignore' })
  } catch (e) { log('receipt write failed:', e.message) }
}

async function dispatch(act, freq) {
  const src = act.content_hash
  if (!src) return
  if (inFlight.has(src)) return
  inFlight.add(src)
  try {
    if (await alreadyAwakened(src)) return            // idempotent: append-only handled-ness

    const spec = await resolveSpec(freq)
    const wake = spec && spec.wake
    if (!wake) {
      log('refuse', src.slice(0, 12), '- no wake-spec for', freq.slice(0, 12))
      return receipt(src, 'refused', { freq, reason: 'no-wake-spec' })
    }
    if (wake.mode !== 'deterministic') {
      log('refuse', src.slice(0, 12), '- mode', JSON.stringify(wake.mode), 'is out of scope')
      return receipt(src, 'refused', { freq, reason: 'mode-out-of-scope', mode: wake.mode || null })
    }
    if (!Array.isArray(wake.verb) || !wake.verb.length || wake.verb.some(x => typeof x !== 'string')) {
      log('refuse', src.slice(0, 12), '- malformed verb')
      return receipt(src, 'refused', { freq, reason: 'malformed-verb' })
    }
    if (wake.verb[0] !== 'lab' || !ALLOW.has(wake.verb[1])) {
      log('refuse', src.slice(0, 12), '- verb not allowlisted:', wake.verb.join(' '))
      return receipt(src, 'refused', { freq, reason: 'verb-not-allowed', verb: wake.verb })
    }

    let out, ok = true
    try { out = execFileSync(LAB, wake.verb.slice(1), { encoding: 'utf8', timeout: 15000 }).trim() }
    catch (e) { ok = false; out = (e.stderr || e.message || '').toString().trim() }
    if (ok) {
      log('woke', `${act.who}·${act.did}`, '→', wake.verb.join(' '), '→', out.slice(0, 80))
      receipt(src, 'closed', { freq, mode: 'deterministic', verb: wake.verb, result: out.slice(0, 240) })
    } else {
      log('failed', src.slice(0, 12), '-', wake.verb.join(' '), '→', out.slice(0, 80))
      receipt(src, 'failed', { freq, mode: 'deterministic', verb: wake.verb, error: out.slice(0, 240) })
    }
  } catch (e) {
    log('dispatch error (no receipt, will retry on restart):', e.message)
  } finally {
    inFlight.delete(src)
  }
}

function namesMe(ifOk) {
  if (!ifOk) return null
  for (const f of FREQS) if (ifOk.includes(f)) return f
  return null
}

async function catchUp() {
  for (const f of FREQS) {
    const { data, error } = await sb.from('logline_acts').select('*')
      .ilike('if_ok', `%${f}%`).order('inserted_at', { ascending: false }).limit(25)
    if (error) { log('catch-up query failed:', error.message); continue }
    for (const act of (data || []).reverse()) await dispatch(act, f)
  }
}

log('v3 receiver -- deterministic-only, allowlist:', [...ALLOW].join(','))
log('listening for', FREQS.length, 'frequency(ies):', FREQS.map(f => f.slice(0, 12)).join(', '))
sb.channel('logline_acts_radio')
  .on('postgres_changes', { event: 'INSERT', schema: 'public', table: 'logline_acts' }, async (p) => {
    try {
      const act = p.new
      const f = namesMe(act && act.if_ok)
      if (f) { log('tap:', `${act.who}·${act.did}·${String(act.content_hash).slice(0, 12)}`); await dispatch(act, f) }
    } catch (e) { log('handler error:', e.message) }
  })
  .subscribe((status) => {
    log('realtime:', status)
    if (status === 'SUBSCRIBED') catchUp()           // catch missed taps on (re)connect
  })

process.on('SIGINT', () => { log('bye'); process.exit(0) })
process.on('SIGTERM', () => process.exit(0))
