// 청크를 그대로 본다:  node scripts/show-chunk.mjs 2 16 44 47
import { readFileSync } from 'node:fs'
const g = JSON.parse(readFileSync(new URL('../test/golden/chunks.json', import.meta.url), 'utf8'))
const [did, ...ords] = process.argv.slice(2)
const d = g.documents.find((x) => x.id === Number(did))
for (const o of ords) {
  const c = d.chunks.find((c) => c.ord === Number(o))
  if (!c) { console.log(`### D${did} #${o} 없음`); continue }
  console.log(`### D${did} #${c.ord} p${c.pageStart}-${c.pageEnd} [${c.kind}] ${c.headingPath}`)
  console.log(c.text.replace(/\t/g, ' | '))
  console.log()
}
