import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'

const SCHEMA = 'sig.session-manifest.v1'
const REQUIRED = ['issue_url', 'repository_url', 'branch', 'worktree_path', 'session_id']

function requiredString(value, field) {
  if (typeof value !== 'string' || value.trim() === '') throw new TypeError(`${field} must be a non-empty string`)
  return value.trim()
}

function canonical(value) {
  return JSON.stringify(value, Object.keys(value).sort())
}

function digest(value) {
  return createHash('sha256').update(canonical(value), 'utf8').digest('hex')
}

export function createSessionManifest(input = {}) {
  for (const field of REQUIRED) if (!(field in input)) throw new TypeError(`missing ${field}`)
  const issueUrl = requiredString(input.issue_url, 'issue_url')
  const repositoryUrl = requiredString(input.repository_url, 'repository_url')
  const branch = requiredString(input.branch, 'branch')
  const worktreePath = requiredString(input.worktree_path, 'worktree_path')
  const sessionId = requiredString(input.session_id, 'session_id')
  let parsedIssue
  try { parsedIssue = new URL(issueUrl) } catch { throw new TypeError('issue_url must be a valid URL') }
  if (parsedIssue.hostname !== 'linear.app') throw new TypeError('issue_url must point to linear.app')
  try { new URL(repositoryUrl) } catch { throw new TypeError('repository_url must be a valid URL') }
  const manifest = {
    schema: SCHEMA,
    issue_url: issueUrl,
    repository_url: repositoryUrl,
    branch,
    worktree_path: worktreePath,
    session_id: sessionId,
    parent_hash: input.parent_hash == null ? null : requiredString(input.parent_hash, 'parent_hash'),
  }
  return { ...manifest, content_hash: digest(manifest) }
}

export function serializeSessionManifest(manifest) {
  return `${JSON.stringify(createSessionManifest(manifest))}\n`
}

export async function saveSessionManifest(path, manifest) {
  await writeFile(path, serializeSessionManifest(manifest), 'utf8')
  return createSessionManifest(manifest)
}

export async function loadSessionManifest(path) {
  let parsed
  try { parsed = JSON.parse(await readFile(path, 'utf8')) } catch (error) { throw new Error(`unable to load session manifest: ${error.message}`) }
  const { content_hash: suppliedHash, ...body } = parsed ?? {}
  const validated = createSessionManifest(body)
  if (suppliedHash !== validated.content_hash) throw new Error('session manifest content_hash mismatch')
  return validated
}
