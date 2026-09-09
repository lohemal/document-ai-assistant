/**
 * PDF 하나를 등록하는 전체 흐름.
 *
 * 화면 코드에서 떼어 놓은 까닭은, 이 흐름에 지켜야 할 순서가 있기 때문이다 —
 * 자리를 잡고 → 뽑고 → 담고 → 끝냈다고 표를 붙인다. 중간에 멈추면 그 자료는
 * `indexing` 인 채로 남고, 검색에 쓰이지 않는다.
 */
import { documentStatus, type PageKind } from './extract'
import { EXTRACTOR, extractPage, openPdf, pageLabels } from './loader'
import {
  documentBytes,
  finishDocument,
  registerDocument,
  saveDocumentPages,
  deleteDocument,
  type Document,
  type PageIn,
} from '@/ipc/documents'

export type Progress = {
  phase: '파일 읽는 중' | '글자 뽑는 중' | '갈무리하는 중'
  page: number
  total: number
}

export type RegisterOutcome =
  | { kind: 'duplicate'; existing: Document }
  | { kind: 'done'; document: Document }
  | { kind: 'cancelled' }

/** 한 번에 담는 쪽 수. 너무 잘게 담으면 느리고, 너무 크게 담으면 취소가 굼뜨다. */
const BATCH = 10

export async function registerPdf(
  collectionId: number,
  sourcePath: string,
  onProgress: (p: Progress) => void,
  isCancelled: () => boolean,
): Promise<RegisterOutcome> {
  onProgress({ phase: '파일 읽는 중', page: 0, total: 0 })

  const res = await registerDocument(collectionId, sourcePath)
  if (res.duplicateOf) return { kind: 'duplicate', existing: res.duplicateOf }
  const doc = res.document
  if (!doc) throw new Error('자료를 등록하지 못했습니다.')

  try {
    const bytes = await documentBytes(doc.id)
    const task = openPdf(bytes)
    const pdf = await task.promise
    const labels = await pageLabels(pdf)
    const total = pdf.numPages

    const kinds: PageKind[] = []
    let batch: PageIn[] = []

    for (let p = 1; p <= total; p++) {
      if (isCancelled()) {
        await task.destroy()
        await deleteDocument(doc.id)
        return { kind: 'cancelled' }
      }

      onProgress({ phase: '글자 뽑는 중', page: p, total })

      const page = await pdf.getPage(p)
      const ex = await extractPage(page, labels[p - 1] ?? null)
      page.cleanup()

      kinds.push(ex.kind)
      batch.push({
        page: ex.page,
        label: ex.label,
        text: ex.text,
        itemMap: JSON.stringify(ex.itemMap),
        kind: ex.kind,
      })

      if (batch.length >= BATCH) {
        await saveDocumentPages(doc.id, batch)
        batch = []
      }
    }

    if (batch.length > 0) await saveDocumentPages(doc.id, batch)

    onProgress({ phase: '갈무리하는 중', page: total, total })
    await task.destroy()

    const status = documentStatus(kinds)
    const finished = await finishDocument(doc.id, total, status, EXTRACTOR, res.previousId)
    return { kind: 'done', document: finished }
  } catch (e) {
    // 반쯤 등록된 자료를 남겨 두지 않는다. 남으면 "넣었는데 검색이 안 된다"가 된다.
    await deleteDocument(doc.id).catch(() => {})
    throw e
  }
}
