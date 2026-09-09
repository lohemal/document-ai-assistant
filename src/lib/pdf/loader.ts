/**
 * 앱 안에서 pdf.js 를 여는 곳. **여기가 유일한 입구다.**
 *
 * pdf.js 는 실행 중에 cMap·글꼴·wasm 을 따로 가져간다. 그 주소를 지정하지
 * 않으면 인터넷에서 받으려 하고, 우리 CSP 가 그걸 막으므로 조용히 실패한다.
 * 조용히 실패하면 옛 한글 공문 PDF 가 **글자 0자로** 들어온다.
 *
 * 그래서 문서를 여는 길을 이 함수 하나로 모아 두고, 주소를 여기서만 정한다.
 */
import * as pdfjs from 'pdfjs-dist'
import type { PDFDocumentProxy, PDFPageProxy } from 'pdfjs-dist'
import {
  imageOpCodes,
  layoutPage,
  pageKind,
  type ItemSpan,
  type PageKind,
  type TextItemLike,
} from './extract.ts'

// 워커도 앱 안에서 가져온다. CDN 을 쓰지 않는다.
pdfjs.GlobalWorkerOptions.workerPort = new Worker(
  new URL('pdfjs-dist/build/pdf.worker.mjs', import.meta.url),
  { type: 'module' },
)

/** `scripts/copy-pdfjs-assets.mjs` 가 여기에 복사해 둔다 */
const ASSETS = '/pdfjs/'

export function openPdf(data: Uint8Array): pdfjs.PDFDocumentLoadingTask {
  return pdfjs.getDocument({
    data,
    cMapUrl: ASSETS + 'cmaps/',
    cMapPacked: true,
    standardFontDataUrl: ASSETS + 'standard_fonts/',
    wasmUrl: ASSETS + 'wasm/',
    iccUrl: ASSETS + 'iccs/',
  })
}

/** 이 판으로 뽑았다고 문서에 적어 둔다. 나중에 항목 번호를 믿어도 되는지 판단한다. */
export const EXTRACTOR = `pdfjs-${pdfjs.version}`

export type ExtractedPage = {
  page: number
  label: string | null
  text: string
  itemMap: ItemSpan[]
  kind: PageKind
}

export async function extractPage(page: PDFPageProxy, label: string | null): Promise<ExtractedPage> {
  const content = await page.getTextContent()
  const { text, itemMap } = layoutPage(content.items as TextItemLike[])

  // 글자가 거의 없는 쪽만 그림을 확인한다. 모든 쪽에 하면 느리다.
  let hasImage = false
  if (text.replace(/\s/g, '').length < 50) {
    const ops = await page.getOperatorList()
    const imageOps = imageOpCodes(pdfjs.OPS as unknown as Record<string, unknown>)
    hasImage = ops.fnArray.some((fn: number) => imageOps.has(fn))
  }

  return {
    page: page.pageNumber,
    label,
    text,
    itemMap,
    kind: pageKind(text, hasImage),
  }
}

/** 쪽마다 PDF 안에 적힌 쪽 이름(있으면). 표지·목차 때문에 물리 쪽과 다를 수 있다. */
export async function pageLabels(doc: PDFDocumentProxy): Promise<(string | null)[]> {
  try {
    const labels = await doc.getPageLabels()
    return labels ?? []
  } catch {
    return []
  }
}
