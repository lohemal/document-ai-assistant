// 지금까지 만든 자동 검사를 **한 번에** 돈다 (P8 최종 회귀).
//
//   npm run check:all
//
// 하나가 실패하면 거기서 멈추지 않고 끝까지 돌린 뒤 무엇이 실패했는지 모아 말한다.
// CI(check.yml)도 같은 검사를 같은 차례로 돈다.
import { spawnSync } from 'node:child_process'

const steps = [
  ['배포 이름·버전', 'npm', ['run', 'check:model']],
  ['업데이터 (키·endpoint·서명·latest.json 꼴)', 'npm', ['run', 'check:updater']],
  ['PDF 추출 · 쪽 · 문자 위치', 'npm', ['run', 'check:pdf']],
  ['청크 · 위치 → pdf.js 항목', 'npm', ['run', 'check:chunk']],
  ['골든 셋 근거 위치', 'npm', ['run', 'check:golden']],
  ['타입 검사 · 화면 빌드', 'npm', ['run', 'build']],
  // Rust: 마이그레이션(downgrade 보호 포함) · 검색 · 검증 · 거부 · 작업 기록 ·
  // 검색 품질(골든 셋 벡터) · P5 답변 평가(갈무리된 답) 까지 전부 #[test] 다
  ['Rust 시험 (마이그레이션 · 검색 · 검증 · 기록 · 골든 셋 · 답변 평가)', 'cargo', ['test', '--lib'], 'src-tauri'],
]

const results = []
const t0 = Date.now()
for (const [name, cmd, args, cwd] of steps) {
  const started = Date.now()
  console.log(`\n══ ${name} ══`)
  const r = spawnSync(cmd, args, { stdio: 'inherit', shell: true, cwd: cwd ?? process.cwd() })
  const sec = ((Date.now() - started) / 1000).toFixed(0)
  results.push({ name, ok: r.status === 0, sec })
}

console.log('\n══ 결과 ══')
for (const r of results) console.log(`  ${r.ok ? '✓' : '✗'} ${r.name}  (${r.sec}초)`)
const failed = results.filter((r) => !r.ok)
console.log(`\n${results.length}개 가운데 ${results.length - failed.length}개 통과 · ${((Date.now() - t0) / 1000).toFixed(0)}초`)
if (failed.length) {
  console.error(`실패: ${failed.map((f) => f.name).join(', ')}`)
  process.exit(1)
}
