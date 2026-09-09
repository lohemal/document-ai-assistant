/**
 * 시험용 PDF 한 쪽이 어떻게 추출되는지 눈으로 본다. 개발용 도구다.
 *
 *   node --experimental-strip-types scripts/dump-page.ts table-ko.pdf 1
 *   node --experimental-strip-types scripts/dump-page.ts table-ko.pdf 1 items
 *
 * `items` 를 붙이면 pdf.js 가 준 항목의 좌표까지 늘어놓는다. 줄이 왜 그렇게
 * 묶였는지, 칸 사이가 왜 안 벌어졌는지 따질 때 쓴다.
 */
import { readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs'
import { layoutPage, type TextItemLike } from '../src/lib/pdf/extract.ts'

const root = resolve(import.meta.dirname, '..')
const file = process.argv[2] ?? 'plain-ko.pdf'
const pageNo = Number(process.argv[3] ?? 1)
const showItems = process.argv[4] === 'items'

const pdfjsDir = join(root, 'node_modules', 'pdfjs-dist')
const task = getDocument({
  data: new Uint8Array(readFileSync(join(root, 'test', 'pdf', file))),
  cMapUrl: pathToFileURL(join(pdfjsDir, 'cmaps') + '/').href,
  cMapPacked: true,
  standardFontDataUrl: pathToFileURL(join(pdfjsDir, 'standard_fonts') + '/').href,
})
const doc = await task.promise
const page = await doc.getPage(pageNo)
const content = await page.getTextContent()
const items = content.items as TextItemLike[]

if (showItems) {
  items.forEach((it, i) => {
    if (!it.str) return
    console.log(
      `${String(i).padStart(3)} x=${(it.transform[4] ?? 0).toFixed(1).padStart(7)} ` +
        `y=${(it.transform[5] ?? 0).toFixed(1).padStart(7)} w=${(it.width ?? 0).toFixed(1).padStart(6)} ` +
        `h=${(it.height ?? 0).toFixed(1).padStart(5)}  ${JSON.stringify(it.str)}`,
    )
  })
  console.log('---')
}

const { text, itemMap } = layoutPage(items)
console.log(text.replace(/\t/g, ' ⇥ '))
console.log(`\n(항목 ${itemMap.length}개, 글자 ${text.length}자)`)
await task.destroy()
