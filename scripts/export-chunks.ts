/**
 * 시험용 PDF 를 청크까지 나눠 JSON 으로 내보낸다.
 *
 *   npm run fixtures:chunks     ->  test/search/chunks.json
 *
 * 검색은 Rust(SQLite) 안에서 돌고, 청크는 화면 쪽(TypeScript)에서 만든다.
 * 그래서 검색 순위를 **실제 자료로** 시험하려면 둘을 이어 줄 것이 필요하다.
 * 이 파일이 그 다리다. Rust 검사가 이 JSON 을 읽어 자료를 만들고 찾아 본다.
 *
 * 자료집을 둘로 나눠 담는다 — 자료집 필터가 실제로 검색 범위를 좁히는지
 * 보려면 같은 낱말이 두 자료집에 있어야 하기 때문이다.
 */
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs'
import { layoutPage, type TextItemLike } from '../src/lib/pdf/extract.ts'
import { buildChunks } from '../src/lib/chunk/build.ts'

const root = resolve(import.meta.dirname, '..')
const pdfDir = join(root, 'test', 'pdf')
const outDir = join(root, 'test', 'search')
const pdfjsDir = join(root, 'node_modules', 'pdfjs-dist')
const asset = (n: string) => join(pdfjsDir, n) + '/'

/** 자료집 나누기 — `초등학교` 가 양쪽에 있어야 필터를 시험할 수 있다 */
const COLLECTIONS = [
  { id: 1, name: '지원금 자료', files: ['plain-ko.pdf', 'cid-ko.pdf'] },
  { id: 2, name: '늘봄 자료', files: ['table-ko.pdf', 'manual-ko.pdf', 'repeat-ko.pdf', 'odd-layer.pdf'] },
]

async function chunksOf(file: string) {
  const task = getDocument({
    data: new Uint8Array(readFileSync(join(pdfDir, file))),
    cMapUrl: asset('cmaps'),
    cMapPacked: true,
    standardFontDataUrl: asset('standard_fonts'),
  })
  const doc = await task.promise
  const pages: { page: number; text: string }[] = []
  for (let p = 1; p <= doc.numPages; p++) {
    const pg = await doc.getPage(p)
    const { text } = layoutPage((await pg.getTextContent()).items as TextItemLike[])
    pages.push({ page: p, text })
    pg.cleanup()
  }
  await task.destroy()
  return buildChunks(pages)
}

const documents: unknown[] = []
let docId = 0
let total = 0

for (const c of COLLECTIONS) {
  for (const file of c.files) {
    const chunks = await chunksOf(file)
    total += chunks.length
    documents.push({
      id: ++docId,
      collectionId: c.id,
      title: file.replace(/\.pdf$/, ''),
      filename: file,
      chunks,
    })
    console.log(`  ${file.padEnd(16)} 자료집 ${c.id} · 청크 ${chunks.length}개`)
  }
}

mkdirSync(outDir, { recursive: true })
writeFileSync(
  join(outDir, 'chunks.json'),
  JSON.stringify(
    { collections: COLLECTIONS.map(({ id, name }) => ({ id, name })), documents },
    null,
    1,
  ) + '\n',
)

console.log(`\n청크 ${total}개를 test/search/chunks.json 으로 내보냈습니다.`)
