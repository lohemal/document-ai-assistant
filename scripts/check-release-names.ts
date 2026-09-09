/**
 * 배포가 조용히 깨지는 것을 막는 검사.
 *
 *   npm run check:model
 *
 * 방과후 앱에서 productName 이 한글이라 릴리스가 조용히 깨진 적이 있다.
 * GitHub Release 가 첨부 파일 이름에서 한글을 떼어 내는 바람에 latest.json 의
 * url 과 실제 주소가 어긋나, 업데이트가 "받는 중"에서 멈췄다.
 *
 * 세 파일의 버전이 어긋나도 업데이터가 새 버전을 알아보지 못한다.
 *
 * 둘 다 빌드는 성공하고 나중에야 드러나는 종류의 사고라서, 여기서 막는다.
 * (설계안 3-1 이름 규칙)
 */
import { readFileSync } from 'node:fs'

const problems: string[] = []
const fail = (m: string) => problems.push(m)

const pkg = JSON.parse(readFileSync('package.json', 'utf8'))
const conf = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'))
const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8')

// ── 1. 버전 세 곳이 같은가 ────────────────────────────────────────────
const cargoVersion = cargo.match(/^version = "(.*)"$/m)?.[1]
const versions = {
  'package.json': pkg.version,
  'tauri.conf.json': conf.version,
  'Cargo.toml': cargoVersion,
}
const distinct = [...new Set(Object.values(versions))]
if (distinct.length !== 1) {
  fail(
    '버전이 어긋납니다. `npm run version:set <버전>` 으로 맞추세요.\n' +
      Object.entries(versions)
        .map(([k, v]) => `      ${k}: ${v}`)
        .join('\n'),
  )
}

// ── 2. 빌드·배포에 닿는 이름은 전부 ASCII 여야 한다 ───────────────────
const ascii = (s: unknown) => typeof s === 'string' && /^[\x20-\x7E]*$/.test(s)

const asciiFields: [string, unknown][] = [
  ['productName', conf.productName],
  ['mainBinaryName', conf.mainBinaryName],
  ['identifier', conf.identifier],
  ['package.json name', pkg.name],
  ['bundle.publisher', conf.bundle?.publisher],
  ['bundle.homepage', conf.bundle?.homepage],
]
for (const [name, value] of asciiFields) {
  if (value !== undefined && !ascii(value)) {
    fail(`${name} 에 한글(또는 ASCII 밖 문자)이 있습니다: ${JSON.stringify(value)}\n` +
      '      설치 파일 이름이 한글이 되어 릴리스가 조용히 깨집니다. 영문으로 바꾸세요.')
  }
}

const cargoName = cargo.match(/^name = "(.*)"$/m)?.[1]
if (!ascii(cargoName)) fail(`Cargo.toml 의 name 이 ASCII 가 아닙니다: ${cargoName}`)

// ── 3. 반대로, 화면에 보이는 이름은 한글이어야 한다 ───────────────────
const title = conf.app?.windows?.[0]?.title
if (typeof title !== 'string' || !/[가-힣]/.test(title)) {
  fail(`창 제목이 한글이 아닙니다: ${JSON.stringify(title)}\n` +
    '      사용자에게 보이는 이름은 한글이어야 합니다 (설계안 3-1).')
}

// ── 4. 업데이터 설정이 실제 저장소를 가리키는가 ───────────────────────
const endpoint: string | undefined = conf.plugins?.updater?.endpoints?.[0]
const homepage: string | undefined = conf.bundle?.homepage
if (endpoint && homepage) {
  const repo = homepage.replace(/^https:\/\/github\.com\//, '').replace(/\/$/, '')
  if (!endpoint.includes(`/${repo}/releases/`)) {
    fail(`업데이터 endpoint 가 homepage 의 저장소와 다릅니다.\n` +
      `      endpoint: ${endpoint}\n      homepage: ${homepage}`)
  }
}
if (!endpoint?.endsWith('/latest.json')) {
  fail('업데이터 endpoint 가 latest.json 으로 끝나지 않습니다.')
}

const pubkey: string | undefined = conf.plugins?.updater?.pubkey
if (!pubkey || pubkey.startsWith('PLACEHOLDER')) {
  fail(
    '업데이터 공개키가 아직 채워지지 않았습니다.\n' +
      '      npx tauri signer generate -w %USERPROFILE%\\.docaid-release\\docaid.key\n' +
      '      로 만든 뒤 공개키를 tauri.conf.json 의 plugins.updater.pubkey 에 넣으세요.',
  )
}

// ── 결과 ──────────────────────────────────────────────────────────────
if (problems.length > 0) {
  console.error(`\n배포 이름·버전 검사에서 ${problems.length}건이 걸렸습니다.\n`)
  problems.forEach((p, i) => console.error(`  ${i + 1}. ${p}\n`))
  process.exit(1)
}

console.log(`배포 이름·버전 검사 통과 (v${pkg.version}, ${conf.productName})`)
