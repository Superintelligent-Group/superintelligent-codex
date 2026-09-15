import { createHash } from 'node:crypto'

const REQUIRED = ['objective', 'invariants', 'blockers', 'next_actions']

function assertString(value, field) {
  if (typeof value !== 'string' || value.trim() === '') throw new TypeError(`${field} must be a non-empty string`)
}

function list(value, field) {
  if (!Array.isArray(value) || value.some((item) => typeof item !== 'string' || item.trim() === '')) {
    throw new TypeError(`${field} must be an array of non-empty strings`)
  }
  return [...new Set(value.map((item) => item.trim()))].sort()
}

function canonical(value) {
  return JSON.stringify(value, Object.keys(value).sort())
}

export function packetHash(packet) {
  return createHash('sha256').update(canonical(packet)).digest('hex')
}

export function packetByteLength(packet) {
  return Buffer.byteLength(JSON.stringify(packet), 'utf8')
}

export function createPacket(input = {}) {
  for (const field of REQUIRED) {
    if (!(field in input)) throw new TypeError(`missing ${field}`)
  }
  assertString(input.objective, 'objective')
  const packet = {
    schema: 'sig.context-packet.v1',
    objective: input.objective.trim(),
    invariants: list(input.invariants, 'invariants'),
    blockers: list(input.blockers, 'blockers'),
    next_actions: list(input.next_actions, 'next_actions'),
    decisions: list(input.decisions ?? [], 'decisions'),
    evidence_refs: list(input.evidence_refs ?? [], 'evidence_refs'),
    budget_ref: input.budget_ref ?? null,
    parent_hash: input.parent_hash ?? null,
  }
  if (packet.budget_ref !== null) assertString(packet.budget_ref, 'budget_ref')
  if (packet.parent_hash !== null) assertString(packet.parent_hash, 'parent_hash')
  return { ...packet, content_hash: packetHash(packet) }
}

export function compactPacketWithReport(packet, { maxEvidence = 12, maxBytes = null } = {}) {
  if (!Number.isInteger(maxEvidence) || maxEvidence < 0) throw new TypeError('maxEvidence must be a non-negative integer')
  const source = createPacket(packet)
  const sourceBytes = packetByteLength(source)
  const compacted = {
    ...source,
    evidence_refs: source.evidence_refs.slice(-maxEvidence),
    parent_hash: source.content_hash,
  }
  delete compacted.content_hash

  if (maxBytes !== null) {
    if (!Number.isInteger(maxBytes) || maxBytes < 1) throw new TypeError('maxBytes must be a positive integer')
    // Evidence and decisions are the only lossy fields. Drop oldest evidence
    // first, then oldest decisions, while the authoritative fields stay intact.
    while (packetByteLength({ ...compacted, content_hash: packetHash(compacted) }) > maxBytes) {
      if (compacted.evidence_refs.length > 0) compacted.evidence_refs.shift()
      else if (compacted.decisions.length > 0) compacted.decisions.shift()
      else throw new RangeError('maxBytes is smaller than the mandatory context envelope')
    }
  }

  const result = { ...compacted, content_hash: packetHash(compacted) }
  return {
    packet: result,
    report: {
      source_hash: source.content_hash,
      source_bytes: sourceBytes,
      output_hash: result.content_hash,
      output_bytes: packetByteLength(result),
      dropped_evidence: source.evidence_refs.length - result.evidence_refs.length,
      dropped_decisions: source.decisions.length - result.decisions.length,
      max_bytes: maxBytes,
    },
  }
}

export function compactPacket(packet, options = {}) {
  return compactPacketWithReport(packet, options).packet
}
