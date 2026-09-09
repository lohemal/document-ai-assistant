/**
 * 이미 등록해 둔 자료를 다시 나눈다.
 *
 * 나누는 규칙을 손보면 옛 자료는 옛 규칙대로 나뉜 채 남는다. 그러면 같은
 * 자료집 안에서 조각 크기가 제각각이 되어 검색 품질을 가늠할 수 없다.
 * 그래서 다시 나누는 길을 열어 둔다.
 *
 * 글자를 다시 뽑지는 않는다 — 쪽 텍스트는 이미 담겨 있고, 그 텍스트가
 * 문자 위치 지도의 기준이기 때문이다.
 */
import { documentPages } from '@/ipc/documents'
import { saveChunks } from '@/ipc/chunks'
import { buildChunks, chunkStats } from './build'

export async function rebuildChunks(documentId: number) {
  const pages = await documentPages(documentId)
  const chunks = buildChunks(pages.map((p) => ({ page: p.page, text: p.text })))
  if (chunks.length > 0) await saveChunks(documentId, chunks)
  return chunkStats(chunks)
}
