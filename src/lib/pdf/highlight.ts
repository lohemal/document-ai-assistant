/**
 * 저장해 둔 문자 위치를 원본 PDF 위의 네모로 바꾼다.
 *
 * **글자를 다시 찾지 않는다.** 같은 문장이 한 쪽에 여러 번 나오면 글자로 찾는
 * 방법은 반드시 틀린다. P2 가 저장해 둔 `[문자시작, 문자끝, 항목번호]` 를 그대로
 * 되짚어, 그 항목의 자리를 계산한다.
 */
import type { ItemSpan } from './extract'

export type HighlightRect = {
  x: number
  y: number
  w: number
  h: number
}

export type ItemGeom = {
  /** pdf.js 항목의 변환행렬 */
  transform: number[]
  /** PDF 단위 너비 */
  width: number
  str: string
}

/** [a,b,c,d,e,f] 두 개를 곱한다 (pdf.js 의 Util.transform 과 같다) */
export function mul(m1: number[], m2: number[]): number[] {
  return [
    m1[0] * m2[0] + m1[2] * m2[1],
    m1[1] * m2[0] + m1[3] * m2[1],
    m1[0] * m2[2] + m1[2] * m2[3],
    m1[1] * m2[2] + m1[3] * m2[3],
    m1[0] * m2[4] + m1[2] * m2[5] + m1[4],
    m1[1] * m2[4] + m1[3] * m2[5] + m1[5],
  ]
}

/**
 * 이 쪽에서 [from, to) 구간을 덮는 네모들.
 *
 * 구간이 항목 한가운데서 시작하거나 끝나면 그 항목의 일부만 칠한다.
 * 글자 너비가 고르다고 치고 비율로 자르는데, 완벽하지는 않아도 항목을
 * 통째로 칠하는 것보다 훨씬 낫다.
 */
export function rectsForRange(
  itemMap: ItemSpan[],
  items: ItemGeom[],
  viewportTransform: number[],
  scale: number,
  from: number,
  to: number,
): HighlightRect[] {
  const out: HighlightRect[] = []

  for (const [s, e, idx] of itemMap) {
    if (e <= from || s >= to) continue
    const item = items[idx]
    if (!item || !item.str || e === s) continue

    const tx = mul(viewportTransform, item.transform)
    // 글자 높이는 변환행렬의 세로 성분 길이다
    const height = Math.hypot(tx[2], tx[3]) || 10
    const left = tx[4]
    const top = tx[5] - height
    const width = (item.width || 0) * scale
    if (width <= 0) continue

    const len = e - s
    const cutFrom = Math.max(s, from) - s
    const cutTo = Math.min(e, to) - s

    out.push({
      x: left + (width * cutFrom) / len,
      y: top,
      w: (width * (cutTo - cutFrom)) / len,
      h: height,
    })
  }

  return merge(out)
}

/**
 * 같은 줄에서 잇닿은 네모를 하나로 합친다.
 *
 * 합치지 않으면 항목마다 네모가 하나씩 생겨, 형광펜이 얼룩덜룩해 보인다.
 */
function merge(rects: HighlightRect[]): HighlightRect[] {
  if (rects.length <= 1) return rects
  const sorted = [...rects].sort((a, b) => a.y - b.y || a.x - b.x)
  const out: HighlightRect[] = []

  for (const r of sorted) {
    const last = out[out.length - 1]
    const sameLine = last && Math.abs(last.y - r.y) <= Math.max(2, r.h * 0.3)
    const touching = last && r.x - (last.x + last.w) <= Math.max(3, r.h * 0.6)
    if (sameLine && touching) {
      const right = Math.max(last.x + last.w, r.x + r.w)
      last.x = Math.min(last.x, r.x)
      last.w = right - last.x
      last.h = Math.max(last.h, r.h)
    } else {
      out.push({ ...r })
    }
  }
  return out
}
