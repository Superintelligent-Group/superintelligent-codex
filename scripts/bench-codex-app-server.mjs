// Benchmark: app-server startup and first-turn latency, stock vs patched Codex.
//
// Each run spawns a fresh `codex app-server` with the user's real config and
// login, then times initialize, model/list, thread/start, and (with --turns)
// time-to-first-token and completion of a trivial turn. Runs alternate between
// binaries so drift (network, machine load) hits both equally.
//
// usage: node scripts/bench-codex-app-server.mjs --a <stock.exe> --b <patched.exe> [--runs 6] [--turns 3]
// Turns make real model calls on the user's login; --turns 0 measures startup only.
import { spawn, spawnSync } from 'node:child_process'
import { createInterface } from 'node:readline'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'

const args = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, all) => {
  if (value.startsWith('--')) pairs.push([value.slice(2), all[index + 1]])
  return pairs
}, []))
const binaries = { a: args.a, b: args.b }
if (!binaries.a || !binaries.b) {
  console.error('usage: --a <stock codex.exe> --b <patched codex.exe> [--runs N] [--turns N]')
  process.exit(2)
}
const runs = Number(args.runs ?? 6)
const stepTimeoutMs = Number(args['step-timeout'] ?? 120000)
const debug = args.debug !== undefined
// Fail loudly with the step name instead of hanging on an event that never arrives.
const step = (label, promise) => Promise.race([
  promise,
  new Promise((_, reject) => setTimeout(() => reject(new Error(`timeout: ${label}`)), stepTimeoutMs)),
])
const turns = Number(args.turns ?? 3)
const turnsPerRun = Number(args['turns-per-run'] ?? 1)
// --stale-cache: backdate the model cache before each run (the cache stays valid, it only looks
// old), reproducing a launch more than 5 minutes after the last refresh.
const staleCache = args['stale-cache'] !== undefined
const modelsCache = join(process.env.CODEX_HOME ?? join(homedir(), '.codex'), 'models_cache.json')
function backdateModelsCache() {
  const text = readFileSync(modelsCache, 'utf8')
  const stale = new Date(Date.now() - 24 * 3600 * 1000).toISOString()
  writeFileSync(modelsCache, text.replace(/"fetched_at":\s*"[^"]*"/, `"fetched_at": "${stale}"`))
}

async function measure(codex, withTurn) {
  const cwd = mkdtempSync(join(tmpdir(), 'sig-codex-bench-'))
  const child = spawn(codex, ['app-server'], { cwd, stdio: ['pipe', 'pipe', 'ignore'], windowsHide: true })
  const pending = new Map()
  const listeners = []
  createInterface({ input: child.stdout }).on('line', (line) => {
    let message
    try { message = JSON.parse(line) } catch { return }
    if (message.id !== undefined && pending.has(message.id)) {
      const { resolve, reject } = pending.get(message.id)
      pending.delete(message.id)
      message.error ? reject(new Error(message.error.message)) : resolve(message.result)
    } else if (message.method) {
      if (debug) console.error(`  <- ${message.method}${message.id !== undefined ? ' (server request)' : ''}${message.method === 'warning' ? ' ' + JSON.stringify(message.params) : ''}`)
      for (const listener of listeners) listener(message)
    }
  })
  let nextId = 1
  const request = (method, params) => new Promise((resolve, reject) => {
    const id = nextId++
    pending.set(id, { resolve, reject })
    child.stdin.write(JSON.stringify({ id, method, params }) + '\n')
  })
  const waitFor = (predicate) => new Promise((resolve) => listeners.push((m) => predicate(m) && resolve(m)))

  if (staleCache) backdateModelsCache()
  const t0 = performance.now()
  const result = {}
  try {
    await step('initialize', request('initialize', { clientInfo: { name: 'sig-bench', title: null, version: '0.0.0' } }))
    child.stdin.write(JSON.stringify({ method: 'initialized', params: {} }) + '\n')
    result.initialize = performance.now() - t0
    let t = performance.now()
    await step('model/list', request('model/list', {}))
    result.modelList = performance.now() - t
    t = performance.now()
    const started = await step('thread/start', request('thread/start', { cwd, ephemeral: true }))
    result.threadStart = performance.now() - t
    if (withTurn) {
      const followups = []
      for (let n = 0; n < turnsPerRun; n++) {
        t = performance.now()
        const firstDelta = waitFor((m) => m.method.endsWith('/delta'))
        const completed = waitFor((m) => m.method === 'turn/completed')
        await step('turn/start', request('turn/start', { threadId: started.thread.id, input: [{ type: 'text', text: 'Reply with exactly: OK', text_elements: [] }] }))
        await step('first delta', firstDelta)
        const firstToken = performance.now() - t
        await step('turn/completed', completed)
        const turn = performance.now() - t
        if (n === 0) Object.assign(result, { firstToken, turn })
        else followups.push(turn)
      }
      if (followups.length) result.followupTurn = median(followups)
    }
    result.total = performance.now() - t0
  } catch (error) {
    result.error = error.message
  } finally {
    // Kill the whole tree: MCP servers outlive a plain kill and pin the temp cwd.
    const exited = new Promise((resolve) => child.once('exit', resolve))
    if (process.platform === 'win32') spawnSync('taskkill', ['/T', '/F', '/PID', String(child.pid)], { stdio: 'ignore' })
    else child.kill()
    await exited
    try { rmSync(cwd, { recursive: true, force: true, maxRetries: 20, retryDelay: 250 }) } catch {}
  }
  return result
}

const median = (values) => {
  const sorted = [...values].sort((x, y) => x - y)
  const mid = Math.floor(sorted.length / 2)
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2
}

const samples = { a: [], b: [] }
for (let i = 0; i < runs; i++) {
  for (const key of i % 2 ? ['b', 'a'] : ['a', 'b']) {
    const sample = await measure(binaries[key], i < turns)
    samples[key].push(sample)
    console.error(`${key} run ${i + 1}: ${JSON.stringify(Object.fromEntries(Object.entries(sample).map(([k, v]) => [k, typeof v === 'number' ? Math.round(v) : v])))}`)
  }
}

const metrics = ['initialize', 'modelList', 'threadStart', 'firstToken', 'turn', 'followupTurn', 'total']
const report = {}
for (const metric of metrics) {
  const a = samples.a.map((s) => s[metric]).filter((v) => typeof v === 'number')
  const b = samples.b.map((s) => s[metric]).filter((v) => typeof v === 'number')
  if (!a.length || !b.length) continue
  report[metric] = { stock_ms: Math.round(median(a)), patched_ms: Math.round(median(b)), n: `${a.length}/${b.length}` }
}
report.errors = [...samples.a, ...samples.b].filter((s) => s.error).map((s) => s.error)
console.log(JSON.stringify(report, null, 2))
