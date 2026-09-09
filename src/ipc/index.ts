import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

/** 문서 하나의 의미 색인 상태 */
export type IndexState =
  | 'no_chunks'
  | 'none'
  | 'queued'
  | 'running'
  | 'paused'
  | 'failed'
  | 'partial'
  | 'done'
  | 'model_mismatch'
  | 'needs_reindex'

export type DocIndex = {
  documentId: number
  collectionId: number
  title: string
  total: number
  done: number
  otherModel: number
  stale: number
  state: IndexState
  /** 서버가 정한 짧은 말. 화면과 서버가 같은 말을 쓰게 한다 */
  label: string
  indexedWith: string | null
  error: string | null
}

export type EmbedModel = {
  id: string
  name: string
  tag: string
  role: 'chat' | 'embed'
  minRamGb: number
  downloadGb: number
  note: string
  dim: number
  msPerChunk: number
}

export type IndexOverview = {
  model: EmbedModel
  documents: DocIndex[]
  running: boolean
}

export type CollectionIndex = {
  collectionId: number
  documents: number
  ready: number
  needsAction: number
  summary: string
}

export type IndexEvent = {
  documentId: number
  title: string
  modelName: string
  modelTag: string
  total: number
  done: number
  percent: number
  elapsedMs: number
  remainingMs: number | null
  state: 'running' | 'paused' | 'done' | 'failed'
  error: string | null
  queued: number
}

export function indexStatus(collectionId?: number): Promise<IndexOverview> {
  return invoke<IndexOverview>('index_status', { collectionId: collectionId ?? null })
}

export function indexCollectionSummary(collectionId: number): Promise<CollectionIndex> {
  return invoke<CollectionIndex>('index_collection_summary', { collectionId })
}

export function indexModels(): Promise<{ models: EmbedModel[]; current: string }> {
  return invoke('index_models')
}

export function indexSetModel(modelId: string): Promise<void> {
  return invoke('index_set_model', { modelId })
}

/** 색인을 시작하거나 이어서 한다. `reindex` 면 쓸 수 없는 벡터를 먼저 지운다 */
export function indexStart(documentIds: number[], reindex = false): Promise<void> {
  return invoke('index_start', { documentIds, reindex })
}

/** 멈춘다. 만들어 둔 벡터는 그대로 남는다 */
export function indexStop(): Promise<boolean> {
  return invoke<boolean>('index_stop')
}

export function onIndexProgress(f: (e: IndexEvent) => void): Promise<UnlistenFn> {
  return listen<IndexEvent>('ai://index', (e) => f(e.payload))
}

/** 이 상태에서 사용자가 누를 수 있는 것 */
export function indexAction(s: IndexState): { label: string; reindex: boolean } | null {
  switch (s) {
    case 'none':
      return { label: '의미 검색 색인', reindex: false }
    case 'partial':
    case 'paused':
      return { label: '이어서 색인', reindex: false }
    case 'failed':
      return { label: '다시 색인', reindex: false }
    case 'model_mismatch':
    case 'needs_reindex':
      return { label: '현재 모델로 다시 색인', reindex: true }
    default:
      return null
  }
}

/** 남은 시간을 사람 말로. 1분 아래는 초로, 그 위는 분으로 */
export function remainingText(ms: number | null): string {
  if (ms === null || ms < 0) return ''
  const sec = Math.round(ms / 1000)
  if (sec < 60) return `약 ${Math.max(sec, 1)}초 남음`
  const min = Math.floor(sec / 60)
  const rest = sec % 60
  if (min < 10 && rest >= 10) return `약 ${min}분 ${Math.round(rest / 10) * 10}초 남음`
  return `약 ${min}분 남음`
}

export function elapsedText(ms: number): string {
  const sec = Math.round(ms / 1000)
  if (sec < 60) return `${sec}초`
  return `${Math.floor(sec / 60)}분 ${sec % 60}초`
}
