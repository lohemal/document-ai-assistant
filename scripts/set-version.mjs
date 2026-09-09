/**
 * 세 곳의 버전을 한 번에 맞춘다.
 *
 *   npm run version:set 0.2.0
 *
 * package.json · tauri.conf.json · Cargo.toml 이 어긋나면 업데이터가
 * 새 버전을 알아보지 못하므로, 손으로 고치지 않고 이 스크립트를 쓴다.
 */
import { readFileSync, writeFileSync } from 'node:fs'

const next = process.argv[2]
if (!next || !/^\d+\.\d+\.\d+$/.test(next)) {
  console.error('사용법: npm run version:set 0.2.0')
  process.exit(1)
}

// package.json
const pkgPath = 'package.json'
const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'))
const before = pkg.version
pkg.version = next
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n')

// tauri.conf.json
const confPath = 'src-tauri/tauri.conf.json'
const conf = JSON.parse(readFileSync(confPath, 'utf8'))
conf.version = next
writeFileSync(confPath, JSON.stringify(conf, null, 2) + '\n')

// Cargo.toml — [package] 안의 첫 version 만 바꾼다
const cargoPath = 'src-tauri/Cargo.toml'
let cargo = readFileSync(cargoPath, 'utf8')
cargo = cargo.replace(/^version = ".*"$/m, `version = "${next}"`)
writeFileSync(cargoPath, cargo)

console.log(`v${before} -> v${next}`)
console.log('  package.json / tauri.conf.json / Cargo.toml')
console.log('')
console.log('다음 단계:')
console.log(`  npm run check:model`)
console.log(`  git add . && git commit -m "v${next}"`)
console.log(`  git tag v${next} && git push origin main --tags`)
