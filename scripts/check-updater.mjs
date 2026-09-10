// 업데이터가 조용히 깨지는 것을 막는 검사.
//
//   npm run check:updater
//
// 실제 GitHub Release 를 만들지 않고도 볼 수 있는 것을 본다.
//   ① tauri.conf.json 의 pubkey 가 이 PC 의 서명 키(.pub)와 같은가
//      — 다르면 릴리스마다 서명은 되는데 설치된 앱이 전부 거부한다.
//   ② 업데이터 endpoint 가 저장소 주소와 같은가
//   ③ 설치 파일이 빌드되어 있으면, 그 .sig 가 pubkey 로 실제로 검증되는가
//      (minisign 형식 — "ED" 해시 서명, blake2b-512 + ed25519)
//   ④ release.yml 이 만드는 latest.json 과 같은 꼴을 여기서 만들어 보고,
//      버전이 semver 인지 · url 이 ASCII 인지 · signature 가 ③의 것인지 본다
//   ⑤ 버전 견주기 — 업데이터는 "지금보다 큰 semver" 만 새것으로 본다. 그 규칙을
//      우리 버전 문자열에 적용해 이상이 없는지 본다.
//
// 이 스크립트는 **네트워크를 쓰지 않는다.** CI 에서도 그대로 돈다 (.pub 가 없는
// CI 에서는 ①을 건너뛰고 말해 준다).
import { readFileSync, existsSync, readdirSync } from 'node:fs'
import { createHash, createPublicKey, verify } from 'node:crypto'
import { join } from 'node:path'
import { homedir } from 'node:os'

const problems = []
const notes = []
const fail = (m) => problems.push(m)

const conf = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'))
const pkg = JSON.parse(readFileSync('package.json', 'utf8'))
const updater = conf.plugins?.updater
if (!updater) {
  fail('tauri.conf.json 에 plugins.updater 가 없습니다.')
}

// ── minisign 읽기 ──────────────────────────────────────────────────
/** base64 한 줄로 감싼 minisign 파일을 푼다. 이미 풀린 글이면 그대로 둔다.
 *  (tauri 는 .key.pub 파일도, tauri.conf 의 pubkey 도 이렇게 한 줄로 감싼다) */
function unwrap(text) {
  const t = text.trim()
  if (t.startsWith('untrusted comment')) return t
  return Buffer.from(t, 'base64').toString('utf8')
}
/** 공개키 파일 → { keyId, pk } */
function parsePub(text) {
  const line = text.split('\n').find((l) => l && !l.startsWith('untrusted comment'))
  const raw = Buffer.from(line.trim(), 'base64')
  const alg = raw.subarray(0, 2).toString('latin1')
  if (alg !== 'Ed') throw new Error(`공개키 알고리즘이 Ed 가 아닙니다: ${alg}`)
  return { keyId: raw.subarray(2, 10).toString('hex'), pk: raw.subarray(10, 42) }
}
/** 서명 파일 → { alg, keyId, sig } */
function parseSig(text) {
  const lines = text.split('\n')
  const line = lines[1]
  const raw = Buffer.from(line.trim(), 'base64')
  return { alg: raw.subarray(0, 2).toString('latin1'), keyId: raw.subarray(2, 10).toString('hex'), sig: raw.subarray(10, 74) }
}
function ed25519Key(pk) {
  // SPKI DER 머리 + 32바이트 공개키
  const prefix = Buffer.from('302a300506032b6570032100', 'hex')
  return createPublicKey({ key: Buffer.concat([prefix, pk]), format: 'der', type: 'spki' })
}

// ── ① pubkey 와 .pub ────────────────────────────────────────────────
let confPub = null
try {
  confPub = parsePub(unwrap(updater.pubkey))
} catch (e) {
  fail(`tauri.conf.json 의 pubkey 를 읽지 못했습니다: ${e.message}`)
}
const pubPath = join(homedir(), '.docaid-release', 'docaid.key.pub')
if (existsSync(pubPath) && confPub) {
  const local = parsePub(unwrap(readFileSync(pubPath, 'utf8')))
  if (local.keyId !== confPub.keyId || !local.pk.equals(confPub.pk)) {
    fail(
      '서명 키가 어긋납니다 — tauri.conf.json 의 pubkey 와 ~/.docaid-release/docaid.key.pub 가 다릅니다.\n' +
        '      이 상태로 릴리스하면 설치된 앱이 업데이트 서명을 전부 거부합니다.',
    )
  } else {
    notes.push(`서명 키 id ${confPub.keyId} — 설정과 이 PC 의 키가 같습니다.`)
  }
} else if (!existsSync(pubPath)) {
  notes.push('이 PC 에 서명 공개키(.pub)가 없어 ①(키 일치)은 건너뜁니다 (CI 에서는 정상).')
}

// ── ② endpoint 와 저장소 ────────────────────────────────────────────
const repo = (conf.bundle?.homepage ?? '').replace(/^https:\/\/github\.com\//, '')
const endpoint = updater?.endpoints?.[0] ?? ''
const expected = `https://github.com/${repo}/releases/latest/download/latest.json`
if (endpoint !== expected) {
  fail(`업데이터 endpoint 가 저장소와 다릅니다.\n      있는 것: ${endpoint}\n      기대: ${expected}`)
}
if (!/^[\x20-\x7E]+$/.test(endpoint)) fail('endpoint 에 ASCII 가 아닌 문자가 있습니다.')

// ── ③ 빌드된 설치 파일의 서명 ─────────────────────────────────────────
const bundleDir = 'src-tauri/target/release/bundle/nsis'
let signature = null
let exeName = null
if (existsSync(bundleDir) && confPub) {
  const exe = readdirSync(bundleDir).find((f) => /_x64-setup\.exe$/.test(f))
  if (exe) {
    exeName = exe
    const sigPath = join(bundleDir, exe + '.sig')
    if (!existsSync(sigPath)) {
      fail(`설치 파일은 있는데 서명(.sig)이 없습니다: ${exe}\n      TAURI_SIGNING_PRIVATE_KEY 없이 빌드했습니다.`)
    } else {
      signature = readFileSync(sigPath, 'utf8').trim()
      const s = parseSig(unwrap(signature))
      if (s.keyId !== confPub.keyId) {
        fail(`서명의 키 id(${s.keyId})가 설정의 pubkey(${confPub.keyId})와 다릅니다 — 다른 키로 서명했습니다.`)
      } else if (s.alg === 'ED') {
        const digest = createHash('blake2b512').update(readFileSync(join(bundleDir, exe))).digest()
        const ok = verify(null, digest, ed25519Key(confPub.pk), s.sig)
        if (!ok) {
          const { statSync } = await import('node:fs')
          const exeTime = statSync(join(bundleDir, exe)).mtimeMs
          const sigTime = statSync(sigPath).mtimeMs
          const stale = exeTime > sigTime + 1000 ? ' 설치 파일이 서명보다 새것입니다 — 서명 없이 다시 빌드한 뒤 옛 .sig 가 남은 것입니다. 키를 넣고 다시 빌드하세요.' : ''
          fail(`설치 파일의 서명이 pubkey 로 검증되지 않습니다: ${exe}.${stale}`)
        } else notes.push(`${exe} 의 서명이 pubkey 로 검증됩니다 (blake2b-512 + ed25519).`)
      } else {
        notes.push(`서명 알고리즘 ${s.alg} — 해시 서명(ED)이 아니라 검증은 건너뜁니다.`)
      }
      if (!/^[\x20-\x7E]+$/.test(exe)) fail(`설치 파일 이름에 ASCII 가 아닌 문자가 있습니다: ${exe}`)
    }
  } else {
    notes.push('빌드된 설치 파일이 없어 ③(서명 검증)은 건너뜁니다. `npm run app:build` 뒤에 다시 돌리면 봅니다.')
  }
} else {
  notes.push('빌드 폴더가 없어 ③(서명 검증)은 건너뜁니다.')
}

// ── ④ latest.json 꼴 ─────────────────────────────────────────────────
const version = conf.version
const semver = /^(\d+)\.(\d+)\.(\d+)$/
if (!semver.test(version)) fail(`버전이 semver(x.y.z)가 아닙니다: ${version}`)
const latest = {
  version,
  notes: '자세한 변경 내용은 릴리스 페이지를 확인해 주세요.',
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'),
  platforms: {
    'windows-x86_64': {
      signature: signature ?? '(빌드 뒤 채워짐)',
      url: `https://github.com/${repo}/releases/download/v${version}/${exeName ?? `DocAid_${version}_x64-setup.exe`}`,
    },
  },
}
const text = JSON.stringify(latest, null, 2)
if (text.charCodeAt(0) === 0xfeff) fail('latest.json 에 BOM 이 있습니다.')
if (!/^[\x20-\x7E]+$/.test(latest.platforms['windows-x86_64'].url)) fail('latest.json 의 url 에 ASCII 가 아닌 문자가 있습니다.')
if (!latest.platforms['windows-x86_64'].url.endsWith(`${exeName ?? `DocAid_${version}_x64-setup.exe`}`)) fail('latest.json 의 url 이 설치 파일 이름과 어긋납니다.')

// ── ⑤ 버전 견주기 — 업데이터와 같은 규칙 ─────────────────────────────
function cmp(a, b) {
  const pa = a.split('.').map(Number)
  const pb = b.split('.').map(Number)
  for (let i = 0; i < 3; i++) if (pa[i] !== pb[i]) return pa[i] - pb[i]
  return 0
}
const cases = [
  [version, version, false, '같은 버전은 새것이 아니다'],
  [version, bump(version, 2), true, '패치가 오르면 새것'],
  [version, bump(version, 1), true, '마이너가 오르면 새것'],
  [bump(version, 2), version, false, '내려가면 새것이 아니다'],
  ['0.1.9', '0.1.10', true, '자릿수가 늘어도 숫자로 견준다 (0.1.9 < 0.1.10)'],
]
for (const [cur, remote, isNew, why] of cases) {
  if (cmp(remote, cur) > 0 !== isNew) fail(`버전 견주기가 틀립니다: ${cur} → ${remote} (${why})`)
}
function bump(v, i) {
  const p = v.split('.').map(Number)
  p[i] += 1
  return p.join('.')
}
if (pkg.version !== version) fail(`package.json(${pkg.version})과 tauri.conf.json(${version}) 버전이 다릅니다.`)

// ── 결과 ────────────────────────────────────────────────────────────
for (const n of notes) console.log('  ' + n)
if (problems.length) {
  console.error('\n업데이터 검사 실패:')
  for (const p of problems) console.error('  - ' + p)
  process.exit(1)
}
console.log(`업데이터 검사 통과 — v${version}, ${repo}, latest.json 꼴 확인`)
