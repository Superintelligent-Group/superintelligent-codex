import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { createPacket, packetHash } from './context-packet.mjs'
import { createSessionManifest } from './session-manifest.mjs'

const SCHEMA = 'sig.restart-packet.v1'

function canonical(value) {
  return JSON.stringify(value, Object.keys(value).sort())
}

function hash(value) {
  return createHash('sha256').update(canonical(value), 'utf8').digest('hex')
}

function report(value) {
  if (!value || typeof value !== 'object') throw new TypeError('compaction_report must be an object')
  for (const field of ['source_hash', 'output_hash']) {
    if (typeof value[field] !== 'string' || value[field].trim() === '') throw new TypeError(`compaction_report.${field} is required`)
  }
  for (const field of ['source_bytes', 'output_bytes', 'dropped_evidence', 'dropped_decisions']) {
    if (!Number.isInteger(value[field]) || value[field] < 0) throw new TypeError(`compaction_report.${field} must be a non-negative integer`)
  }
  return {
    source_hash: value.source_hash,
    source_bytes: value.source_bytes,
    output_hash: value.output_hash,
    output_bytes: value.output_bytes,
    dropped_evidence: value.dropped_evidence,
    dropped_decisions: value.dropped_decisions,
    max_bytes: value.max_bytes ?? null,
  }
}

export function createRestartPacket({ manifest, context, compaction_report }) {
  const normalizedManifest = createSessionManifest(manifest)
  const normalizedContext = createPacket(context)
  const normalizedReport = report(compaction_report)
  if (normalizedReport.output_hash !== normalizedContext.content_hash) throw new Error('compaction report does not describe context packet')
  const body = {
    schema: SCHEMA,
    manifest: normalizedManifest,
    context: normalizedContext,
    compaction_report: normalizedReport,
  }
  return { ...body, content_hash: hash(body) }
}

export function serializeRestartPacket(packet) {
  return `${JSON.stringify(createRestartPacket(packet))}\n`
}

export async function saveRestartPacket(path, packet) {
  await writeFile(path, serializeRestartPacket(packet), 'utf8')
  return createRestartPacket(packet)
}

export async function loadRestartPacket(path) {
  let parsed
  try { parsed = JSON.parse(await readFile(path, 'utf8')) } catch (error) { throw new Error(`unable to load restart packet: ${error.message}`) }
  const { content_hash: suppliedHash, ...body } = parsed ?? {}
  const replayed = createRestartPacket(body)
  if (suppliedHash !== replayed.content_hash) throw new Error('restart packet content_hash mismatch')
  if (replayed.manifest.content_hash !== body.manifest?.content_hash) throw new Error('restart packet manifest lineage mismatch')
  const { content_hash: contextHash, ...contextBody } = body.context ?? {}
  if (packetHash(contextBody) !== contextHash) throw new Error('restart packet context lineage mismatch')
  return replayed
}



