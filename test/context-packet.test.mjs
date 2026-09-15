import test from 'node:test'
import assert from 'node:assert/strict'
import { compactPacket, createPacket } from '../src/context-packet.mjs'

test('packet is deterministic and hash-addressed', () => {
  const a = createPacket({
    objective: 'ship governed work sessions',
    invariants: ['preserve authority', 'preserve authority'],
    blockers: ['missing provider receipt'],
    next_actions: ['add observer'],
  })
  const b = createPacket({
    objective: 'ship governed work sessions',
    invariants: ['preserve authority'],
    blockers: ['missing provider receipt'],
    next_actions: ['add observer'],
  })
  assert.deepEqual(a, b)
  assert.match(a.content_hash, /^[0-9a-f]{64}$/)
})

test('compaction retains invariants, blockers, and next actions', () => {
  const packet = createPacket({
    objective: 'dogfood pods',
    invariants: ['settlement is truthful'],
    blockers: ['terminal receipt missing'],
    next_actions: ['implement provider observer'],
    evidence_refs: Array.from({ length: 20 }, (_, i) => `evidence-${i}`),
  })
  const compacted = compactPacket(packet, { maxEvidence: 3 })
  assert.deepEqual(compacted.invariants, packet.invariants)
  assert.deepEqual(compacted.blockers, packet.blockers)
  assert.deepEqual(compacted.next_actions, packet.next_actions)
  assert.deepEqual(compacted.evidence_refs, ['evidence-7', 'evidence-8', 'evidence-9'])
  assert.equal(compacted.parent_hash, packet.content_hash)
})

test('malformed packets fail closed', () => {
  assert.throws(() => createPacket({ objective: 'x', invariants: [], blockers: [] }), /missing next_actions/)
})
