/**
 * 쪽 텍스트들을 이어 붙여 자른 뒤, 다시 **쪽마다의 문자 구간**으로 되돌린다.
 *
 * 쪽 단위로 자르지 않는 까닭은, 한 문단이 쪽을 걸쳐 있을 때 억지로 끊기지
 * 않게 하려는 것이다. 대신 자른 뒤에 반드시 쪽으로 되돌려 놓아야 형광펜을
 * 칠할 수 있다. 그 되돌리는 일이 이 파일의 전부다.
 */
import { normalize } from '../pdf/extract.ts'
import { DEFAULT_SPLIT, splitDocument, type SplitOptions } from './split.ts'

export type PageText = {
  page: number
  text: string
}

/** 청크가 어느 쪽 어디를 가리키는가 */
export type ChunkSpan = {
  page: number
  /** 그 쪽 텍스트 안의 시작 위치 */
  charStart: number
  charEnd: number
}

export type ChunkIn = {
  /** 문서 안에서 몇 번째 조각인가 */
  ord: number
  headingPath: string
  text: string
  textNorm: string
  kind: 'text' | 'table'
  pageStart: number
  pageEnd: number
  spans: ChunkSpan[]
}

/** 쪽을 이을 때 끼우는 글자. 쪽 경계에서 낱말이 붙지 않게 한다. */
const PAGE_JOIN = '\n'

type PageRange = { page: number; start: number; end: number }

export function buildDocText(pages: PageText[]): { text: string; ranges: PageRange[] } {
  const ranges: PageRange[] = []
  let text = ''
  for (const p of pages) {
    if (text.length > 0) text += PAGE_JOIN
    const start = text.length
    text += p.text
    ranges.push({ page: p.page, start, end: text.length })
  }
  return { text, ranges }
}

/**
 * 문서 전체 좌표 [start, end) 를 쪽마다의 구간으로 쪼갠다.
 *
 * 쪽 사이에 끼운 글자는 어느 쪽에도 넣지 않는다. 그 자리는 원본 PDF 에
 * 없는 글자라서 칠할 곳이 없다.
 */
export function toSpans(start: number, end: number, ranges: PageRange[]): ChunkSpan[] {
  const out: ChunkSpan[] = []
  for (const r of ranges) {
    const s = Math.max(start, r.start)
    const e = Math.min(end, r.end)
    if (e <= s) continue
    out.push({ page: r.page, charStart: s - r.start, charEnd: e - r.start })
  }
  return out
}

export function buildChunks(pages: PageText[], opts: SplitOptions = DEFAULT_SPLIT): ChunkIn[] {
  const usable = pages.filter((p) => p.text.trim().length > 0)
  if (usable.length === 0) return []

  const { text, ranges } = buildDocText(usable)
  const drafts = splitDocument(text, opts)

  const out: ChunkIn[] = []
  for (const d of drafts) {
    const spans = toSpans(d.start, d.end, ranges)
    if (spans.length === 0) continue
    const body = text.slice(d.start, d.end)
    out.push({
      ord: out.length,
      headingPath: d.headingPath,
      text: body,
      textNorm: normalize(body),
      kind: d.kind,
      pageStart: spans[0].page,
      pageEnd: spans[spans.length - 1].page,
      spans,
    })
  }
  return out
}

/** 조각 크기를 살펴본다. 검사와 화면에서 함께 쓴다. */
export function chunkStats(chunks: ChunkIn[]) {
  if (chunks.length === 0) return { count: 0, min: 0, max: 0, avg: 0, crossPage: 0, tables: 0 }
  const sizes = chunks.map((c) => c.text.length)
  return {
    count: chunks.length,
    min: Math.min(...sizes),
    max: Math.max(...sizes),
    avg: Math.round(sizes.reduce((a, b) => a + b, 0) / sizes.length),
    crossPage: chunks.filter((c) => c.pageStart !== c.pageEnd).length,
    tables: chunks.filter((c) => c.kind === 'table').length,
  }
}
