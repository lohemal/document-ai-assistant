/**
 * PDF 한 쪽에서 텍스트와 **문자 위치 지도**를 만든다.
 *
 * 이 파일은 이 프로그램에서 가장 조심스럽게 다뤄야 하는 곳이다. 여기서 만든
 * 오프셋이 나중에 "근거가 이 쪽 이 자리에 있다"는 형광펜의 근거가 되기 때문이다.
 *
 * 밖에서 가져오는 것이 하나도 없다. 그래야 앱(WebView)과 검사 스크립트(node)가
 * **똑같은 코드**를 돌릴 수 있다. pdf.js 의 타입도 구조만 흉내 내어 적어 둔다.
 */

/** pdf.js `getTextContent()` 가 주는 항목 중 우리가 쓰는 부분만 */
export type TextItemLike = {
  str: string
  /** [a, b, c, d, e, f] — e, f 가 글자의 왼쪽 아래 좌표 */
  transform: number[]
  width: number
  height: number
}

/** `[문자시작, 문자끝, 항목번호]` — 항목번호는 `getTextContent().items` 안의 자리 */
export type ItemSpan = [number, number, number]

export type PageLayout = {
  text: string
  itemMap: ItemSpan[]
}

type Piece = {
  i: number
  str: string
  x: number
  y: number
  w: number
  h: number
  /** 글자가 없고 자리만 차지하는 항목인가 (표의 칸 사이 등) */
  blank: boolean
}

/** 한 줄로 묶을지 판단할 때 쓰는 세로 여유. 글자 높이에 비례한다. */
function lineTolerance(h: number): number {
  return Math.max(1.5, h * 0.5)
}

/**
 * 항목들을 화면에 보이는 차례대로 늘어놓아 쪽 텍스트를 만든다.
 *
 * pdf.js 가 주는 차례는 **PDF 안에 그려진 차례**라서 눈으로 읽는 차례와 다를 수
 * 있다 (단 나누기, 표, 머리말·꼬리말). 그래서 y 로 줄을 묶고 x 로 정렬한다.
 *
 * 이 일을 여기서 하는 까닭은, 나중에 하면 오프셋이 전부 어긋나기 때문이다.
 * **이 함수가 만든 텍스트가 최종본이고, 이후 단계는 자르기만 한다.**
 */
export function layoutPage(items: TextItemLike[]): PageLayout {
  const pieces: Piece[] = []
  for (let i = 0; i < items.length; i++) {
    const it = items[i]
    if (!it || typeof it.str !== 'string' || it.str.length === 0) continue
    const t = it.transform ?? [1, 0, 0, 1, 0, 0]
    pieces.push({
      i,
      str: it.str,
      x: t[4] ?? 0,
      y: t[5] ?? 0,
      w: it.width ?? 0,
      // transform[3] 이 실제 글자 크기다. height 가 0 으로 오는 문서가 있다.
      h: it.height || Math.abs(t[3]) || 10,
      blank: it.str.trim().length === 0,
    })
  }
  if (pieces.length === 0) return { text: '', itemMap: [] }

  // 위에서 아래로. PDF 는 y 가 클수록 위쪽이다.
  pieces.sort((a, b) => b.y - a.y || a.x - b.x)

  // y 가 비슷한 것끼리 한 줄로 묶는다
  const lines: Piece[][] = []
  let line: Piece[] = [pieces[0]]
  let lineY = pieces[0].y
  let lineH = pieces[0].h
  for (let k = 1; k < pieces.length; k++) {
    const p = pieces[k]
    if (Math.abs(p.y - lineY) <= lineTolerance(Math.max(lineH, p.h))) {
      line.push(p)
      lineH = Math.max(lineH, p.h)
    } else {
      lines.push(line)
      line = [p]
      lineY = p.y
      lineH = p.h
    }
  }
  lines.push(line)

  // 줄 안에서는 왼쪽에서 오른쪽으로
  for (const l of lines) l.sort((a, b) => a.x - b.x)

  let text = ''
  const itemMap: ItemSpan[] = []

  for (let li = 0; li < lines.length; li++) {
    if (li > 0) text += '\n'
    const l = lines[li]
    const unit = charUnit(l)

    // 자리만 차지하는 항목은 텍스트로 옮기지 않고 **사이 기호**로 바꾼다.
    // 그러지 않으면 표에서 폭 140짜리 칸 사이가 공백 한 칸으로 눌려 버려,
    // "500,000원" 이 어느 칸의 값인지 알 수 없게 된다.
    let pending = ''
    let prev: Piece | null = null

    for (const p of l) {
      if (p.blank) {
        pending = wider(pending, gapMark(p.w, unit))
        continue
      }
      if (prev) text += pending || separator(prev, p)
      const start = text.length
      text += p.str
      itemMap.push([start, text.length, p.i])
      prev = p
      pending = ''
    }
  }

  return { text, itemMap }
}

/** 이 줄에서 글자 하나가 대략 얼마나 넓은가. 사이 간격을 재는 잣대가 된다. */
function charUnit(line: Piece[]): number {
  const widths: number[] = []
  for (const p of line) {
    if (p.blank || p.w <= 0) continue
    widths.push(p.w / Math.max(1, p.str.length))
  }
  if (widths.length === 0) return 0
  widths.sort((a, b) => a - b)
  return widths[Math.floor(widths.length / 2)] // 가운뎃값
}

/** 자리만 차지하는 항목의 폭을 보고 무엇으로 바꿀지 정한다 */
function gapMark(width: number, unit: number): string {
  if (unit > 0 && width >= unit * 2) return '\t'
  return ' '
}

/** 사이 기호가 여럿 겹치면 넓은 쪽을 남긴다 */
function wider(a: string, b: string): string {
  if (a === '\t' || b === '\t') return '\t'
  return a || b
}

/**
 * 같은 줄에서 앞 항목과 뒤 항목 사이에 무엇을 끼울지 정한다.
 *
 * 사이가 많이 벌어져 있으면 탭을 넣는다. 표의 칸 경계가 대개 여기서 살아난다.
 * 표 구조를 제대로 복원하는 것은 아니고, **칸이 붙어 버리지 않게** 하는 것이
 * 목적이다. 붙어 버리면 "500,000원" 이 어느 칸의 값인지 알 수 없게 된다.
 */
function separator(prev: Piece, cur: Piece): string {
  // 이미 공백이 들어 있으면 더 넣지 않는다
  if (/\s$/.test(prev.str) || /^\s/.test(cur.str)) return ''

  const gap = cur.x - (prev.x + prev.w)
  const avg = avgCharWidth(prev, cur)
  if (avg <= 0) return ''

  if (gap >= avg * 2) return '\t'
  if (gap >= avg * 0.28) return ' '
  return ''
}

function avgCharWidth(a: Piece, b: Piece): number {
  const wa = a.w > 0 ? a.w / Math.max(1, a.str.length) : 0
  const wb = b.w > 0 ? b.w / Math.max(1, b.str.length) : 0
  const both = [wa, wb].filter((v) => v > 0)
  if (both.length === 0) return 0
  return both.reduce((s, v) => s + v, 0) / both.length
}

/** 한 쪽에서 글자를 얼마나 건졌는가 */
export type PageKind =
  /** 글자를 제대로 뽑았다 */
  | 'text'
  /** 글자가 거의 없고 그림이 있다 — 스캔한 종이 */
  | 'scanned'
  /** 글자도 그림도 없다 — 빈 쪽이거나, 글꼴 정보가 없어 못 읽은 쪽 */
  | 'empty'

/** 이 아래면 "글자를 못 건졌다"고 본다 */
const MIN_CHARS = 50

export function pageKind(text: string, hasImage: boolean): PageKind {
  if (text.replace(/\s/g, '').length >= MIN_CHARS) return 'text'
  return hasImage ? 'scanned' : 'empty'
}

/** 예전 이름. 검사 스크립트와 앱이 함께 쓴다. */
export function looksScanned(text: string, hasImage: boolean): boolean {
  return pageKind(text, hasImage) === 'scanned'
}

/** 문서 전체를 어떻게 볼 것인가 */
export type DocStatus =
  | 'ok'
  /** 스캔본. OCR 은 아직 지원하지 않는다 */
  | 'scanned'
  /** 글자를 읽지 못했다. 글꼴 정보가 없거나 보호된 문서일 수 있다 */
  | 'extract_failed'

/** 문제 있는 쪽이 이 비율을 넘으면 문서 자체에 표를 붙인다 */
const BAD_RATIO = 0.3

/**
 * 쪽별 판정을 모아 문서 상태를 정한다.
 *
 * **조용히 등록하지 않는 것이 이 함수의 목적이다.** 글자가 안 들어간 문서를
 * 멀쩡한 문서처럼 등록해 두면, 나중에 "자료에 없습니다" 라는 답이 돌아온다.
 * 그건 환각보다 나쁘다 — 사용자는 자료를 넣었다고 믿고 있기 때문이다.
 *
 * 스캔본과 "글자를 못 읽음" 을 나누는 까닭은 사용자가 할 일이 다르기 때문이다.
 * 스캔본은 OCR 을 기다려야 하지만, 글꼴 문제는 원본을 다시 저장하면 풀린다.
 */
export function documentStatus(kinds: PageKind[]): DocStatus {
  if (kinds.length === 0) return 'extract_failed'
  const bad = kinds.filter((k) => k !== 'text')
  if (bad.length / kinds.length <= BAD_RATIO) return 'ok'
  const scanned = bad.filter((k) => k === 'scanned').length
  return scanned > bad.length / 2 ? 'scanned' : 'extract_failed'
}

/** 사용자에게 보여 줄 안내. 상태마다 할 일이 다르다. */
export function statusMessage(status: DocStatus): string | null {
  switch (status) {
    case 'scanned':
      return '텍스트를 추출할 수 없는 PDF입니다. OCR 기능은 현재 지원하지 않습니다.'
    case 'extract_failed':
      return '이 PDF에서 글자를 읽지 못했습니다. 글꼴 정보가 없거나 보호된 문서일 수 있습니다. 원본을 PDF로 다시 저장한 뒤 등록해 보세요.'
    default:
      return null
  }
}

/** 검색 색인에 넣기 좋게 다듬는다. 오프셋과는 상관이 없다 (따로 저장한다). */
export function normalize(text: string): string {
  return text
    .replace(/[！-～]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)) // 전각 -> 반각
    .replace(/　/g, ' ')
    .replace(/[\t\r\n]+/g, ' ')
    .replace(/ {2,}/g, ' ')
    .trim()
}

/**
 * `itemMap` 을 되짚어, 텍스트의 [from, to) 구간이 어느 항목들에서 왔는지 찾는다.
 * P3 에서 형광펜 사각형을 그릴 때 이 함수를 쓴다.
 */
export function itemsForRange(itemMap: ItemSpan[], from: number, to: number): number[] {
  const out: number[] = []
  for (const [s, e, idx] of itemMap) {
    if (e > from && s < to) out.push(idx)
  }
  return out
}

/**
 * 그림을 그리는 명령 번호들.
 *
 * pdf.js 판마다 이름이 조금씩 바뀌므로(예전에 있던 `paintJpegXObject` 는 6판에서
 * 사라졌다), 이름 목록을 훑어 **있는 것만** 모은다. 없는 이름을 그대로 쓰면
 * `undefined` 와 견주게 되어 그림을 영영 못 찾고, 스캔본이 멀쩡한 문서로
 * 등록된다.
 */
export function imageOpCodes(OPS: Record<string, unknown>): Set<number> {
  const names = [
    'paintImageXObject',
    'paintImageXObjectRepeat',
    'paintInlineImageXObject',
    'paintInlineImageXObjectGroup',
    'paintImageMaskXObject',
    'paintImageMaskXObjectRepeat',
    'paintImageMaskXObjectGroup',
    'paintJpegXObject',
    'paintSolidColorImageMask',
  ]
  const out = new Set<number>()
  for (const n of names) {
    const v = OPS[n]
    if (typeof v === 'number') out.add(v)
  }
  return out
}
