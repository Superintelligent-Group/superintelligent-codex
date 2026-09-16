import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'

const SCHEMA = 'sig.session-ledger.v1'

function text(value, field) {
  if (typeof value !== 'string' || value.trim() === '') throw new TypeError(`${field} must be a non-empty string`)
  return value.trim()
}

function canonical(value) { return JSON.stringify(value, Object.keys(value).sort()) }
function hash(value) { return createHash('sha256').update(canonical(value), 'utf8').digest('hex') }

function normalizeEvent(event, previousHash) {
  const body = {
    sequence: event.sequence,
    kind: text(event.kind, 'kind'),
    summary: text(event.summary, 'summary'),
    evidence_refs: Array.isArray(event.evidence_refs) ? [...new Set(event.evidence_refs.map((ref) => text(ref, 'evidence_ref')))].sort() : [],
    previous_hash: previousHash,
  }
  if (!Number.isInteger(body.sequence) || body.sequence < 0) throw new TypeError('sequence must be a non-negative integer')
  return { ...body, event_hash: hash(body) }
}

export function createLedger(events = []) {
  if (!Array.isArray(events)) throw new TypeError('events must be an array')
  let previousHash = null
  const normalized = events.map((event, index) => {
    const next = normalizeEvent({ ...event, sequence: index }, previousHash)
    previousHash = next.event_hash
    return next
  })
  const body = { schema: SCHEMA, events: normalized }
  return { ...body, content_hash: hash(body) }
}

export function appendLedgerEvent(ledger, event) {
  const current = createLedger(ledger?.events ?? [])
  return createLedger([...current.events, { ...event, sequence: current.events.length }])
}

export function verifyLedger(ledger) {
  const replayed = createLedger(ledger?.events ?? [])
  if (ledger?.content_hash !== replayed.content_hash) throw new Error('session ledger content_hash mismatch')
  if (JSON.stringify(ledger.events) !== JSON.stringify(replayed.events)) throw new Error('session ledger event chain mismatch')
  return replayed
}

export function serializeLedger(ledger) { return `${JSON.stringify(verifyLedger(ledger))}\n` }
export async function saveLedger(path, ledger) { await writeFile(path, serializeLedger(ledger), 'utf8'); return verifyLedger(ledger) }
export async function loadLedger(path) {
  let parsed
  try { parsed = JSON.parse(await readFile(path, 'utf8')) } catch (error) { throw new Error(`unable to load session ledger: ${error.message}`) }
  return verifyLedger(parsed)
}
