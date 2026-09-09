/**
 * 시험용 PDF 를 만든다.
 *
 *   node scripts/fixtures/make-fixtures.mjs
 *
 * 실제 학교 자료를 저장소에 넣을 수는 없으므로, 성격이 다른 PDF 를 손으로
 * 만들어 둔다. 나오는 파일은 `test/pdf/` 에 들어가고 저장소에 함께 담는다 —
 * 그래야 CI 에서도 추출 검사를 돌릴 수 있다.
 *
 * 만드는 것:
 *   plain-ko.pdf    한국어 글 위주. 제목 번호매기기(제1장/1./가./①)가 섞여 있다
 *   table-ko.pdf    표. 병합 셀도 일부러 넣었다 (못 하는 것을 보기 위해)
 *   manual-ko.pdf   60쪽짜리 긴 매뉴얼
 *   odd-layer.pdf   자간·양쪽정렬·회전·세로쓰기 — 텍스트 항목이 잘게 쪼개진다
 *   scanned.pdf     글자 레이어가 없는 스캔본 (이미지만)
 *   mixed.pdf       글자 쪽과 스캔 쪽이 섞인 문서
 *
 * 그리고 `test/pdf/expected.json` 에 "어느 파일 몇 쪽에 어떤 문장이 있는지"를
 * 함께 적는다. 추출 검사가 이걸 기준으로 맞대어 본다.
 */
import { deflateSync } from 'node:zlib'
import { execFileSync } from 'node:child_process'
import { mkdirSync, writeFileSync, rmSync, existsSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const root = resolve(here, '..', '..')
const outDir = join(root, 'test', 'pdf')
const tmpDir = join(root, 'test', '.tmp-fixtures')

mkdirSync(outDir, { recursive: true })
mkdirSync(tmpDir, { recursive: true })

/** 어느 파일 몇 쪽에 어떤 문장이 있어야 하는가 */
const expected = []
const expect = (file, page, sentence) => expected.push({ file, page, sentence })

// ─────────────────────────────────────────────────────────────────────────
// Edge 로 HTML 을 PDF 로 뽑는다
// ─────────────────────────────────────────────────────────────────────────

const EDGE = [
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
].find((p) => existsSync(p))

if (!EDGE) {
  console.error('Microsoft Edge 를 찾지 못했습니다. 이 스크립트는 Windows 전용입니다.')
  process.exit(1)
}

function htmlToPdf(name, html) {
  const htmlPath = join(tmpDir, `${name}.html`)
  const pdfPath = join(outDir, `${name}.pdf`)
  writeFileSync(htmlPath, html, 'utf8')
  execFileSync(
    EDGE,
    [
      '--headless',
      '--disable-gpu',
      '--no-first-run',
      '--no-pdf-header-footer',
      `--user-data-dir=${join(tmpDir, `profile-${name}`)}`,
      `--print-to-pdf=${pdfPath}`,
      `file:///${htmlPath.replace(/\\/g, '/')}`,
    ],
    { stdio: 'ignore', timeout: 120000 },
  )
  console.log(`  ${name}.pdf`)
}

const PAGE_CSS = `
  @page { size: A4; margin: 20mm 18mm; }
  body { font-family: 'Malgun Gothic', sans-serif; font-size: 11pt; line-height: 1.7; color: #000; }
  .pg { page-break-after: always; }
  .pg:last-child { page-break-after: auto; }
  h1 { font-size: 15pt; margin: 0 0 10pt; }
  h2 { font-size: 13pt; margin: 14pt 0 6pt; }
  h3 { font-size: 11.5pt; margin: 10pt 0 4pt; }
  p  { margin: 0 0 7pt; }
  table { border-collapse: collapse; width: 100%; margin: 8pt 0; }
  th, td { border: 1px solid #333; padding: 4pt 6pt; font-size: 10.5pt; }
  th { background: #eee; }
`

const page = (inner) => `<div class="pg">${inner}</div>`
const doc = (title, body, extraCss = '') =>
  `<!doctype html><html lang="ko"><head><meta charset="utf-8"><title>${title}</title>` +
  `<style>${PAGE_CSS}${extraCss}</style></head><body>${body}</body></html>`

/** 그 쪽에만 있는 표지 문장. 추출 검사가 이 문장을 찾는다. */
const marker = (n, code) => `이 문장은 ${n}쪽에만 있는 확인용 문장입니다. 확인코드 ${code}.`

// ─────────────────────────────────────────────────────────────────────────
// 1. 한국어 글 위주
// ─────────────────────────────────────────────────────────────────────────

{
  const codes = ['A3F7', 'B8K2', 'C1M9', 'D6P4']
  const pages = [
    `<h1>제1장 총칙</h1>
     <h2>1. 목적</h2>
     <p>이 지침은 늘봄학교 운영에 필요한 사항을 정하여 학교 현장의 업무 처리가
        일관되게 이루어지도록 하는 것을 목적으로 한다.</p>
     <h3>가. 적용 범위</h3>
     <p>이 지침은 관내 모든 초등학교에 적용한다. 다만 학교의 사정에 따라 달리
        정할 필요가 있는 경우에는 학교운영위원회의 심의를 거쳐 따로 정할 수 있다.</p>
     <p>${marker(1, codes[0])}</p>`,

    `<h1>제2장 프로그램 운영</h1>
     <h2>2. 강사 채용</h2>
     <p>① 학교의 장은 프로그램 운영에 필요한 강사를 공개 모집하여 채용한다.</p>
     <p>② 강사의 자격은 관련 분야의 자격증을 소지하거나 이에 준하는 경력이 있는
        사람으로 한다.</p>
     <p>③ 채용 결과는 학교 누리집에 7일 이상 공개하여야 한다.</p>
     <p>${marker(2, codes[1])}</p>`,

    `<h1>제3장 이용권</h1>
     <h2>3. 사용 범위</h2>
     <p>초등학교 3학년 이용권은 교재비로 사용할 수 있으며, 도서 형태의 학습교재가
        이에 해당한다. 다만 학습 준비물에 해당하는 소모품은 사용 대상에서 제외한다.</p>
     <p>이용권을 사용한 경우에는 증빙자료를 갖추어 5년간 보관한다.</p>
     <p>${marker(3, codes[2])}</p>`,

    `<h1>제4장 보칙</h1>
     <h2>4. 시행일</h2>
     <p>이 지침은 2026. 3. 1. 부터 시행한다. 제3조 및 제7조의 규정은 2026학년도
        1학기부터 적용한다.</p>
     <p>${marker(4, codes[3])}</p>`,
  ]
  htmlToPdf('plain-ko', doc('늘봄학교 운영지침', pages.map(page).join('')))
  codes.forEach((c, i) => expect('plain-ko.pdf', i + 1, marker(i + 1, c)))
  // 조사가 붙은 낱말이 실제로 붙어 나오는지도 본다
  expect('plain-ko.pdf', 3, '이용권을 사용한 경우에는')
  expect('plain-ko.pdf', 4, '2026. 3. 1. 부터 시행한다')
}

// ─────────────────────────────────────────────────────────────────────────
// 2. 표
// ─────────────────────────────────────────────────────────────────────────

{
  const pages = [
    `<h1>붙임 1. 지원 단가표</h1>
     <table>
       <tr><th>구분</th><th>대상</th><th>1인당 지원액</th><th>비고</th></tr>
       <tr><td>가형</td><td>초등학교 1~2학년</td><td>250,000원</td><td>학기당</td></tr>
       <tr><td>나형</td><td>초등학교 3학년</td><td>500,000원</td><td>학기당</td></tr>
       <tr><td>다형</td><td>초등학교 4~6학년</td><td>300,000원</td><td>학기당</td></tr>
     </table>
     <p>${marker(1, 'T100')}</p>`,

    `<h1>붙임 2. 대상 기준표 (병합 셀 포함)</h1>
     <table>
       <tr><th rowspan="2">학년</th><th colspan="2">지원 요건</th><th rowspan="2">제출 서류</th></tr>
       <tr><th>소득</th><th>거주</th></tr>
       <tr><td>3학년</td><td>중위소득 100% 이하</td><td>관내</td><td>주민등록등본</td></tr>
       <tr><td>4학년</td><td>제한 없음</td><td>관내</td><td>없음</td></tr>
     </table>
     <p>${marker(2, 'T200')}</p>`,

    `<h1>붙임 3. 추진 일정</h1>
     <table>
       <tr><th>시기</th><th>내용</th><th>담당</th></tr>
       <tr><td>2026. 2.</td><td>계획 수립 및 안내</td><td>교육청</td></tr>
       <tr><td>2026. 3.</td><td>신청 접수</td><td>학교</td></tr>
       <tr><td>2026. 4.</td><td>지원금 교부</td><td>교육청</td></tr>
     </table>
     <p>${marker(3, 'T300')}</p>`,
  ]
  htmlToPdf('table-ko', doc('지원금 운영지침 붙임', pages.map(page).join('')))
  expect('table-ko.pdf', 1, marker(1, 'T100'))
  expect('table-ko.pdf', 2, marker(2, 'T200'))
  expect('table-ko.pdf', 3, marker(3, 'T300'))
  // 표 안의 값도 그 쪽에서 나와야 한다 (구조 복원은 아직 목표가 아니다)
  expect('table-ko.pdf', 1, '500,000원')
  expect('table-ko.pdf', 2, '중위소득 100% 이하')
}

// ─────────────────────────────────────────────────────────────────────────
// 3. 긴 매뉴얼 (60쪽)
// ─────────────────────────────────────────────────────────────────────────

{
  const N = 60
  const pages = []
  for (let i = 1; i <= N; i++) {
    const code = `M${String(i).padStart(3, '0')}`
    pages.push(
      `<h1>제${i}절 운영 세부사항</h1>
       <p>학교의 장은 프로그램 운영에 관한 사항을 매 학기 초에 정하여 안내한다.
          운영 시간과 장소는 학교 여건을 고려하여 조정할 수 있다.</p>
       <p>${marker(i, code)}</p>
       <p>세부 사항은 관련 부서와 협의하여 정하며, 협의 결과는 문서로 남긴다.</p>`,
    )
    // 앞·중간·끝만 검사한다. 60개를 다 넣으면 검사 결과가 읽기 어려워진다.
    if (i === 1 || i === 30 || i === 60) expect('manual-ko.pdf', i, marker(i, code))
  }
  htmlToPdf('manual-ko', doc('늘봄학교 운영 매뉴얼', pages.map(page).join('')))
}

// ─────────────────────────────────────────────────────────────────────────
// 4. 텍스트 레이어가 특이한 문서
// ─────────────────────────────────────────────────────────────────────────

{
  const css = `
    .spaced { letter-spacing: 4px; }
    .just { text-align: justify; text-justify: inter-character; }
    .rot { transform: rotate(-12deg); transform-origin: left top; margin: 24pt 0 40pt; }
    .vert { writing-mode: vertical-rl; height: 180pt; }
    .tiny { font-size: 7pt; }
  `
  const pages = [
    `<h1>자간이 벌어진 글</h1>
     <p class="spaced">자간이 넓은 문장은 글자마다 항목이 쪼개지기 쉽습니다.</p>
     <p class="just">양쪽 정렬된 긴 문단은 낱말 사이 간격이 늘어나므로 항목 경계가
        본래 낱말 경계와 달라질 수 있습니다. 이 문단은 그 상황을 만들기 위해
        일부러 길게 썼습니다. 추출 결과에서 낱말이 붙거나 떨어지는지 봅니다.</p>
     <p>${marker(1, 'X1AA')}</p>`,

    `<h1>기울어진 글과 세로쓰기</h1>
     <p class="rot">기울어진 글도 같은 쪽에서 나와야 합니다.</p>
     <div class="vert">세로로 쓴 글입니다. 줄 재구성이 무너지는지 봅니다.</div>
     <p class="tiny">아주 작은 글씨로 쓴 각주입니다.</p>
     <p>${marker(2, 'X2BB')}</p>`,
  ]
  htmlToPdf('odd-layer', doc('특이한 텍스트 레이어', pages.map(page).join(''), css))
  expect('odd-layer.pdf', 1, marker(1, 'X1AA'))
  expect('odd-layer.pdf', 2, marker(2, 'X2BB'))
}

// ─────────────────────────────────────────────────────────────────────────
// 5. 스캔본 — 손으로 만든다 (글자 레이어가 아예 없어야 하므로)
// ─────────────────────────────────────────────────────────────────────────

/** 종이를 스캔한 것처럼 보이는 회색 줄무늬 이미지 */
function scanLikeImage(w, h, seed) {
  const buf = Buffer.alloc(w * h * 3, 0xf4)
  let s = seed
  const rnd = () => ((s = (s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff)

  // 글줄처럼 보이는 검은 막대를 늘어놓는다
  let y = Math.floor(h * 0.08)
  while (y < h * 0.92) {
    const lineH = 6 + Math.floor(rnd() * 3)
    const x0 = Math.floor(w * 0.1)
    const x1 = x0 + Math.floor((w * 0.8) * (0.5 + rnd() * 0.5))
    for (let yy = y; yy < y + lineH && yy < h; yy++) {
      for (let xx = x0; xx < x1 && xx < w; xx++) {
        // 낱말 사이를 띄운다
        if (Math.floor(xx / 7) % 6 === 5) continue
        const i = (yy * w + xx) * 3
        const v = 40 + Math.floor(rnd() * 40)
        buf[i] = v
        buf[i + 1] = v
        buf[i + 2] = v
      }
    }
    y += lineH + 10 + Math.floor(rnd() * 6)
  }
  return buf
}

/** 최소한의 PDF 를 손으로 짠다. 객체 번호와 xref 오프셋만 맞추면 된다. */
function buildPdf(pages) {
  const chunks = []
  let len = 0
  const push = (b) => {
    const buf = Buffer.isBuffer(b) ? b : Buffer.from(b, 'binary')
    chunks.push(buf)
    len += buf.length
    return len
  }

  const offsets = [] // 객체 번호 -> 오프셋
  const obj = (num, body) => {
    offsets[num] = len
    push(`${num} 0 obj\n`)
    if (Buffer.isBuffer(body)) push(body)
    else push(body)
    push('\nendobj\n')
  }

  push('%PDF-1.4\n%\xe2\xe3\xcf\xd3\n')

  const W = 595
  const H = 842
  // 1 = Catalog, 2 = Pages, 3.. = 각 페이지가 쓰는 객체들
  const pageObjNums = []
  let next = 3
  const later = []

  for (const p of pages) {
    const pageNum = next++
    const contentNum = next++
    const imgNum = p.kind === 'image' ? next++ : null
    const fontNum = p.kind === 'text' || p.kind === 'cid' ? next++ : null
    // CID 글꼴은 자손 글꼴과 글꼴 서술자가 더 필요하다
    const descNum = p.kind === 'cid' ? next++ : null
    const fdNum = p.kind === 'cid' ? next++ : null
    pageObjNums.push(pageNum)
    later.push({ p, pageNum, contentNum, imgNum, fontNum, descNum, fdNum })
  }

  obj(1, `<< /Type /Catalog /Pages 2 0 R >>`)
  obj(
    2,
    `<< /Type /Pages /Kids [${pageObjNums.map((n) => `${n} 0 R`).join(' ')}] /Count ${pageObjNums.length} >>`,
  )

  for (const { p, pageNum, contentNum, imgNum, fontNum, descNum, fdNum } of later) {
    const res =
      p.kind === 'image'
        ? `<< /XObject << /Im0 ${imgNum} 0 R >> >>`
        : `<< /Font << /F1 ${fontNum} 0 R >> >>`
    obj(
      pageNum,
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${W} ${H}] /Resources ${res} /Contents ${contentNum} 0 R >>`,
    )

    let content
    if (p.kind === 'image') {
      content = `q ${W} 0 0 ${H} 0 0 cm /Im0 Do Q`
    } else if (p.kind === 'cid') {
      // UniKS-UCS2-H 는 2바이트 UCS-2 를 CID 로 옮긴다. 그러니 글자를
      // UTF-16BE 로 적어 넣으면 된다. 글꼴은 넣지 않는다 — 옛 한글 공문
      // PDF 가 딱 이 모양이라, pdf.js 가 cMap 을 읽어야만 글자가 나온다.
      const lines = p.lines
        .map((t, i) => {
          const hex = Buffer.from(t, 'utf16le').swap16().toString('hex')
          return `BT /F1 14 Tf 72 ${H - 100 - i * 28} Td <${hex}> Tj ET`
        })
        .join('\n')
      content = lines
    } else {
      content = `BT /F1 14 Tf 72 ${H - 100} Td (${p.text}) Tj ET`
    }
    const cbuf = Buffer.from(content, 'latin1')
    offsets[contentNum] = len
    push(`${contentNum} 0 obj\n<< /Length ${cbuf.length} >>\nstream\n`)
    push(cbuf)
    push('\nendstream\nendobj\n')

    if (p.kind === 'image') {
      const iw = 600
      const ih = 850
      const raw = scanLikeImage(iw, ih, p.seed ?? 7)
      const z = deflateSync(raw, { level: 9 })
      offsets[imgNum] = len
      push(
        `${imgNum} 0 obj\n<< /Type /XObject /Subtype /Image /Width ${iw} /Height ${ih} ` +
          `/ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length ${z.length} >>\nstream\n`,
      )
      push(z)
      push('\nendstream\nendobj\n')
    } else if (p.kind === 'cid') {
      obj(
        fontNum,
        `<< /Type /Font /Subtype /Type0 /BaseFont /HYSMyeongJo-Medium ` +
          `/Encoding /UniKS-UCS2-H /DescendantFonts [${descNum} 0 R] >>`,
      )
      obj(
        descNum,
        `<< /Type /Font /Subtype /CIDFontType0 /BaseFont /HYSMyeongJo-Medium ` +
          `/CIDSystemInfo << /Registry (Adobe) /Ordering (Korea1) /Supplement 1 >> ` +
          `/FontDescriptor ${fdNum} 0 R /DW 1000 >>`,
      )
      obj(
        fdNum,
        `<< /Type /FontDescriptor /FontName /HYSMyeongJo-Medium /Flags 6 ` +
          `/FontBBox [-28 -148 1001 880] /ItalicAngle 0 /Ascent 880 /Descent -148 ` +
          `/CapHeight 720 /StemV 60 >>`,
      )
    } else {
      obj(fontNum, `<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>`)
    }
  }

  const total = next - 1
  const xrefAt = len
  let xref = `xref\n0 ${total + 1}\n0000000000 65535 f \n`
  for (let i = 1; i <= total; i++) {
    xref += String(offsets[i] ?? 0).padStart(10, '0') + ' 00000 n \n'
  }
  push(xref)
  push(`trailer\n<< /Size ${total + 1} /Root 1 0 R >>\nstartxref\n${xrefAt}\n%%EOF\n`)

  return Buffer.concat(chunks)
}

writeFileSync(
  join(outDir, 'scanned.pdf'),
  buildPdf([
    { kind: 'image', seed: 11 },
    { kind: 'image', seed: 23 },
  ]),
)
console.log('  scanned.pdf')

// 글자 쪽 1 + 스캔 쪽 3 = 스캔 비율 75%
writeFileSync(
  join(outDir, 'mixed.pdf'),
  buildPdf([
    { kind: 'text', text: 'This page has a real text layer. MIXEDPAGE1' },
    { kind: 'image', seed: 31 },
    { kind: 'image', seed: 41 },
    { kind: 'image', seed: 53 },
  ]),
)
console.log('  mixed.pdf')
expect('mixed.pdf', 1, 'MIXEDPAGE1')

// 글꼴을 넣지 않은 CID 한글 PDF — 옛 한글 공문이 이 모양이다.
// pdf.js 가 cMap(UniKS-UCS2-H, Adobe-Korea1-UCS2)을 읽어야만 글자가 나오므로,
// **cMap 을 앱에 제대로 번들했는지**를 이 파일 하나로 확인할 수 있다.
{
  const p1 = [
    '○○교육지원청 공문 알림',
    '수신: 관내 초등학교장',
    '제목: 2026학년도 자유수강권 운영지침 알림',
    '자유수강권 운영지침을 붙임과 같이 알려 드립니다.',
    '각 학교에서는 지침에 따라 대상자를 선정하고 결과를 보고하여 주시기 바랍니다.',
    '제출 기한은 2026. 3. 20. 까지이며, 기한을 넘기면 예산 배정에서 제외될 수 있습니다.',
    '문의: 교육지원청 학교지원과',
    '확인코드 CID1.',
  ]
  const p2 = [
    '붙임. 자유수강권 운영지침',
    '제7조(지원 범위)',
    '1인당 100,000원 이내로 지원한다.',
    '지원 대상은 중위소득 이하 가정의 학생으로 하며, 학교장이 최종 확정한다.',
    '지원금은 방과후학교 수강료로만 사용할 수 있고, 교재비는 별도로 정한다.',
    '집행 잔액이 발생한 경우에는 학기 종료 후 반납하여야 한다.',
    '확인코드 CID2.',
  ]
  writeFileSync(
    join(outDir, 'cid-ko.pdf'),
    buildPdf([
      { kind: 'cid', lines: p1 },
      { kind: 'cid', lines: p2 },
    ]),
  )
  console.log('  cid-ko.pdf')
  expect('cid-ko.pdf', 1, '자유수강권 운영지침을 붙임과 같이 알려 드립니다.')
  expect('cid-ko.pdf', 2, '1인당 100,000원 이내로 지원한다.')
}

// ─────────────────────────────────────────────────────────────────────────

writeFileSync(join(outDir, 'expected.json'), JSON.stringify(expected, null, 2) + '\n')
rmSync(tmpDir, { recursive: true, force: true })

console.log(`\n시험용 PDF ${6}개와 기대값 ${expected.length}건을 만들었습니다.`)
console.log(`  ${outDir}`)
