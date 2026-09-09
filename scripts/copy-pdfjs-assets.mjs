/**
 * pdf.js 가 실행 중에 가져가는 자료를 앱 안으로 복사한다.
 *
 *   node scripts/copy-pdfjs-assets.mjs      ->  public/pdfjs/**
 *
 * 이걸 빠뜨리면 pdf.js 가 이 자료들을 **인터넷에서** 받으려 한다. 업무자료를
 * 다루는 프로그램에서 그런 일이 나면 안 되고(설계안 8-2), 애초에 CSP 가 막기
 * 때문에 조용히 실패한다. 조용히 실패한 결과가 무엇인지는 겪어 봤다 —
 * 옛 한글 공문 PDF 가 **글자 0자로 추출된다**.
 *
 * 무엇을 왜 담는가:
 *   cmaps          옛 한글 공문처럼 글꼴을 담지 않은 PDF 의 글자를 풀 때 (필수)
 *   standard_fonts Helvetica 같은 기본 글꼴을 대신할 글꼴
 *   wasm           스캔 PDF 에 흔한 JPEG2000 · JBIG2 그림을 풀 때
 *   iccs           색 프로파일
 */
import { cpSync, mkdirSync, rmSync, existsSync, readdirSync, statSync } from 'node:fs'
import { join, resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const src = join(root, 'node_modules', 'pdfjs-dist')
const dest = join(root, 'public', 'pdfjs')

if (!existsSync(src)) {
  console.error('pdfjs-dist 가 없습니다. 먼저 npm install 을 하세요.')
  process.exit(1)
}

const FOLDERS = ['cmaps', 'standard_fonts', 'wasm', 'iccs']

rmSync(dest, { recursive: true, force: true })
mkdirSync(dest, { recursive: true })

const size = (dir) => {
  let n = 0
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name)
    n += e.isDirectory() ? size(p) : statSync(p).size
  }
  return n
}

let total = 0
for (const folder of FOLDERS) {
  const from = join(src, folder)
  if (!existsSync(from)) {
    console.error(`pdfjs-dist 에 ${folder} 폴더가 없습니다. pdf.js 판이 바뀌었을 수 있습니다.`)
    process.exit(1)
  }
  cpSync(from, join(dest, folder), { recursive: true })
  const n = size(join(dest, folder))
  total += n
  console.log(`  ${folder.padEnd(15)} ${(n / 1024 / 1024).toFixed(2)} MB`)
}

console.log(`pdf.js 자료를 앱 안으로 복사했습니다 — 모두 ${(total / 1024 / 1024).toFixed(2)} MB`)
