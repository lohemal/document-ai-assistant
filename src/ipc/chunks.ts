import { invoke } from '@tauri-apps/api/core'
import type { ChunkIn } from '@/lib/chunk/build'

export type Span = {
  page: number
  charStart: number
  charEnd: number
}

export type Chunk = {
  id: number
  documentId: number
  ord: number
  headingPath: string | null
  text: string
  kind: 'text' | 'table'
  pageStart: number
  pageEnd: number
  spans: Span[]
}

export function saveChunks(documentId: number, chunks: ChunkIn[]): Promise<number> {
  return invoke<number>('chunk_save', { documentId, chunks })
}

export function listChunks(documentId: number): Promise<Chunk[]> {
  return invoke<Chunk[]>('chunk_list', { documentId })
}

export function countChunks(documentId: number): Promise<number> {
  return invoke<number>('chunk_count', { documentId })
}
