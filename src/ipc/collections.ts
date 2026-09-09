import { invoke } from '@tauri-apps/api/core'

export type Collection = {
  id: number
  name: string
  /** 이 자료집에 든 문서 수 */
  docCount: number
  /** 그 중 임베딩이 아직 없는 문서 수 */
  embedPending: number
  /** 이 자료집을 색인할 때 쓴 임베딩 모델. 아직 없으면 null */
  embedModel: string | null
  createdAt: string
}

export function listCollections(): Promise<Collection[]> {
  return invoke<Collection[]>('collection_list')
}

export function createCollection(name: string): Promise<number> {
  return invoke<number>('collection_create', { name })
}

export function renameCollection(id: number, name: string): Promise<void> {
  return invoke<void>('collection_rename', { id, name })
}

export function deleteCollection(id: number): Promise<void> {
  return invoke<void>('collection_delete', { id })
}
