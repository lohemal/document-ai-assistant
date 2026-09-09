/**
 * 글을 검색하기 좋은 크기로 자른다.
 *
 * 이 파일이 지켜야 할 것은 딱 하나다 — **자른 자리를 문자 오프셋으로 정확히
 * 돌려준다.** 텍스트를 새로 만들거나 다듬지 않는다. 오직 "어디부터 어디까지"만
 * 정한다. 그래야 P2 가 만든 문자 위치 지도가 그대로 살아 있고, 나중에 원본
 * PDF 위에 형광펜을 칠할 수 있다.
 *
 * 밖에서 가져오는 것이 없다. 앱과 검사 스크립트가 같은 코드를 돌린다.
 */

export type ChunkDraft = {
  /** 문서 전체 텍스트 안의 시작 위치 */
  start: number
  /** 끝 위치 (이 자리는 포함하지 않는다) */
  end: number
  /** "제3장 이용권 > 3. 사용 범위" */
  headingPath: string
  /** text | table */
  kind: 'text' | 'table'
}

export type SplitOptions = {
  /** 이만큼을 목표로 자른다 */
  target: number
  /** 이보다 크면 반드시 자른다 */
  max: number
  /** 앞 조각의 끝을 이만큼 물려 준다 */
  overlap: number
  /** 이보다 작은 조각은 앞 조각에 붙인다 */
  min: number
  /** 이 깊이까지의 제목에서만 "반드시" 끊는다. 더 작은 제목은 끊기 좋은 자리로만 쓴다 */
  topicLevel: number
}

export const DEFAULT_SPLIT: SplitOptions = {
  target: 700,
  max: 900,
  overlap: 150,
  min: 120,
  topicLevel: 1,
}

// ── 제목 찾기 ────────────────────────────────────────────────────────────

type Heading = { start: number; end: number; level: number; text: string }

/**
 * 줄 첫머리에서 제목처럼 생긴 것을 찾는다.
 *
 * 학교 자료는 번호매기기가 제각각이라 완벽할 수 없다. 여기서 노리는 것은
 * **다른 장의 내용이 한 조각에 섞이지 않게** 하는 것이다. 몇 개 놓쳐도
 * 문단·문장 경계가 받쳐 준다.
 */
const HEADING_RULES: { re: RegExp; level: number }[] = [
  { re: /^제\s*\d+\s*[장편부]/, level: 1 },
  { re: /^[ⅠⅡⅢⅣⅤⅥⅦⅧⅨⅩ]+\s*[.．]/, level: 1 },
  { re: /^【[^】]{1,40}】/, level: 1 },
  { re: /^붙임\s*\d*\s*[.．]?/, level: 1 },
  { re: /^제\s*\d+\s*[절관]/, level: 2 },
  { re: /^제\s*\d+\s*[조항]/, level: 2 },
  { re: /^\d{1,2}\s*[.．]\s+\S/, level: 2 },
  { re: /^[가나다라마바사아자차카타파하]\s*[.．]\s+\S/, level: 3 },
  { re: /^[①-⑳]/, level: 4 },
  { re: /^\(\s*\d{1,2}\s*\)\s+\S/, level: 4 },
]

function findHeadings(text: string): Heading[] {
  const out: Heading[] = []
  let lineStart = 0
  while (lineStart <= text.length) {
    let lineEnd = text.indexOf('\n', lineStart)
    if (lineEnd === -1) lineEnd = text.length
    const line = text.slice(lineStart, lineEnd)
    const trimmed = line.trim()

    // 표의 한 줄은 제목이 아니다
    if (trimmed.length > 0 && trimmed.length <= 60 && !line.includes('\t')) {
      for (const { re, level } of HEADING_RULES) {
        if (re.test(trimmed)) {
          out.push({ start: lineStart, end: lineEnd, level, text: trimmed })
          break
        }
      }
    }
    lineStart = lineEnd + 1
  }
  return out
}

/** 제목들을 쌓아 "제3장 이용권 > 3. 사용 범위" 를 만든다 */
function headingPathAt(headings: Heading[], pos: number): string {
  const stack: Heading[] = []
  for (const h of headings) {
    if (h.start > pos) break
    while (stack.length > 0 && stack[stack.length - 1].level >= h.level) stack.pop()
    stack.push(h)
  }
  return stack.map((h) => h.text).join(' > ')
}

// ── 끊어도 되는 자리 ─────────────────────────────────────────────────────

/**
 * 한국어 문장이 끝나는 자리를 찾는다.
 *
 * `.` 만 보고 끊으면 안 된다. 학교 자료에는 이런 것이 흔하다:
 *   `1.` `제3조.` `2026. 3. 1.` `가.` `100%.`
 * 그래서 앞뒤를 같이 본다.
 */
export function sentenceBreaks(text: string): number[] {
  const out: number[] = []
  for (let i = 0; i < text.length; i++) {
    const c = text[i]
    if (c !== '.' && c !== '!' && c !== '?' && c !== '。') continue

    // 뒤가 공백이거나 끝이어야 문장 끝이다
    const next = text[i + 1]
    if (next !== undefined && !/\s/.test(next)) continue

    if (c === '.') {
      const prev = text[i - 1]
      // 숫자 뒤 마침표는 번호이거나 날짜다: "1." "2026. 3. 1."
      if (prev !== undefined && /[0-9]/.test(prev)) continue
      // 줄 첫머리의 한두 글자 뒤 마침표는 목록 기호다: "가." "A."
      const lineStart = text.lastIndexOf('\n', i - 1) + 1
      if (i - lineStart <= 1) continue
    }
    out.push(i + 1)
  }
  return out
}

/** 끊어도 되는 자리를 좋은 순서대로 모은다 */
type BreakKind = 'heading' | 'para' | 'line' | 'sentence'

function breakPoints(text: string): Map<number, BreakKind> {
  const map = new Map<number, BreakKind>()
  for (const at of sentenceBreaks(text)) map.set(at, 'sentence')

  // 줄바꿈
  for (let i = 0; i < text.length; i++) {
    if (text[i] === '\n') map.set(i + 1, 'line')
  }
  // 빈 줄(문단)이 가장 좋다
  const para = /\n[ \t]*\n/g
  let m: RegExpExecArray | null
  while ((m = para.exec(text)) !== null) map.set(m.index + m[0].length, 'para')

  return map
}

const RANK: Record<BreakKind, number> = { heading: 4, para: 3, line: 2, sentence: 1 }

// ── 표 ───────────────────────────────────────────────────────────────────

type Line = { start: number; end: number; text: string }

function lines(text: string): Line[] {
  const out: Line[] = []
  let s = 0
  while (s <= text.length) {
    let e = text.indexOf('\n', s)
    if (e === -1) e = text.length
    out.push({ start: s, end: e, text: text.slice(s, e) })
    s = e + 1
  }
  return out
}

/**
 * 칸이 나뉜 줄이 세 줄 넘게 이어지면 표로 본다.
 *
 * 표를 알아보는 까닭은 구조를 복원하려는 것이 아니라, **표 한가운데를 자르지
 * 않으려는** 것이다. 잘리면 값이 어느 항목의 것인지 알 수 없게 된다.
 */
function tableRanges(text: string): { start: number; end: number }[] {
  const ls = lines(text)
  const out: { start: number; end: number }[] = []
  let runStart = -1
  let runEnd = -1
  let count = 0

  const cells = (l: Line) => (l.text.match(/\t/g) ?? []).length

  for (const l of ls) {
    if (cells(l) >= 1) {
      if (count === 0) runStart = l.start
      runEnd = l.end
      count++
    } else {
      if (count >= 3) out.push({ start: runStart, end: runEnd })
      count = 0
    }
  }
  if (count >= 3) out.push({ start: runStart, end: runEnd })
  return out
}

// ── 자르기 ───────────────────────────────────────────────────────────────

/**
 * 문서 전체 텍스트를 조각으로 자른다.
 *
 * 자르는 자리를 고르는 차례:
 *   ① 제목 경계 — 다른 장이 섞이지 않게
 *   ② 문단(빈 줄)
 *   ③ 줄바꿈
 *   ④ 문장 끝
 *   ⑤ 그래도 넘치면 그냥 자른다
 */
export function splitDocument(text: string, opts: SplitOptions = DEFAULT_SPLIT): ChunkDraft[] {
  if (text.trim().length === 0) return []

  const headings = findHeadings(text)
  const breaks = breakPoints(text)
  const tables = tableRanges(text)

  const inTable = (pos: number) => tables.find((t) => pos > t.start && pos < t.end)

  // 큰 제목이 시작하는 자리는 "다른 이야기가 시작되는" 자리다.
  //
  // 다만 **제목마다 무조건 끊지는 않는다.** 학교 규정은 제목이 촘촘해서
  // 그렇게 하면 "제1장 총칙" 여섯 글자짜리 조각이 나온다. 그런 조각은
  // 검색에 아무 쓸모가 없다. 그래서 조각이 어느 정도 자란 뒤에만 끊는다.
  const topicStops = headings
    .filter((h) => h.level <= opts.topicLevel && !inTable(h.start))
    .map((h) => h.start)
    .sort((a, b) => a - b)

  // 작은 제목도 끊기 좋은 자리이긴 하다. 문단보다 살짝 높게 친다.
  for (const h of headings) {
    if (!inTable(h.start)) breaks.set(h.start, 'heading')
  }

  const drafts: ChunkDraft[] = []
  let cursor = 0

  while (cursor < text.length) {
    // 앞쪽 공백은 건너뛴다
    while (cursor < text.length && /\s/.test(text[cursor])) cursor++
    if (cursor >= text.length) break

    const hardLimit = Math.min(cursor + opts.max, text.length)

    // 조각이 쓸 만한 크기로 자란 뒤에 나오는 첫 큰 제목
    const stop = topicStops.find((s) => s >= cursor + opts.min)

    let end: number
    if (stop !== undefined && stop <= cursor + opts.max) {
      end = stop
    } else {
      // 목표 크기 근처에서 가장 좋은 자리를 고른다
      end = bestBreak(breaks, cursor, cursor + opts.target, hardLimit, opts.min) ?? hardLimit
    }

    // 표 한가운데면 표 끝까지 밀어 준다 (너무 커지지 않는 선에서)
    const t = inTable(end)
    if (t && t.end - cursor <= opts.max * 2) end = t.end

    if (end <= cursor) end = hardLimit

    // 꼬리가 너무 짧으면 이 조각에 붙인다
    const rest = text.slice(end).trim().length
    if (rest > 0 && rest < opts.min && end - cursor + rest <= opts.max * 1.3) {
      end = text.length
    }

    const draft = makeDraft(text, cursor, end, headings, tables)
    if (draft) drafts.push(draft)

    if (end >= text.length) break

    // 다음 조각은 조금 물려서 시작한다. 다만 제목 경계는 넘지 않는다.
    let next = end
    if (opts.overlap > 0 && !topicStops.includes(end)) {
      const want = Math.max(cursor + 1, end - opts.overlap)
      const back = bestBreak(breaks, want, want, end, 0) ?? end
      // 제목을 거슬러 올라가지 않는다
      let limited = back
      for (const s of topicStops) if (s > back && s <= end) limited = Math.max(limited, s)
      next = Math.max(cursor + 1, limited)
    }
    cursor = next
  }

  return drafts
}

/**
 * `near` 근처에서 끊기 좋은 자리를 고른다.
 *
 * 등급(제목 > 문단 > 줄 > 문장)은 **가까운 후보들 사이에서만** 따진다.
 * 등급을 거리보다 크게 치면, 한참 떨어진 제목이 목표 지점의 줄바꿈을 이겨서
 * "붙임. 자유수강권 운영지침" 열네 글자짜리 조각이 나온다. 실제로 그랬다.
 *
 * 그래서 먼저 **쓸 만한 크기가 되는 구간**을 정하고, 그 안에서 등급을 본다.
 * 그 안에 아무것도 없으면 목표에 가장 가까운 자리를 쓴다.
 */
function bestBreak(
  breaks: Map<number, BreakKind>,
  from: number,
  near: number,
  limit: number,
  min: number,
): number | null {
  const floor = from + min // 이보다 짧은 조각은 만들지 않는다
  const windowStart = Math.max(floor, near - Math.round((near - from) * 0.25))

  let best: number | null = null
  let bestRank = -1
  let bestDist = Infinity
  let nearest: number | null = null
  let nearestDist = Infinity

  for (const [at, kind] of breaks) {
    if (at <= floor || at > limit) continue
    const dist = Math.abs(at - near)

    if (dist < nearestDist) {
      nearestDist = dist
      nearest = at
    }
    if (at < windowStart) continue

    const rank = RANK[kind]
    if (rank > bestRank || (rank === bestRank && dist < bestDist)) {
      bestRank = rank
      bestDist = dist
      best = at
    }
  }
  return best ?? nearest
}

function makeDraft(
  text: string,
  start: number,
  end: number,
  headings: Heading[],
  tables: { start: number; end: number }[],
): ChunkDraft | null {
  // 앞뒤 공백을 오프셋에서 덜어 낸다. 텍스트를 다듬는 것이 아니라 범위를 줄인다.
  let s = start
  let e = end
  while (s < e && /\s/.test(text[s])) s++
  while (e > s && /\s/.test(text[e - 1])) e--
  if (e <= s) return null

  const overlapsTable = tables.some((t) => t.start < e && t.end > s)

  return {
    start: s,
    end: e,
    headingPath: headingPathAt(headings, s),
    kind: overlapsTable ? 'table' : 'text',
  }
}
