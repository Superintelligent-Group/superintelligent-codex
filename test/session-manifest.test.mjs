import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createSessionManifest, loadSessionManifest, saveSessionManifest, serializeSessionManifest } from '../src/session-manifest.mjs'

const input = { issue_url: 'https://linear.app/superintelligent-group/issue/SUP-1101/session', repository_url: 'https://github.com/Superintelligent-Group/superintelligent-codex-harness', branch: 'dev', worktree_path: 'V:/worktrees/sup-1101', session_id: 'session-1101' }

test('manifest is deterministic and pins the governed identity', () => {
  const manifest = createSessionManifest(input)
  assert.equal(manifest.schema, 'sig.session-manifest.v1')
  assert.match(manifest.content_hash, /^[0-9a-f]{64}$/)
  assert.deepEqual(createSessionManifest({ ...input }), manifest)
  assert.equal(serializeSessionManifest(manifest), serializeSessionManifest({ ...input }))
})

test('manifest saves and reloads with hash verification', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'sig-session-'))
  const path = join(dir, 'manifest.json')
  try {
    const saved = await saveSessionManifest(path, input)
    const loaded = await loadSessionManifest(path)
    assert.deepEqual(loaded, saved)
    assert.equal((await readFile(path, 'utf8')).slice(-1), String.fromCharCode(10))
  } finally { await rm(dir, { recursive: true, force: true }) }
})

test('missing or tampered identity fails closed', async () => {
  assert.throws(() => createSessionManifest({ ...input, issue_url: 'https://example.com/SUP-1101' }), /linear.app/)
  const dir = await mkdtemp(join(tmpdir(), 'sig-session-'))
  const path = join(dir, 'manifest.json')
  try {
    await saveSessionManifest(path, input)
    const text = await readFile(path, 'utf8')
    await (await import('node:fs/promises')).writeFile(path, text.replace('session-1101', 'other-session'), 'utf8')
    await assert.rejects(loadSessionManifest(path), /content_hash mismatch/)
  } finally { await rm(dir, { recursive: true, force: true }) }
})

