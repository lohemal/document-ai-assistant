/**
 * 청크 하나가 PDF 위에서 어디에 칠해지는지 숫자로 본다. 개발용 도구다.
 *
 *   node --experimental-strip-types scripts/dump-rects.ts table-ko.pdf 1
 *
 * 화면에서 "칠해져야 할 곳이 안 칠해졌다" 싶을 때, 어느 항목이 빠졌는지
 * 여기서 가린다.
 */
import { readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs'
import { layoutPage, type ItemSpan, type TextItemLike } from '../src/lib/pdf/extract.ts'
import { buildChunks } from '../src/lib/chunk/build.ts'
import { rectsForRange, type ItemGeom } from '../src/lib/pdf/highlight.ts'

const root = resolve(import.meta.dirname, '..')
const file = process.argv[2] ?? 'table-ko.pdf'
const wantOrd = Number(process.argv[3] ?? 0)
const pdfjsDir = join(root, 'node_modules', 'pdfjs-dist')
const asset = (n: string) => join(pdfjsDir, n) + '/'

const task = getDocument({
  data: new Uint8Array(readFileSync(join(root, 'test', 'pdf', file))),
  cMapUrl: asset('cmaps'),
  cMapPacked: true,
  standardFontDataUrl: asset('standard_fonts'),
})
const doc = await task.promise

const pages: { page: number; text: string; itemMap: ItemSpan[]; items: TextItemLike[] }[] = []
for (let p = 1; p <= doc.numPages; p++) {
  const pg = await doc.getPage(p)
  const items = (await pg.getTextContent()).items as TextItemLike[]
  const { text, itemMap } = layoutPage(items)
  pages.push({ page: p, text, itemMap, items })
}

const chunks = buildChunks(pages.map((p) => ({ page: p.page, text: p.text })))
const chunk = chunks[wantOrd]
if (!chunk) {
  console.error(`청크 #${wantOrd} 가 없습니다 (모두 ${chunks.length}개).`)
  process.exit(1)
}

console.log(`${file} 청크 #${chunk.ord} · ${chunk.text.length}자 · ${chunk.kind}`)
console.log(`제목: ${chunk.headingPath}\n`)

for (const span of chunk.spans) {
  const pg = pages.find((p) => p.page === span.page)!
  const pdfPage = await doc.getPage(span.page)
  const viewport = pdfPage.getViewport({ scale: 1 })

  const covered = pg.itemMap.filter(([s, e]) => e > span.charStart && s < span.charEnd)
  const rects = rectsForRange(
    pg.itemMap,
    pg.items as unknown as ItemGeom[],
    viewport.transform,
    1,
    span.charStart,
    span.charEnd,
  )

  console.log(`${span.page}쪽 ${span.charStart}~${span.charEnd} — 걸린 항목 ${covered.length}개, 네모 ${rects.length}개`)

  // 네모가 안 나온 항목을 짚어 준다
  const skipped = covered.filter(([, , idx]) => {
    const it = pg.items[idx]
    return !it || !it.str || !(it.width > 0)
  })
  for (const [s, e, idx] of skipped) {
    const it = pg.items[idx]
    console.log(
      `   칠하지 못함: 항목 ${idx} ${JSON.stringify(pg.text.slice(s, e))} ` +
        `width=${it?.width} height=${it?.height} transform=${JSON.stringify(it?.transform)}`,
    )
  }
  if (skipped.length === 0) console.log('   모두 칠함')
  console.log('')
}

await task.destroy()
