import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const stack = JSON.parse(await readFile(new URL('../patches/codex/stack.json', import.meta.url), 'utf8'))

test('patch stack pins an upstream release tag and commit', () => {
  assert.match(stack.base.tag, /^rust-v\d+\.\d+\.\d+$/)
  assert.match(stack.base.commit, /^[0-9a-f]{40}$/)
  assert.equal(stack.branch, `sig/${stack.base.tag.replace(/^rust-v/, '')}`)
})

test('every patch declares its kind, reason, and exit condition', () => {
  assert.ok(stack.patches.length > 0)
  for (const patch of stack.patches) {
    assert.ok(['backport', 'sig'].includes(patch.kind), patch.subject)
    assert.ok(patch.reason?.trim(), `${patch.subject}: reason`)
    assert.ok(patch.drop_when?.trim(), `${patch.subject}: drop_when`)
    if (patch.kind === 'backport') assert.match(patch.upstream, /^[0-9a-f]{7,40}$/, patch.subject)
    else assert.equal(patch.upstream, null, patch.subject)
  }
})

test('patch stack matches the fork branch, in order', async (t) => {
  const { execFileSync } = await import('node:child_process')
  const { existsSync } = await import('node:fs')
  const fork = process.env.SIG_CODEX_FORK ?? 'C:/Github/superintelligent-codex'
  if (!existsSync(fork)) return t.skip(`fork checkout not found at ${fork}`)
  let subjects
  try {
    subjects = execFileSync('git', ['-C', fork, 'log', '--reverse', '--format=%s', `${stack.base.tag}..${stack.branch}`], { encoding: 'utf8' })
      .split('\n').filter(Boolean)
  } catch {
    return t.skip(`${stack.branch} or ${stack.base.tag} not available in ${fork}`)
  }
  assert.deepEqual(subjects, stack.patches.map((patch) => patch.subject))
})
