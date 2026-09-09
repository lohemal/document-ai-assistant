import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type EngineState =
  /** 이 PC 에 깔려 있지 않다 */
  | 'not_installed'
  /** 깔려 있는데 켜져 있지 않다 */
  | 'not_running'
  /** 켜져 있는 것 같은데 붙지 못한다 */
  | 'unreachable'
  /** 11434 포트에 다른 프로그램이 있다 */
  | 'not_ollama'
  /** 쓸 수 있다 */
  | 'ready'

export type ModelRole = 'chat' | 'embed'

export type ModelSpec = {
  id: string
  name: string
  tag: string
  role: ModelRole
  minRamGb: number
  downloadGb: number
  note: string
}

export type InstalledModel = { tag: string; sizeBytes: number }

export type AiStatus = {
  engine: EngineState
  engineName: string
  engineVersion: string | null
  detail: string
  hint: string
  binaryPath: string | null
  hasWinget: boolean
  ramGb: number | null
  freeGb: number | null
  installed: InstalledModel[]
  chatReady: string | null
  embedReady: string | null
  recommendedChat: ModelSpec
  recommendedEmbed: ModelSpec
  recommendReason: string
  models: ModelSpec[]
  worksWithoutAi: string[]
}

export type InstallHelp = {
  downloadUrl: string
  wingetCommand: string
  hasWinget: boolean
  whyManual: string
  offlineNote: string
}

export type PullEvent = {
  modelId: string
  tag: string
  step: string
  status: string
  completedBytes: number
  totalBytes: number
  percent: number | null
  done: boolean
}

export type ModelTest = { ok: boolean; detail: string }

export function aiStatus(): Promise<AiStatus> {
  return invoke<AiStatus>('ai_status')
}

export function aiInstallHelp(): Promise<InstallHelp> {
  return invoke<InstallHelp>('ai_install_help')
}

export function aiPullModel(modelId: string): Promise<void> {
  return invoke<void>('ai_pull_model', { modelId })
}

export function aiCancelPull(): Promise<boolean> {
  return invoke<boolean>('ai_cancel_pull')
}

export function aiTestModel(modelId: string): Promise<ModelTest> {
  return invoke<ModelTest>('ai_test_model', { modelId })
}

export function onPullProgress(fn: (e: PullEvent) => void): Promise<UnlistenFn> {
  return listen<PullEvent>('ai://pull', (e) => fn(e.payload))
}

/** AI 없이도 되는 일들만 있는, 아무것도 준비되지 않은 상태 */
export const AI_UNKNOWN: AiStatus | null = null

export function gb(bytes: number): string {
  return (bytes / 1024 / 1024 / 1024).toFixed(1) + 'GB'
}

/** 이 기능을 지금 쓸 수 있는가. 못 쓰면 왜 못 쓰는지 한 줄로. */
export function blockedReason(
  status: AiStatus | null,
  need: 'keyword' | 'embed' | 'chat',
): string | null {
  if (need === 'keyword') return null // 낱말로 찾기는 언제나 된다
  if (!status) return 'AI 상태를 아직 확인하지 못했습니다.'
  if (status.engine !== 'ready') return status.detail
  if (need === 'embed' && !status.embedReady) return '검색용 AI 모델이 아직 없습니다.'
  if (need === 'chat' && !status.chatReady) return '답변용 AI 모델이 아직 없습니다.'
  return null
}
