import { invoke } from '@tauri-apps/api/core'
import type { ItemSpan } from '@/lib/pdf/extract'

export type DocStatusValue = 'ok' | 'scanned' | 'extract_failed' | 'indexing'

export type Document = {
  id: number
  collectionId: number
  title: string
  filename: string
  sha256: string
  byteSize: number
  pageCount: number
  status: DocStatusValue
  embedState: 'none' | 'partial' | 'done'
  extractor: string | null
  /** 글자를 못 건진 쪽 수 */
  blankPages: number
  createdAt: string
}

export type PageOut = {
  page: number
  label: string | null
  text: string
  /** JSON 문자열. `parseItemMap` 으로 푼다 */
  itemMap: string
  isScanned: boolean
}

export type RegisterResult = {
  duplicateOf: Document | null
  document: Document | null
  previousId: number | null
}

export type PageIn = {
  page: number
  label: string | null
  text: string
  itemMap: string
  kind: string
}

export function registerDocument(collectionId: number, sourcePath: string): Promise<RegisterResult> {
  return invoke<RegisterResult>('document_register', { collectionId, sourcePath })
}

/** 등록해 둔 PDF 를 날바이트로 받는다 */
export async function documentBytes(documentId: number): Promise<Uint8Array> {
  const got = await invoke<ArrayBuffer | number[]>('document_bytes', { documentId })
  return got instanceof ArrayBuffer ? new Uint8Array(got) : Uint8Array.from(got)
}

export function saveDocumentPages(documentId: number, pages: PageIn[]): Promise<void> {
  return invoke<void>('document_save_pages', { documentId, pages })
}

export function finishDocument(
  documentId: number,
  pageCount: number,
  status: string,
  extractor: string,
  previousId: number | null,
): Promise<Document> {
  return invoke<Document>('document_finish', {
    documentId,
    pageCount,
    status,
    extractor,
    previousId,
  })
}

export function listDocuments(collectionId: number): Promise<Document[]> {
  return invoke<Document[]>('document_list', { collectionId })
}

export function documentPages(documentId: number): Promise<PageOut[]> {
  return invoke<PageOut[]>('document_pages', { documentId })
}

export function deleteDocument(documentId: number): Promise<void> {
  return invoke<void>('document_delete', { documentId })
}

export function parseItemMap(json: string): ItemSpan[] {
  try {
    return JSON.parse(json) as ItemSpan[]
  } catch {
    return []
  }
}
