// 앱 아이콘 원본(1024x1024 PNG)을 만든다. 외부 의존성 없이 zlib 만 쓴다.
//
//   node scripts/gen-icon.mjs        ->  scripts/appicon.png
//   npx tauri icon scripts/appicon.png
//
// 그림: 청록 타일 위에 흰 문서, 그 위에 돋보기.
// "등록한 자료 안에서 찾는다" 는 이 프로그램의 일을 그대로 그린 것이다.
import { deflateSync } from 'node:zlib'
import { writeFileSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const SIZE = 1024
const here = dirname(fileURLToPath(import.meta.url))

// ---- 도형 유틸 (SDF) ----------------------------------------------------

const clamp = (v, a, b) => (v < a ? a : v > b ? b : v)
const smoothstep = (e0, e1, x) => {
  const t = clamp((x - e0) / (e1 - e0), 0, 1)
  return t * t * (3 - 2 * t)
}
const mix = (a, b, t) => a + (b - a) * t

/** 중심 기준 둥근 사각형까지의 거리 */
function sdRoundRect(px, py, halfW, halfH, r) {
  const qx = Math.abs(px) - (halfW - r)
  const qy = Math.abs(py) - (halfH - r)
  const ax = Math.max(qx, 0)
  const ay = Math.max(qy, 0)
  return Math.hypot(ax, ay) + Math.min(Math.max(qx, qy), 0) - r
}

/** 선분까지의 거리 */
function sdSegment(px, py, ax, ay, bx, by) {
  const pax = px - ax
  const pay = py - ay
  const bax = bx - ax
  const bay = by - ay
  const h = clamp((pax * bax + pay * bay) / (bax * bax + bay * bay), 0, 1)
  return Math.hypot(pax - bax * h, pay - bay * h)
}

/** 채우기: 거리<0 인 안쪽을 1 로 */
const fill = (d) => 1 - smoothstep(-1.2, 1.2, d)
/** 테두리: 거리의 절대값이 w/2 안쪽을 1 로 */
const stroke = (d, w) => 1 - smoothstep(w / 2 - 1.2, w / 2 + 1.2, Math.abs(d))

// ---- 색 -----------------------------------------------------------------

const TOP = [21, 148, 168] // 청록 (밝은 쪽)
const BOT = [12, 96, 122] // 청록 (어두운 쪽)
const PAPER = [255, 255, 255]
const LINE = [176, 205, 214] // 문서 안의 글줄
const LENS = [250, 204, 21] // 돋보기 (노랑)
const LENS_IN = [255, 247, 214] // 렌즈 안쪽

// ---- 픽셀 ---------------------------------------------------------------

const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1))

// 문서: 살짝 왼쪽 위로 치우치게
const DOC_CX = 452
const DOC_CY = 470
const DOC_HW = 208
const DOC_HH = 268

// 돋보기: 오른쪽 아래
const LENS_CX = 660
const LENS_CY = 650
const LENS_R = 150

for (let y = 0; y < SIZE; y++) {
  const rowStart = y * (SIZE * 4 + 1)
  raw[rowStart] = 0 // filter: none

  for (let x = 0; x < SIZE; x++) {
    const px = x + 0.5
    const py = y + 0.5
    const cx = px - SIZE / 2
    const cy = py - SIZE / 2

    // 바탕 타일 (여백 40, 모서리 224)
    const tile = fill(sdRoundRect(cx, cy, SIZE / 2 - 40, SIZE / 2 - 40, 224))

    const t = y / (SIZE - 1)
    let r = mix(TOP[0], BOT[0], t)
    let g = mix(TOP[1], BOT[1], t)
    let b = mix(TOP[2], BOT[2], t)

    // 문서
    const doc = fill(sdRoundRect(px - DOC_CX, py - DOC_CY, DOC_HW, DOC_HH, 26))
    r = mix(r, PAPER[0], doc)
    g = mix(g, PAPER[1], doc)
    b = mix(b, PAPER[2], doc)

    // 문서 안의 글줄 네 개 (길이가 조금씩 다르게)
    const lineYs = [
      [352, 316, 588],
      [424, 316, 588],
      [496, 316, 520],
      [568, 316, 560],
    ]
    let lines = 0
    for (const [ly, x0, x1] of lineYs) {
      lines = Math.max(lines, stroke(sdSegment(px, py, x0, ly, x1, ly), 26))
    }
    lines *= doc // 문서 밖으로 삐져나가지 않게
    r = mix(r, LINE[0], lines)
    g = mix(g, LINE[1], lines)
    b = mix(b, LINE[2], lines)

    // 돋보기 손잡이 (먼저 그려 테두리가 위로 오게)
    const handle = stroke(
      sdSegment(px, py, LENS_CX + LENS_R * 0.72, LENS_CY + LENS_R * 0.72, 830, 820),
      54,
    )
    r = mix(r, LENS[0], handle)
    g = mix(g, LENS[1], handle)
    b = mix(b, LENS[2], handle)

    // 렌즈 안쪽 (문서를 살짝 비쳐 보이게 하지 않고, 단순하게 옅은 노랑)
    const dLens = Math.hypot(px - LENS_CX, py - LENS_CY) - LENS_R
    const lensIn = fill(dLens)
    r = mix(r, LENS_IN[0], lensIn * 0.82)
    g = mix(g, LENS_IN[1], lensIn * 0.82)
    b = mix(b, LENS_IN[2], lensIn * 0.82)

    // 렌즈 테두리
    const ring = stroke(dLens, 50)
    r = mix(r, LENS[0], ring)
    g = mix(g, LENS[1], ring)
    b = mix(b, LENS[2], ring)

    const i = rowStart + 1 + x * 4
    raw[i] = Math.round(r)
    raw[i + 1] = Math.round(g)
    raw[i + 2] = Math.round(b)
    raw[i + 3] = Math.round(tile * 255)
  }
}

// ---- PNG 인코딩 ---------------------------------------------------------

function crc32(buf) {
  let c
  const table =
    crc32.table ??
    (crc32.table = (() => {
      const t = new Int32Array(256)
      for (let n = 0; n < 256; n++) {
        c = n
        for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
        t[n] = c
      }
      return t
    })())
  let crc = -1
  for (let i = 0; i < buf.length; i++) crc = (crc >>> 8) ^ table[(crc ^ buf[i]) & 0xff]
  return (crc ^ -1) >>> 0
}

function chunk(type, data) {
  const len = Buffer.alloc(4)
  len.writeUInt32BE(data.length)
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data])
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(body))
  return Buffer.concat([len, body, crc])
}

const ihdr = Buffer.alloc(13)
ihdr.writeUInt32BE(SIZE, 0)
ihdr.writeUInt32BE(SIZE, 4)
ihdr[8] = 8 // bit depth
ihdr[9] = 6 // RGBA
ihdr[10] = 0
ihdr[11] = 0
ihdr[12] = 0

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0)),
])

mkdirSync(here, { recursive: true })
const out = join(here, 'appicon.png')
writeFileSync(out, png)
console.log(`생성 완료: ${out} (${SIZE}x${SIZE}, ${(png.length / 1024).toFixed(1)}KB)`)
