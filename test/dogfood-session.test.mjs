import test from 'node:test'
import assert from 'node:assert/strict'
import { compactPacketWithReport, createPacket } from '../src/context-packet.mjs'
import { createSessionManifest } from '../src/session-manifest.mjs'
import { createRestartPacket, loadRestartPacket, saveRestartPacket } from '../src/restart-packet.mjs'
import { appendLedgerEvent, createLedger } from '../src/session-ledger.mjs'
import { mkdtemp, rm } from 'node:fs/promises'
import { join } from 'node:path'
import { tmpdir } from 'node:os'

test('governed session dogfood replays identity, context, ledger, and loss report', async () => {
  const manifest = createSessionManifest({
    issue_url: 'https://linear.app/superintelligent-group/issue/SUP-1101/codex-context-harness-governed-compaction-and-durable-session',
    repository_url: 'https://github.com/Superintelligent-Group/superintelligent-codex-harness',
    branch: 'dev',
    worktree_path: 'V:/worktrees/sup-1101-dogfood',
    session_id: 'dogfood-session-1101',
  })
  const source = createPacket({ objective: 'dogfood governed pods', invariants: ['authority is explicit'], blockers: ['cloud admission'], next_actions: ['reconnect and replay'], evidence_refs: ['local:test:16', 'linear:SUP-1101', 'run:pending'] })
  const compacted = compactPacketWithReport(source, { maxEvidence: 2 })
  let ledger = createLedger()
  ledger = appendLedgerEvent(ledger, { kind: 'decision', summary: 'keep cloud gate fail-closed', evidence_refs: ['linear:SUP-1101'] })
  ledger = appendLedgerEvent(ledger, { kind: 'evidence', summary: 'local restart proof passed', evidence_refs: [`packet:${compacted.packet.content_hash}`] })
  const restart = { manifest, context: compacted.packet, compaction_report: compacted.report }
  const dir = await mkdtemp(join(tmpdir(), 'sig-dogfood-'))
  try {
    const saved = await saveRestartPacket(join(dir, 'restart.json'), restart)
    const replayed = await loadRestartPacket(join(dir, 'restart.json'))
    assert.deepEqual(replayed, saved)
    assert.equal(replayed.manifest.issue_url, manifest.issue_url)
    assert.equal(replayed.compaction_report.dropped_evidence, 1)
    assert.equal(ledger.events.length, 2)
    assert.equal(ledger.events[1].previous_hash, ledger.events[0].event_hash)
  } finally { await rm(dir, { recursive: true, force: true }) }
})
