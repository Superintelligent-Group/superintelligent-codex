import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { compactPacketWithReport, createPacket } from '../src/context-packet.mjs'
import { createSessionManifest } from '../src/session-manifest.mjs'
import { createRestartPacket, loadRestartPacket, saveRestartPacket, serializeRestartPacket } from '../src/restart-packet.mjs'

const manifest = createSessionManifest({ issue_url: 'https://linear.app/superintelligent-group/issue/SUP-1101/session', repository_url: 'https://github.com/Superintelligent-Group/superintelligent-codex-harness', branch: 'dev', worktree_path: 'V:/worktrees/sup-1101', session_id: 'session-1101' })

function input() {
  const source = createPacket({ objective: 'dogfood pods', invariants: ['settlement is truthful'], blockers: ['terminal receipt missing'], next_actions: ['reconnect worker'], decisions: ['keep bounded'], evidence_refs: ['old', 'new'] })
  const compacted = compactPacketWithReport(source, { maxEvidence: 1 })
  return { manifest, context: compacted.packet, compaction_report: compacted.report }
}

test('restart packet deterministically replays manifest, context, and loss report', () => {
  const a = createRestartPacket(input())
  const b = createRestartPacket(input())
  assert.deepEqual(a, b)
  assert.equal(a.compaction_report.dropped_evidence, 1)
  assert.equal(a.compaction_report.output_hash, a.context.content_hash)
  assert.equal(serializeRestartPacket(a), serializeRestartPacket(b))
})

test('restart packet persists and rejects tampering', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'sig-restart-'))
  const path = join(dir, 'restart.json')
  try {
    const saved = await saveRestartPacket(path, input())
    assert.deepEqual(await loadRestartPacket(path), saved)
    const text = await readFile(path, 'utf8')
    await (await import('node:fs/promises')).writeFile(path, text.replace('dogfood pods', 'tampered objective'), 'utf8')
    await assert.rejects(loadRestartPacket(path), /(mismatch|does not describe)/)
  } finally { await rm(dir, { recursive: true, force: true }) }
})

test('restart packet fails closed when report and context diverge', () => {
  const data = input()
  assert.throws(() => createRestartPacket({ ...data, compaction_report: { ...data.compaction_report, output_hash: '0'.repeat(64) } }), /does not describe context packet/)
})
