import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { appendLedgerEvent, createLedger, loadLedger, saveLedger, verifyLedger } from '../src/session-ledger.mjs'

const first = { kind: 'decision', summary: 'preserve owner authority', evidence_refs: ['linear:SUP-1101', 'linear:SUP-1101'] }
const second = { kind: 'evidence', summary: 'local tests passed', evidence_refs: ['test:13'] }

test('ledger is deterministic and hash chained', () => {
  const a = appendLedgerEvent(appendLedgerEvent(createLedger(), first), second)
  const b = appendLedgerEvent(appendLedgerEvent(createLedger(), first), second)
  assert.deepEqual(a, b)
  assert.equal(a.events[0].previous_hash, null)
  assert.equal(a.events[1].previous_hash, a.events[0].event_hash)
  assert.match(a.content_hash, /^[0-9a-f]{64}$/)
  assert.deepEqual(verifyLedger(a), a)
})

test('ledger persists and replays append-only history', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'sig-ledger-'))
  const path = join(dir, 'ledger.json')
  try {
    const ledger = appendLedgerEvent(createLedger(), first)
    const saved = await saveLedger(path, ledger)
    assert.deepEqual(await loadLedger(path), saved)
    assert.equal((await readFile(path, 'utf8')).slice(-1), String.fromCharCode(10))
  } finally { await rm(dir, { recursive: true, force: true }) }
})

test('ledger rejects reordered or tampered events', async () => {
  const ledger = appendLedgerEvent(appendLedgerEvent(createLedger(), first), second)
  assert.throws(() => verifyLedger({ ...ledger, events: [...ledger.events].reverse() }), /mismatch/)
  const dir = await mkdtemp(join(tmpdir(), 'sig-ledger-'))
  const path = join(dir, 'ledger.json')
  try {
    await writeFile(path, JSON.stringify({ ...ledger, events: [{ ...ledger.events[0], summary: 'tampered' }, ledger.events[1]] }), 'utf8')
    await assert.rejects(loadLedger(path), /mismatch/)
  } finally { await rm(dir, { recursive: true, force: true }) }
})
