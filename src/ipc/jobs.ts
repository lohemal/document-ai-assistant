import { invoke } from '@tauri-apps/api/core'

/** answer | limited | refuse | no_model | cancelled */
export type JobStatus = 'answer' | 'limited' | 'refuse' | 'no_model' | 'cancelled'

export type JobRow = {
  id: number
  kind: string
  question: string
  status: JobStatus
  pinned: boolean
  createdAt: string
  /** [{id, name}] — 빈 목록이면 전체 자료 */
  collectionsJson: string
  llmModel: string | null
  searchMode: string
  evidenceCount: number
}

/** same | superseded | changed | deleted — 당시 문서와 지금 문서를 견준 것 */
export type DocState = 'same' | 'superseded' | 'changed' | 'deleted'

export type JobEvidence = {
  ord: number
  sourceId: string
  documentId: number | null
  docTitle: string
  docSha256: string
  collectionName: string
  pageStart: number
  pageEnd: number
  headingPath: string | null
  quotedText: string
  chunkId: number | null
  /** 당시 형광펜 자리 [{page, charStart, charEnd}] */
  spansJson: string
  cited: boolean
  docState: DocState
  note: string | null
  /** 원문을 그 자리에 다시 열어도 되는가 */
  canOpen: boolean
}

export type JobDetail = {
  job: JobRow
  /** 당시 답변 전체 (AnswerOut JSON) */
  answerJson: string
  evidence: JobEvidence[]
  changedCount: number
}

export function listJobs(pinnedOnly: boolean, limit = 200): Promise<JobRow[]> {
  return invoke<JobRow[]>('job_list', { pinnedOnly, limit })
}

export function getJob(jobId: number): Promise<JobDetail> {
  return invoke<JobDetail>('job_get', { jobId })
}

export function pinJob(jobId: number, pinned: boolean): Promise<void> {
  return invoke('job_pin', { jobId, pinned })
}

export function deleteJob(jobId: number): Promise<void> {
  return invoke('job_delete', { jobId })
}

export function jobRetention(): Promise<number> {
  return invoke<number>('job_retention')
}

export function setJobRetention(days: number): Promise<void> {
  return invoke('job_set_retention', { days })
}

export function purgeJobs(): Promise<number> {
  return invoke<number>('job_purge')
}

export const RETENTION_CHOICES: { days: number; label: string }[] = [
  { days: 7, label: '7일' },
  { days: 30, label: '30일 (기본)' },
  { days: 90, label: '90일' },
  { days: 365, label: '1년' },
  { days: 0, label: '직접 지울 때까지' },
]

export function statusLabel(s: JobStatus): string {
  switch (s) {
    case 'answer':
      return '답변'
    case 'limited':
      return '확인되지 않은 부분 있음'
    case 'refuse':
      return '근거 부족'
    case 'no_model':
      return 'AI 답변 없음 · 근거만'
    case 'cancelled':
      return '답변 생성 중지'
  }
}

export function statusClass(s: JobStatus): string {
  switch (s) {
    case 'answer':
      return 'chip-ok'
    case 'limited':
      return 'chip-warn'
    case 'refuse':
      return 'chip-bad'
    default:
      return 'chip-off'
  }
}

/** "2026-09-10T11:35:37+09:00" → "2026-09-10 11:35" */
export function when(iso: string): string {
  const m = iso.match(/^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2})/)
  return m ? `${m[1]} ${m[2]}` : iso
}

export function collectionNames(json: string): string {
  try {
    const arr = JSON.parse(json) as { name: string }[]
    return arr.length ? arr.map((c) => c.name).join(', ') : '전체 자료'
  } catch {
    return '전체 자료'
  }
}
