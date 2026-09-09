/**
 * 골든 셋의 **정답 문장이 원본 PDF 의 그 자리에 정말 있는지** 확인한다.
 *
 *   npm run check:golden
 *
 * 검색이 좋은 청크를 골라도, 그 청크가 가리키는 PDF 자리가 어긋나 있으면
 * 근거 표시는 거짓이 된다. 점수보다 이쪽이 먼저다.
 *
 * 확인하는 사슬은 P3 하이라이트가 쓰는 것과 **완전히 같다**:
 *
 *   정답 문장 → 청크 → chunk_span(쪽·문자구간) → item_map → pdf.js 항목
 *
 * 마지막 항목들의 글자를 이어 붙여 정답 문장이 그 안에 있으면, 화면에서
 * 형광펜이 칠할 자리가 맞다는 뜻이다(좌표는 그 항목들에서 뽑는다).
 */
import { readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs'
import { itemsForRange, layoutPage, type ItemSpan, type TextItemLike } from '../src/lib/pdf/extract.ts'

const root = resolve(import.meta.dirname, '..')
const dir = join(root, 'test', 'golden')
const pdfjsDir = join(root, 'node_modules', 'pdfjs-dist')
const asset = (n: string) => join(pdfjsDir, n) + '/'

type Span = { page: number; charStart: number; charEnd: number }
type Chunk = { ord: number; text: string; pageStart: number; pageEnd: number; kind: string; spans: Span[] }
type Doc = { id: number; title: string; filename: string; chunks: Chunk[] }

const corpus = JSON.parse(readFileSync(join(dir, 'chunks.json'), 'utf8')) as { documents: Doc[] }
const golden = JSON.parse(readFileSync(join(dir, 'golden.json'), 'utf8')) as {
  questions: {
    question_id: string
    document_id: number
    question: string
    expected_chunk_ids: number[]
    expected_pages: number[]
    evidence: string[]
  }[]
}

const bare = (s: string) => s.replace(/\s+/g, '')

type Page = { text: string; itemMap: ItemSpan[]; items: TextItemLike[] }

/** 쪽을 하나씩 읽는다. 368쪽 문서를 다 읽지 않도록 필요한 쪽만 캐시한다. */
async function reader(file: string) {
  const task = getDocument({
    data: new Uint8Array(readFileSync(join(dir, 'pdf', file))),
    cMapUrl: asset('cmaps'),
    cMapPacked: true,
    standardFontDataUrl: asset('standard_fonts'),
    wasmUrl: asset('wasm'),
  })
  const doc = await task.promise
  const cache = new Map<number, Page>()
  return {
    async page(n: number): Promise<Page> {
      const got = cache.get(n)
      if (got) return got
      const pg = await doc.getPage(n)
      const items = (await pg.getTextContent()).items as TextItemLike[]
      const { text, itemMap } = layoutPage(items)
      const page = { text, itemMap, items }
      cache.set(n, page)
      pg.cleanup()
      return page
    },
    async close() {
      await task.destroy()
    },
  }
}

const problems: string[] = []
let checked = 0
let itemCount = 0

for (const d of corpus.documents) {
  const qs = golden.questions.filter((q) => q.document_id === d.id)
  if (qs.length === 0) continue
  const pdf = await reader(d.filename)

  for (const q of qs) {
    for (const ord of q.expected_chunk_ids) {
      const chunk = d.chunks.find((c) => c.ord === ord)
      if (!chunk) {
        problems.push(`${q.question_id}: 청크 #${ord} 이 없습니다`)
        continue
      }

      // ① 청크 글자를 쪽 구간에서 되짚으면 같아야 한다
      let rebuilt = ''
      for (const s of chunk.spans) {
        const page = await pdf.page(s.page)
        if (rebuilt.length > 0) rebuilt += '\n'
        rebuilt += page.text.slice(s.charStart, s.charEnd)
      }
      if (rebuilt !== chunk.text) {
        problems.push(
          `${q.question_id} 청크 #${ord}: 쪽 구간에서 되짚은 글자가 청크와 다릅니다\n` +
            `      청크:   ${JSON.stringify(chunk.text.slice(0, 50))}\n` +
            `      되짚음: ${JSON.stringify(rebuilt.slice(0, 50))}`,
        )
        continue
      }

      // ② 그 구간에 걸리는 pdf.js 항목의 글자 안에 정답 문장이 있어야 한다
      //    (화면 좌표는 이 항목들에서 뽑는다 — 여기가 맞으면 형광펜도 맞다)
      let fromItems = ''
      let used = 0
      for (const s of chunk.spans) {
        const page = await pdf.page(s.page)
        const idx = itemsForRange(page.itemMap, s.charStart, s.charEnd)
        used += idx.length
        for (const i of idx) fromItems += page.items[i].str
      }
      itemCount += used
      if (used === 0) {
        problems.push(`${q.question_id} 청크 #${ord}: 걸리는 PDF 항목이 하나도 없습니다`)
        continue
      }

      for (const anchor of q.evidence) {
        if (!bare(rebuilt).includes(bare(anchor))) continue // 다른 청크에 든 문장
        if (!bare(fromItems).includes(bare(anchor))) {
          problems.push(
            `${q.question_id} 청크 #${ord}: 정답 문장이 PDF 항목 글자에 없습니다\n` +
              `      찾던 문장: ${JSON.stringify(anchor)}\n` +
              `      항목 글자: ${JSON.stringify(fromItems.slice(0, 80))}`,
          )
        }
      }

      // ③ 쪽 번호가 골든 셋에 적힌 것과 맞아야 한다
      const pages = chunk.spans.map((s) => s.page)
      if (!pages.some((p) => q.expected_pages.includes(p))) {
        problems.push(
          `${q.question_id} 청크 #${ord}: 쪽이 어긋납니다 (청크 ${pages.join(',')} / 골든 ${q.expected_pages.join(',')})`,
        )
      }
      checked++
    }
  }
  await pdf.close()
  console.log(`  ${d.filename.padEnd(18)} 물음 ${qs.length}개 · 청크 ${qs.reduce((a, q) => a + q.expected_chunk_ids.length, 0)}개 확인`)
}

console.log(
  `\n정답 청크 ${checked}개 · PDF 항목 ${itemCount}개를 짚어 봤습니다.`,
)

if (problems.length > 0) {
  console.error(`\n어긋난 곳 ${problems.length}건`)
  for (const p of problems) console.error(`  - ${p}`)
  process.exit(1)
}
console.log('정답 문장 → 청크 → 쪽·문자구간 → PDF 항목이 전부 맞습니다.')
