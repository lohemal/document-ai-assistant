/**
 * 실제 업무자료 3종을 등록과 **똑같은 길**로 처리해 보고, 자료의 성격을 잰다.
 *
 *   npm run golden:analyze          -> 통계를 화면에 뿌리고
 *                                      test/golden/chunks.json 을 만든다
 *   npm run golden:analyze -- --dump 쪽 텍스트도 test/golden/text/ 에 떨어뜨린다
 *                                      (골든셋 질문을 사람이 만들 때 본다)
 *
 * 앱과 같은 `src/lib/pdf/*`, `src/lib/chunk/*` 를 부른다. 여기서 나온 숫자가
 * 앱에서 보일 숫자와 다르면 그건 이 스크립트의 잘못이다.
 */
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { getDocument, OPS } from 'pdfjs-dist/legacy/build/pdf.mjs'
import {
  documentStatus,
  imageOpCodes,
  layoutPage,
  pageKind,
  statusMessage,
  type ItemSpan,
  type PageKind,
  type TextItemLike,
} from '../src/lib/pdf/extract.ts'
import { buildChunks, type ChunkIn } from '../src/lib/chunk/build.ts'
import { DEFAULT_SPLIT, type SplitOptions } from '../src/lib/chunk/split.ts'

const root = resolve(import.meta.dirname, '..')
const pdfDir = join(root, 'test', 'golden', 'pdf')
const outDir = join(root, 'test', 'golden')
const pdfjsDir = join(root, 'node_modules', 'pdfjs-dist')
const asset = (n: string) => join(pdfjsDir, n) + '/'
const IMAGE_OPS = imageOpCodes(OPS as unknown as Record<string, unknown>)

/** 세 자료를 각각 다른 자료집에 담는다 — 실제로 그렇게 쓸 자료들이다 */
export const GOLDEN_DOCS = [
  { id: 1, collectionId: 1, collection: '방과후학교', file: 'afterschool.pdf', title: '2025 방과후학교 운영 길라잡이(개정판)' },
  { id: 2, collectionId: 2, collection: '학교회계', file: 'accounting.pdf', title: '2026학년도 학교회계 예산편성 및 집행지침' },
  { id: 3, collectionId: 3, collection: '정보통신윤리교육', file: 'ethics.pdf', title: '2026년도 정보통신윤리교육 기본계획' },
]

type Page = {
  page: number
  text: string
  itemMap: ItemSpan[]
  items: number
  hasImage: boolean
  kind: PageKind
  /** 넓은 빈칸(표 칸 사이)이 몇 번 나오는가 */
  tabs: number
  /** 줄이 몇 개인가 */
  lines: number
}

async function pagesOf(file: string): Promise<Page[]> {
  const task = getDocument({
    data: new Uint8Array(readFileSync(join(pdfDir, file))),
    cMapUrl: asset('cmaps'),
    cMapPacked: true,
    standardFontDataUrl: asset('standard_fonts'),
    wasmUrl: asset('wasm'),
  })
  const doc = await task.promise
  const out: Page[] = []
  for (let p = 1; p <= doc.numPages; p++) {
    const page = await doc.getPage(p)
    const items = (await page.getTextContent()).items as TextItemLike[]
    const { text, itemMap } = layoutPage(items)
    let hasImage = false
    try {
      const ops = await page.getOperatorList()
      hasImage = ops.fnArray.some((fn: number) => IMAGE_OPS.has(fn))
    } catch {
      // 그림 판정에 실패해도 글자는 이미 뽑았다
    }
    out.push({
      page: p,
      text,
      itemMap,
      items: items.length,
      hasImage,
      kind: pageKind(text, hasImage),
      tabs: (text.match(/\t/g) ?? []).length,
      lines: text.split('\n').length,
    })
    page.cleanup()
  }
  await task.destroy()
  return out
}

function median(nums: number[]): number {
  if (nums.length === 0) return 0
  const s = [...nums].sort((a, b) => a - b)
  const m = s.length >> 1
  return s.length % 2 ? s[m] : Math.round((s[m - 1] + s[m]) / 2)
}

function stats(chunks: ChunkIn[]) {
  const sizes = chunks.map((c) => c.text.length)
  return {
    count: chunks.length,
    avg: sizes.length ? Math.round(sizes.reduce((a, b) => a + b, 0) / sizes.length) : 0,
    med: median(sizes),
    min: sizes.length ? Math.min(...sizes) : 0,
    max: sizes.length ? Math.max(...sizes) : 0,
    /** 짧은 청크가 얼마나 되는가 — P3 에서 문제가 됐던 것 */
    tiny: sizes.filter((n) => n < 200).length,
    crossPage: chunks.filter((c) => c.pageStart !== c.pageEnd).length,
    tables: chunks.filter((c) => c.kind === 'table').length,
  }
}

const dump = process.argv.includes('--dump')
/** `--split target,max,overlap,min` 으로 다른 기준을 시험해 볼 수 있다 */
const splitArg = process.argv.find((a) => a.startsWith('--split='))
const topicArg = process.argv.find((a) => a.startsWith('--topic='))
const opts: SplitOptions = splitArg
  ? (() => {
      const [target, max, overlap, min] = splitArg.slice('--split='.length).split(',').map(Number)
      return { ...DEFAULT_SPLIT, target, max, overlap, min }
    })()
  : DEFAULT_SPLIT
if (topicArg) opts.topicLevel = Number(topicArg.slice('--topic='.length))

console.log(`분할 기준: target ${opts.target} · max ${opts.max} · overlap ${opts.overlap} · min ${opts.min}\n`)

const documents: unknown[] = []

for (const d of GOLDEN_DOCS) {
  const pages = await pagesOf(d.file)
  const chunks = buildChunks(
    pages.map((p) => ({ page: p.page, text: p.text })),
    opts,
  )
  const chars = pages.reduce((a, p) => a + p.text.length, 0)
  const st = stats(chunks)
  const status = documentStatus(pages.map((p) => p.kind))
  const tablePages = pages.filter((p) => p.tabs >= 5)
  const imagePages = pages.filter((p) => p.hasImage)

  console.log(`── ${d.title}`)
  console.log(`   파일 ${d.file} · 자료집 ${d.collection}`)
  console.log(`   쪽 ${pages.length} · 글자 ${chars.toLocaleString()} · 청크 ${st.count}`)
  console.log(`   청크 길이  평균 ${st.avg} · 중앙값 ${st.med} · 최소 ${st.min} · 최대 ${st.max}`)
  console.log(`   200자 미만 청크 ${st.tiny} · 쪽 넘는 청크 ${st.crossPage} · 표 청크 ${st.tables}`)
  console.log(`   문서 상태 ${status}${statusMessage(status) ? ' — ' + statusMessage(status) : ''}`)
  console.log(`   표로 보이는 쪽 ${tablePages.length} (${tablePages.slice(0, 12).map((p) => p.page).join(',')}${tablePages.length > 12 ? '…' : ''})`)
  console.log(`   그림 있는 쪽 ${imagePages.length} · 글자 못 건진 쪽 ${pages.filter((p) => p.kind !== 'text').map((p) => `${p.page}(${p.kind})`).join(',') || '없음'}`)
  const thin = pages.filter((p) => p.text.replace(/\s/g, '').length < 200)
  console.log(`   글자 200자 미만인 쪽 ${thin.length ? thin.map((p) => `${p.page}:${p.text.replace(/\s/g, '').length}자`).join(', ') : '없음'}`)
  console.log()

  documents.push({
    id: d.id,
    collectionId: d.collectionId,
    title: d.title,
    filename: d.file,
    pageCount: pages.length,
    chars,
    status,
    chunks,
  })

  if (dump) {
    const dir = join(outDir, 'text')
    mkdirSync(dir, { recursive: true })
    writeFileSync(
      join(dir, d.file.replace(/\.pdf$/, '.txt')),
      pages.map((p) => `\n===== ${p.page}쪽 (${p.text.length}자, 항목 ${p.items}, 탭 ${p.tabs}) =====\n${p.text}`).join('\n'),
    )
    writeFileSync(
      join(dir, d.file.replace(/\.pdf$/, '.chunks.txt')),
      chunks
        .map(
          (c) =>
            `\n===== #${c.ord} ${c.pageStart}${c.pageEnd !== c.pageStart ? `-${c.pageEnd}` : ''}쪽 ` +
            `(${c.text.length}자, ${c.kind}) [${c.headingPath}] =====\n${c.text}`,
        )
        .join('\n'),
    )
  }
}

mkdirSync(outDir, { recursive: true })
writeFileSync(
  join(outDir, 'chunks.json'),
  JSON.stringify(
    {
      split: opts,
      collections: GOLDEN_DOCS.map((d) => ({ id: d.collectionId, name: d.collection })),
      documents,
    },
    null,
    1,
  ) + '\n',
)
console.log(`test/golden/chunks.json 을 만들었습니다.${dump ? ' 쪽 텍스트는 test/golden/text/ 에 있습니다.' : ''}`)
