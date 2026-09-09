/**
 * PDF 추출이 믿을 만한지 검사한다.
 *
 *   npm run check:pdf
 *
 * 이 검사가 이 프로젝트에서 가장 중요한 검사다. "근거가 이 파일 몇 쪽 어디에
 * 있다" 는 말이 참이려면, 아래 네 가지가 서로 맞아야 한다.
 *
 *   원문 문장  ->  추출 텍스트  ->  쪽 번호  ->  저장된 문자 위치
 *
 * 앱이 쓰는 것과 **똑같은** `src/lib/pdf/extract.ts` 를 부른다. 검사만 통과하고
 * 앱에서는 다르게 도는 일이 없도록.
 */
import { readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { getDocument, OPS } from 'pdfjs-dist/legacy/build/pdf.mjs'
import {
  documentStatus,
  imageOpCodes,
  layoutPage,
  looksScanned,
  pageKind,
  type PageKind,
  type TextItemLike,
} from '../src/lib/pdf/extract.ts'

const root = resolve(import.meta.dirname, '..')
const pdfDir = join(root, 'test', 'pdf')
const pdfjsDir = join(root, 'node_modules', 'pdfjs-dist')

/**
 * node 에서 pdf.js 는 이 값들을 **파일 경로 + 끝 슬래시**로 받는다.
 * `file://` URL 을 주면 조용히 빈 결과가 나오고, http URL 도 받지 않는다.
 * (앱에서는 브라우저이므로 `/cmaps/` 같은 앱 안 주소를 준다.)
 */
const asset = (name: string) => join(pdfjsDir, name) + '/'

const IMAGE_OPS = imageOpCodes(OPS as unknown as Record<string, unknown>)

const problems: string[] = []
const notes: string[] = []
const fail = (m: string) => problems.push(m)

type Extracted = {
  page: number
  text: string
  itemMap: [number, number, number][]
  items: TextItemLike[]
  isScanned: boolean
  kind: PageKind
}

async function extract(file: string, withCMaps = true): Promise<Extracted[]> {
  const data = new Uint8Array(readFileSync(join(pdfDir, file)))
  const task = getDocument({
    data,
    // 글꼴 자료는 반드시 로컬에서 읽는다. 앱에서도 같은 원칙이다 (설계안 8-2).
    cMapUrl: withCMaps ? asset('cmaps') : undefined,
    cMapPacked: true,
    standardFontDataUrl: asset('standard_fonts'),
    wasmUrl: asset('wasm'),
    iccUrl: asset('iccs'),
    isEvalSupported: false,
  })
  const doc = await task.promise

  const out: Extracted[] = []
  for (let p = 1; p <= doc.numPages; p++) {
    const page = await doc.getPage(p)
    const content = await page.getTextContent()
    const items = content.items as TextItemLike[]
    const { text, itemMap } = layoutPage(items)

    let hasImage = false
    // 글자가 거의 없는 쪽만 그림을 확인한다. 모든 쪽에 하면 느리다.
    if (text.replace(/\s/g, '').length < 50) {
      const ops = await page.getOperatorList()
      hasImage = ops.fnArray.some((fn: number) => IMAGE_OPS.has(fn))
    }

    out.push({
      page: p,
      text,
      itemMap,
      items,
      isScanned: looksScanned(text, hasImage),
      kind: pageKind(text, hasImage),
    })
    page.cleanup()
  }
  await task.destroy()
  return out
}

/** 공백 차이를 무시하고 문장이 들어 있는지 본다 */
const loose = (s: string) => s.replace(/\s+/g, '')

// ─────────────────────────────────────────────────────────────────────────

const expected: { file: string; page: number; sentence: string }[] = JSON.parse(
  readFileSync(join(pdfDir, 'expected.json'), 'utf8'),
)

const files = [...new Set(expected.map((e) => e.file))]
for (const f of ['scanned.pdf', 'mixed.pdf', 'odd-layer.pdf']) {
  if (!files.includes(f)) files.push(f)
}

const cache = new Map<string, Extracted[]>()
for (const file of files) {
  cache.set(file, await extract(file))
}

// ── 1. 문장이 바로 그 쪽에서 나오는가 ─────────────────────────────────
for (const { file, page, sentence } of expected) {
  const pages = cache.get(file)!
  const got = pages.find((p) => p.page === page)
  if (!got) {
    fail(`${file} ${page}쪽이 없습니다 (전체 ${pages.length}쪽).`)
    continue
  }
  if (!loose(got.text).includes(loose(sentence))) {
    fail(
      `${file} ${page}쪽에서 문장을 찾지 못했습니다.\n` +
        `      찾던 것: ${sentence}\n` +
        `      그 쪽 앞부분: ${got.text.slice(0, 120).replace(/\n/g, ' / ')}`,
    )
    continue
  }
  // 다른 쪽에서 나오면 쪽 번호를 잘못 붙이고 있다는 뜻이다
  const elsewhere = pages.filter((p) => p.page !== page && loose(p.text).includes(loose(sentence)))
  if (elsewhere.length > 0) {
    fail(`${file} 의 "${sentence.slice(0, 20)}…" 이 ${elsewhere.map((p) => p.page).join(', ')}쪽에도 있습니다.`)
  }
}

// ── 2. 문자 위치 지도가 텍스트와 정확히 맞는가 ★ ──────────────────────
// 이게 형광펜의 근거다. 하나라도 어긋나면 근거 표시가 엉뚱한 곳을 가리킨다.
for (const [file, pages] of cache) {
  for (const pg of pages) {
    for (const [start, end, idx] of pg.itemMap) {
      const fromText = pg.text.slice(start, end)
      const fromItem = pg.items[idx]?.str
      if (fromItem === undefined) {
        fail(`${file} ${pg.page}쪽: 항목 ${idx} 가 없습니다.`)
        break
      }
      if (fromText !== fromItem) {
        fail(
          `${file} ${pg.page}쪽: 문자 위치가 어긋났습니다 (항목 ${idx}, ${start}~${end}).\n` +
            `      텍스트에서: ${JSON.stringify(fromText)}\n` +
            `      항목에서:   ${JSON.stringify(fromItem)}`,
        )
        break
      }
    }
    // 지도가 텍스트 전체를 덮는가 (공백·줄바꿈은 우리가 끼운 것이니 빼고)
    const mapped = pg.itemMap.reduce((n, [s, e]) => n + (e - s), 0)
    const real = pg.text.replace(/[\n\t ]/g, '').length
    const mappedReal = pg.itemMap.reduce(
      (n, [s, e]) => n + pg.text.slice(s, e).replace(/[\n\t ]/g, '').length,
      0,
    )
    if (mapped > 0 && mappedReal !== real) {
      fail(
        `${file} ${pg.page}쪽: 지도가 덮지 못한 글자가 있습니다 ` +
          `(글자 ${real}자 중 ${mappedReal}자만 덮음).`,
      )
    }
  }
}

// ── 3. 스캔본을 스캔본이라고 하는가 ───────────────────────────────────
{
  const scanned = cache.get('scanned.pdf')!
  if (!scanned.every((p) => p.isScanned)) {
    fail(`scanned.pdf 의 모든 쪽이 스캔본으로 잡혀야 합니다 (${scanned.map((p) => p.isScanned).join(',')}).`)
  }

  const mixed = cache.get('mixed.pdf')!
  if (mixed[0]?.isScanned) fail('mixed.pdf 1쪽은 글자가 있으므로 스캔본이 아닙니다.')
  if (!mixed.slice(1).every((p) => p.isScanned)) {
    fail(`mixed.pdf 2~4쪽은 스캔본이어야 합니다 (${mixed.map((p) => p.isScanned).join(',')}).`)
  }

  // 반대로, 멀쩡한 문서를 스캔본이라고 하면 안 된다
  for (const f of ['plain-ko.pdf', 'table-ko.pdf', 'manual-ko.pdf']) {
    const bad = cache.get(f)!.filter((p) => p.isScanned)
    if (bad.length > 0) fail(`${f} 의 ${bad.map((p) => p.page).join(', ')}쪽을 스캔본으로 잘못 봤습니다.`)
  }
}

// ── 3-2. 문서 상태를 옳게 매기는가 ────────────────────────────────────
{
  const cases: [string, string][] = [
    ['plain-ko.pdf', 'ok'],
    ['table-ko.pdf', 'ok'],
    ['manual-ko.pdf', 'ok'],
    ['cid-ko.pdf', 'ok'],
    ['scanned.pdf', 'scanned'],
    ['mixed.pdf', 'scanned'],
  ]
  for (const [file, want] of cases) {
    const got = documentStatus(cache.get(file)!.map((p) => p.kind))
    if (got !== want) fail(`${file} 의 상태가 '${want}' 여야 하는데 '${got}' 입니다.`)
  }
}

// ── 3-3. cMap 을 못 읽으면 "글자를 못 읽었다"고 해야 한다 ★ ───────────
// 이게 이 검사에서 가장 값진 항목이다. 옛 한글 공문 PDF 는 글꼴을 담지 않고
// cMap 에 기대므로, cMap 번들을 빠뜨리면 **글자 0자로 조용히 등록**된다.
// 이미지도 없어서 스캔본으로도 안 잡힌다. 사용자는 자료를 넣었다고 믿는데
// 프로그램은 "자료에 없습니다" 라고 답하게 된다.
{
  const broken = await extract('cid-ko.pdf', false)
  const chars = broken.reduce((n, p) => n + p.text.replace(/\s/g, '').length, 0)
  if (chars > 0) {
    notes.push(`(참고) cMap 없이도 cid-ko.pdf 에서 ${chars}자가 나왔습니다.`)
  } else {
    const status = documentStatus(broken.map((p) => p.kind))
    if (status !== 'extract_failed') {
      fail(
        `cMap 을 못 읽어 글자가 0자인 문서의 상태가 'extract_failed' 여야 하는데 ` +
          `'${status}' 입니다. 이러면 빈 문서가 멀쩡한 문서처럼 등록됩니다.`,
      )
    } else {
      notes.push('cMap 없이 연 문서는 extract_failed 로 잡힘 — 조용히 등록되지 않는다')
    }
  }
}

// ── 4. 한글이 깨지지 않았는가 ─────────────────────────────────────────
for (const f of ['plain-ko.pdf', 'table-ko.pdf', 'manual-ko.pdf', 'odd-layer.pdf', 'cid-ko.pdf']) {
  const pages = cache.get(f)!
  const all = pages.map((p) => p.text).join('')
  const hangul = (all.match(/[가-힣]/g) ?? []).length
  if (hangul < 100) fail(`${f} 에서 한글이 ${hangul}자밖에 나오지 않았습니다. 글자가 깨졌을 수 있습니다.`)
  // 대체 문자(U+FFFD)가 있으면 어딘가에서 깨진 것이다
  if (all.includes('�')) fail(`${f} 에 깨진 글자(U+FFFD)가 있습니다.`)
}

// ── 5. 기록: 표가 어떻게 나오는지 (P4 검색 품질 판단 자료) ────────────
{
  const t = cache.get('table-ko.pdf')!
  for (const pg of t) {
    const lines = pg.text.split('\n').filter((l) => l.includes('\t'))
    notes.push(`table-ko.pdf ${pg.page}쪽 — 칸이 나뉜 줄 ${lines.length}개`)
    for (const l of lines.slice(0, 4)) notes.push(`    ${l.replace(/\t/g, ' | ')}`)
  }
  const o = cache.get('odd-layer.pdf')!
  for (const pg of o) {
    notes.push(
      `odd-layer.pdf ${pg.page}쪽 — 항목 ${pg.itemMap.length}개, 줄 ${pg.text.split('\n').length}개`,
    )
  }
  const m = cache.get('manual-ko.pdf')!
  notes.push(`manual-ko.pdf — ${m.length}쪽, 글자 ${m.reduce((n, p) => n + p.text.length, 0)}자`)
}

// ─────────────────────────────────────────────────────────────────────────

console.log('\n[기록] 표와 특이 문서가 어떻게 추출되었는지')
notes.forEach((n) => console.log('  ' + n))

if (problems.length > 0) {
  console.error(`\nPDF 추출 검사에서 ${problems.length}건이 걸렸습니다.\n`)
  problems.slice(0, 20).forEach((p, i) => console.error(`  ${i + 1}. ${p}\n`))
  if (problems.length > 20) console.error(`  … 그리고 ${problems.length - 20}건 더\n`)
  process.exit(1)
}

const totalPages = [...cache.values()].reduce((n, ps) => n + ps.length, 0)
console.log(
  `\nPDF 추출 검사 통과 — 파일 ${cache.size}개 ${totalPages}쪽, 문장 대조 ${expected.length}건, ` +
    `문자 위치 지도 전부 일치`,
)
