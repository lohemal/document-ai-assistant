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
  /** 그 방법이 쓴 점수. 방법마다 단위가 다르다. 개발용 */
  score: number
  /** 낱말 검색에서 몇 등이었나. 개발용 */
  keywordRank: number | null
  /** 뜻 검색에서 몇 등이었나. 개발용 */
  semanticRank: number | null
  /** 사용자에게 보여 줄 말: 높음 | 보통 */
  relevance: string
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
  /** hybrid | keyword | semantic */
  mode: string
  /** 왜 그 방식이 되었는지 */
  modeNote: string | null
}

export function searchKeyword(
  text: string,
  collectionIds: number[],
  limit = 20,
): Promise<SearchResult> {
  return invoke<SearchResult>('search_keyword', { text, collectionIds, limit })
}

/**
 * 찾기. **방식을 고르지 않는다** — 쓸 수 있는 것을 쓴다.
 * 의미 색인이 있고 AI 가 돌면 섞어 찾기, 아니면 낱말로 찾기.
 */
export function searchQuery(
  text: string,
  collectionIds: number[],
  limit = 20,
): Promise<SearchResult> {
  return invoke<SearchResult>('search_query', { text, collectionIds, limit })
}

/** 검색 방식을 사람 말로 */
export function modeLabel(mode: string): string {
  switch (mode) {
    case 'hybrid':
      return '혼합 검색'
    case 'semantic':
      return '의미 검색'
    default:
      return '낱말 검색'
  }
}

/** 고른 청크의 이웃. 검색 순위와는 상관이 없다 (P5 에서 쓴다). */
export function searchNeighbors(chunkId: number, radius = 1): Promise<number[]> {
  return invoke<number[]>('search_neighbors', { chunkId, radius })
}
