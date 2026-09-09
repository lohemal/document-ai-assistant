import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { Span } from './chunks'

export type Decision = 'answer' | 'limited' | 'refuse' | 'no_model'

export type Evidence = {
  sourceId: string
  documentId: number
  docTitle: string
  chunkId: number
  ord: number
  pageStart: number
  pageEnd: number
  headingPath: string | null
  text: string
  spans: Span[]
  /** 검색이 고른 것인가(false), 이웃으로 딸려 온 것인가(true) */
  neighbor: boolean
  keywordRank: number | null
  semanticRank: number | null
  searchRank: number | null
}

export type Claim = {
  text: string
  sources: string[]
  kind: 'fact' | 'interpretation'
}

export type SourceCheck = {
  sourceId: string
  known: boolean
  chunkId: number | null
  documentId: number | null
  page: number | null
}

export type NumberCheck = {
  raw: string
  kind: string
  found: boolean
  sourceId: string | null
}

export type ClaimCheck = {
  text: string
  sources: string[]
  /** 해석에는 뒷받침 검사를 걸지 않는다 — 근거의 말을 옮긴 것이 아니므로 */
  kind: 'fact' | 'interpretation'
  /** 주장의 낱말 가운데 인용한 근거에도 있는 것의 비율 */
  overlap: number
  supported: boolean
  /** 인용을 고쳐 붙였으면, AI 가 원래 적었던 근거 이름 */
  repairedFrom: string | null
}

export type Verdict = {
  claims: ClaimCheck[]
  sources: SourceCheck[]
  unknownSources: string[]
  claimsWithoutSource: string[]
  numbers: NumberCheck[]
  citationsOk: boolean
  /** 내용이 실제로 있는 근거로 바로잡은 인용 ("근거3 → 근거5") */
  repaired: string[]
  hasInterpretation: boolean
  citationMessage: string
  numberMessage: string
}

export type AnswerOut = {
  question: string
  decision: Decision
  answer: string
  claims: Claim[]
  evidence: Evidence[]
  cited: string[]
  verdict: Verdict | null
  judgement: { decision: Decision; reasons: string[] }
  interpretation: boolean
  interpretationNotes: string[]
  /** 모델이 스스로 적은 자신감. 판단에는 쓰지 않는다 (개발용) */
  confidence: string | null
  search: {
    mode: string
    modeNote: string | null
    hits: number
    elapsedMs: number
    terms: string[]
  }
  model: string | null
  modelNote: string | null
  tokens: number
  llmMs: number
  totalMs: number
  cancelled: boolean
  raw: string | null
}

export type AnswerEvent = {
  phase: 'searching' | 'reading' | 'writing' | 'verifying'
  note: string
  chars: number
}

export function ask(text: string, collectionIds: number[]): Promise<AnswerOut> {
  return invoke<AnswerOut>('answer_ask', { text, collectionIds })
}

export function cancelAsk(): Promise<boolean> {
  return invoke<boolean>('answer_cancel')
}

export function onAnswerProgress(f: (e: AnswerEvent) => void): Promise<UnlistenFn> {
  return listen<AnswerEvent>('ai://answer', (e) => f(e.payload))
}

/** 판단을 사람 말로. 점수가 아니라 **무엇을 확인했는지**로 말한다. */
export function decisionLabel(d: Decision): string {
  switch (d) {
    case 'answer':
      return '근거 확인'
    case 'limited':
      return '확인되지 않은 부분 있음'
    case 'refuse':
      return '근거 부족'
    case 'no_model':
      return 'AI 답변 없음'
  }
}
