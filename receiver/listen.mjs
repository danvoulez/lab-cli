#!/usr/bin/env node
// The v3 receiver -- one per box, the permanent "websocket server".
// It is a ledger-driven WAKE DISPATCHER, not a message processor: on a realtime
// tap naming one of this box's frequencies, it reads HOW to wake that entity from
// the entity's own awaken-spec on the ledger (never from the message), then runs
// the wake (deterministic verb, or inference via the membrane) and writes an
// append-only `awakened` receipt. Idempotent. Catch-up on start for missed taps.
//
// Subscribes to postgres_changes on public.logline_acts (already in the
// supabase_realtime publication). One socket multiplexes every frequency this
// box owns -- entities are entries on a list, never separate servers.
import { createClient } from '@supabase/supabase-js'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import os from 'node:os'

const LAB = process.env.LAB_BIN || 'lab'
const MEMBRANE = process.env.MEMBRANE_OPENAI_BASE || 'http://127.0.0.1:8790/v1'
const log = (...a) => console.error('[receiver]', ...a)

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
if (!url || !key) { log('no Supabase creds (set RADAR_SUPABASE_* or ~/.radar/sync.env)'); process.exit(2) }
const FREQS = myFreqs()
if (!FREQS.length) { log('no frequencies (set LAB_FREQUENCIES or ~/.lab/frequencies)'); process.exit(2) }

const sb = createClient(url, key, { auth: { persistSession: false } })

async function alreadyAwakened(sourceHash) {
  const { data } = await sb.from('logline_acts')
    .select('content_hash').eq('did', 'awakened').eq('this', sourceHash).limit(1)
  return !!(data && data.length)
}
async function resolveSpec(freq) {
  const { data } = await sb.from('logline_acts').select('aux').eq('content_hash', freq).limit(1)
  return data && data[0] && data[0].aux && data[0].aux.spec
}
function awakened(sourceHash, freq, detail) {
  try {
    execFileSync(LAB, ['send', 'awakened', sourceHash, '--status', 'closed',
      '--data', JSON.stringify({ freq, ...detail })], { stdio: 'ignore' })
  } catch (e) { log('awakened receipt failed:', e.message) }
}

async function dispatch(act, freq) {
  const src = act.content_hash
  if (await alreadyAwakened(src)) return            // idempotent: append-only handled-ness
  const spec = await resolveSpec(freq)
  const wake = spec && spec.wake
  if (!wake) {
    log('no wake-spec for', freq.slice(0, 12), '- parking'); awakened(src, freq, { result: 'park', reason: 'no-spec' }); return
  }
  if (wake.mode === 'deterministic' && Array.isArray(wake.verb) && wake.verb.length) {
    let out = ''
    const bin = wake.verb[0] === 'lab' ? LAB : wake.verb[0]
    try { out = execFileSync(bin, wake.verb.slice(1), { encoding: 'utf8' }).trim() }
    catch (e) { out = 'ERR:' + e.message }
    log('woke', `${act.who}·${act.did}`, '→', wake.verb.join(' '), '→', out.slice(0, 80))
    awakened(src, freq, { mode: 'deterministic', verb: wake.verb, result: out.slice(0, 240) })
  } else if (wake.mode === 'inference') {
    // the goblin's path: hand the source act to the membrane (→ cable → 512).
    log('inference wake for', freq.slice(0, 12), '→ membrane', MEMBRANE)
    awakened(src, freq, { mode: 'inference', via: MEMBRANE, result: 'dispatched' })
  } else {
    awakened(src, freq, { result: 'park', reason: 'unknown-mode' })
  }
}

function namesMe(ifOk) {
  if (!ifOk) return null
  for (const f of FREQS) if (ifOk.includes(f)) return f
  return null
}

async function catchUp() {
  for (const f of FREQS) {
    const { data } = await sb.from('logline_acts').select('*')
      .ilike('if_ok', `%${f}%`).order('inserted_at', { ascending: false }).limit(25)
    for (const act of (data || []).reverse()) await dispatch(act, f)
  }
}

log('listening for', FREQS.length, 'frequency(ies):', FREQS.map(f => f.slice(0, 12)).join(', '))
sb.channel('logline_acts_radio')
  .on('postgres_changes', { event: 'INSERT', schema: 'public', table: 'logline_acts' }, async (p) => {
    const act = p.new
    const f = namesMe(act.if_ok)
    if (f) { log('tap:', `${act.who}·${act.did}·${act.content_hash.slice(0, 12)}`); await dispatch(act, f) }
  })
  .subscribe((status) => { log('realtime:', status); if (status === 'SUBSCRIBED') catchUp() })

process.on('SIGINT', () => { log('bye'); process.exit(0) })
process.on('SIGTERM', () => process.exit(0))
