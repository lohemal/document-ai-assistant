import { invoke } from '@tauri-apps/api/core'
import type { Span } from './chunks'

export type Hit = {
  /** 1부터 */
  rank: number
  chunkId: number
  documentId: number
  collectionId: number
  docTitle: string
  ord: number
  headingPath: string | null
  text: string
  kind: 'text' | 'table'
  pageStart: number
  pageEnd: number
  spans: Span[]
  /** 찾는 낱말 가운데 몇 개가 들어 있었는가 */
  matched: number
  matchedTerms: string[]
  /** SQLite BM25. 작을수록 잘 맞는다. 개발용 */
  bm25: number
}

export type SearchResult = {
  hits: Hit[]
  /** 실제로 찾아 본 낱말들 (조사를 뗀 꼴 포함) */
  terms: string[]
  /** 너무 짧아 훑어서 찾은 낱말들 */
  shortTerms: string[]
  /** 물음말이라 버린 낱말들 */
  droppedTerms: string[]
  candidates: number
  elapsedMs: number
  note: string | null
}

export function searchKeyword(
  text: string,
  collectionIds: number[],
  limit = 20,
): Promise<SearchResult> {
  return invoke<SearchResult>('search_keyword', { text, collectionIds, limit })
}

/** 고른 청크의 이웃. 검색 순위와는 상관이 없다 (P5 에서 쓴다). */
export function searchNeighbors(chunkId: number, radius = 1): Promise<number[]> {
  return invoke<number[]>('search_neighbors', { chunkId, radius })
}
