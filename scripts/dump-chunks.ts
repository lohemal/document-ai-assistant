/**
 * 시험용 PDF 를 청크로 나눈 결과를 눈으로 본다. 개발용 도구다.
 *
 *   node --experimental-strip-types scripts/dump-chunks.ts cid-ko.pdf
 *   node --experimental-strip-types scripts/dump-chunks.ts table-ko.pdf full
 */
import { readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs'
import { layoutPage, type TextItemLike } from '../src/lib/pdf/extract.ts'
import { buildChunks, chunkStats } from '../src/lib/chunk/build.ts'

const root = resolve(import.meta.dirname, '..')
const file = process.argv[2] ?? 'plain-ko.pdf'
const full = process.argv[3] === 'full'
const pdfjsDir = join(root, 'node_modules', 'pdfjs-dist')
const asset = (n: string) => join(pdfjsDir, n) + '/'

const task = getDocument({
  data: new Uint8Array(readFileSync(join(root, 'test', 'pdf', file))),
  cMapUrl: asset('cmaps'),
  cMapPacked: true,
  standardFontDataUrl: asset('standard_fonts'),
})
const doc = await task.promise
const pages: { page: number; text: string }[] = []
for (let p = 1; p <= doc.numPages; p++) {
  const page = await doc.getPage(p)
  const { text } = layoutPage((await page.getTextContent()).items as TextItemLike[])
  pages.push({ page: p, text })
  page.cleanup()
}
await task.destroy()

const chunks = buildChunks(pages)
const s = chunkStats(chunks)
console.log(
  `${file} — 청크 ${s.count}개 · 평균 ${s.avg}자 · 최소 ${s.min} · 최대 ${s.max} · ` +
    `쪽 걸침 ${s.crossPage} · 표 ${s.tables}\n`,
)

for (const c of chunks) {
  const where = c.spans.map((x) => `${x.page}쪽 ${x.charStart}~${x.charEnd}`).join(', ')
  console.log(`[${c.ord}] ${c.text.length}자 · ${c.kind} · ${where}`)
  if (c.headingPath) console.log(`    제목: ${c.headingPath}`)
  const body = full ? c.text : c.text.slice(0, 110)
  console.log(
    '    ' + body.replace(/\n/g, '\n    ').replace(/\t/g, ' | ') + (full ? '' : c.text.length > 110 ? ' …' : ''),
  )
  console.log('')
}
