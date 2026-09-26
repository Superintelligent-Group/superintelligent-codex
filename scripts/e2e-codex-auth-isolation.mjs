// E2E: an app-server API-key login must not overwrite a persisted ChatGPT login.
//
// Runs a real `codex app-server` over stdio against a throwaway CODEX_HOME
// seeded with a synthetic (unsigned, never-sent) ChatGPT auth.json, performs
// account/login/start {type: apiKey}, and checks that auth.json is byte-for-byte
// unchanged while the session itself reports an API-key account.
//
// usage: node scripts/e2e-codex-auth-isolation.mjs [path\to\codex.exe]
// exit 0 = isolated (patched), 1 = clobbered (upstream bug), 2 = harness error
import { spawn } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createInterface } from 'node:readline'

const codex = process.argv[2] ?? 'codex'
const b64 = (value) => Buffer.from(JSON.stringify(value)).toString('base64url')
const idToken = [
  b64({ alg: 'none', typ: 'JWT' }),
  b64({ email: 'e2e@example.com', 'https://api.openai.com/auth': { chatgpt_plan_type: 'pro', chatgpt_account_id: 'acct-e2e' } }),
  Buffer.from('signature').toString('base64url'),
].join('.')

const home = mkdtempSync(join(tmpdir(), 'sig-codex-e2e-'))
const authPath = join(home, 'auth.json')
writeFileSync(join(home, 'config.toml'), '')
writeFileSync(authPath, JSON.stringify({
  auth_mode: 'chatgpt',
  OPENAI_API_KEY: null,
  tokens: { id_token: idToken, access_token: 'e2e-access', refresh_token: 'e2e-refresh', account_id: 'acct-e2e' },
  last_refresh: new Date().toISOString(),
}, null, 2))
const before = readFileSync(authPath, 'utf8')

const env = { ...process.env, CODEX_HOME: home }
for (const key of ['OPENAI_API_KEY', 'CODEX_API_KEY', 'CODEX_ACCESS_TOKEN']) delete env[key]
const child = spawn(codex, ['app-server'], { env, stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true })

const pending = new Map()
createInterface({ input: child.stdout }).on('line', (line) => {
  let message
  try { message = JSON.parse(line) } catch { return }
  if (message.id !== undefined && pending.has(message.id)) {
    const { resolve, reject } = pending.get(message.id)
    pending.delete(message.id)
    message.error ? reject(new Error(`${message.error.code}: ${message.error.message}`)) : resolve(message.result)
  }
})
let nextId = 1
const request = (method, params) => new Promise((resolve, reject) => {
  const id = nextId++
  pending.set(id, { resolve, reject })
  child.stdin.write(JSON.stringify({ id, method, params }) + '\n')
})
const notify = (method, params) => child.stdin.write(JSON.stringify({ method, params }) + '\n')

let code = 2
const timer = setTimeout(() => { console.error('timeout'); child.kill(); process.exit(2) }, 60_000)
try {
  await request('initialize', { clientInfo: { name: 'sig-e2e', title: null, version: '0.0.0' } })
  notify('initialized', {})
  const login = await request('account/login/start', { type: 'apiKey', apiKey: 'sk-e2e-dummy-not-a-real-key' })
  const read = await request('account/read', { refreshToken: false })
  const after = readFileSync(authPath, 'utf8')
  const session = read.account?.type
  const persisted = JSON.parse(after).auth_mode
  console.log(JSON.stringify({ codex, login, sessionAccount: session, persistedAuthMode: persisted, authJsonUnchanged: after === before }))
  code = after === before && session === 'apiKey' ? 0 : 1
} catch (error) {
  console.error(error.message)
} finally {
  clearTimeout(timer)
  const exited = new Promise((resolve) => child.once('exit', resolve))
  child.kill()
  await exited
  rmSync(home, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 })
}
process.exit(code)
