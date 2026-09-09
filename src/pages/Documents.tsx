import { useCallback, useEffect, useRef, useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { listCollections, type Collection } from '@/ipc/collections'
import {
  deleteDocument,
  documentPages,
  listDocuments,
  type Document,
  type PageOut,
} from '@/ipc/documents'
import { registerPdf, type Progress } from '@/lib/pdf/register'
import { statusMessage, type DocStatus } from '@/lib/pdf/extract'
import { message } from '@/lib/err'
import PageTextView from '@/components/PageTextView'

const STATUS_LABEL: Record<string, string> = {
  ok: '정상',
  scanned: '스캔본',
  extract_failed: '글자 못 읽음',
  indexing: '등록 중',
}

export default function Documents() {
  const [collections, setCollections] = useState<Collection[] | null>(null)
  const [collectionId, setCollectionId] = useState<number | null>(null)
  const [docs, setDocs] = useState<Document[]>([])
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [progress, setProgress] = useState<Progress | null>(null)
  const cancelled = useRef(false)

  // 글자 보기
  const [openDoc, setOpenDoc] = useState<Document | null>(null)
  const [pages, setPages] = useState<PageOut[] | null>(null)
  const [pageNo, setPageNo] = useState(1)

  useEffect(() => {
    listCollections()
      .then((cs) => {
        setCollections(cs)
        if (cs.length > 0) setCollectionId((prev) => prev ?? cs[0].id)
      })
      .catch((e) => setError(message(e)))
  }, [])

  const reload = useCallback(async (id: number) => {
    try {
      setDocs(await listDocuments(id))
      setError(null)
    } catch (e) {
      setError(message(e))
    }
  }, [])

  useEffect(() => {
    if (collectionId !== null) void reload(collectionId)
  }, [collectionId, reload])

  async function onRegister() {
    if (collectionId === null) return
    setError(null)
    setNotice(null)

    const picked = await open({
      multiple: false,
      filters: [{ name: 'PDF 문서', extensions: ['pdf'] }],
    })
    if (typeof picked !== 'string') return

    cancelled.current = false
    try {
      const out = await registerPdf(
        collectionId,
        picked,
        setProgress,
        () => cancelled.current,
      )
      if (out.kind === 'duplicate') {
        setNotice(`같은 내용의 자료가 이미 있습니다 — ${out.existing.title}`)
      } else if (out.kind === 'cancelled') {
        setNotice('등록을 멈췄습니다.')
      } else {
        const msg = statusMessage(out.document.status as DocStatus)
        setNotice(msg ? `${out.document.title} — ${msg}` : `${out.document.title} 을(를) 등록했습니다.`)
      }
    } catch (e) {
      setError(message(e))
    } finally {
      setProgress(null)
      await reload(collectionId)
    }
  }

  async function onOpenText(doc: Document) {
    setOpenDoc(doc)
    setPages(null)
    setPageNo(1)
    try {
      setPages(await documentPages(doc.id))
    } catch (e) {
      setError(message(e))
    }
  }

  async function onDelete(doc: Document) {
    try {
      await deleteDocument(doc.id)
      if (openDoc?.id === doc.id) setOpenDoc(null)
      if (collectionId !== null) await reload(collectionId)
    } catch (e) {
      setError(message(e))
    }
  }

  if (collections !== null && collections.length === 0) {
    return (
      <div className="page">
        <h1 className="page-title">자료 등록</h1>
        <div className="empty">
          <p>먼저 자료집을 만들어 주세요.</p>
          <p className="muted">자료는 자료집 안에 들어갑니다.</p>
        </div>
      </div>
    )
  }

  const current = pages?.find((p) => p.page === pageNo)

  return (
    <div className="page page-wide">
      <h1 className="page-title">자료 등록</h1>
      <p className="page-lead">
        PDF 를 등록하면 쪽마다 글자를 뽑아 둡니다. <strong>AI 모델이 없어도 됩니다.</strong>
      </p>

      <div className="row-form">
        <label className="field-label" htmlFor="col">
          자료집
        </label>
        <select
          id="col"
          className="input"
          value={collectionId ?? ''}
          onChange={(e) => setCollectionId(Number(e.target.value))}
          disabled={progress !== null}
        >
          {collections?.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
        <button className="btn btn-primary" onClick={() => void onRegister()} disabled={progress !== null}>
          PDF 등록
        </button>
      </div>

      {progress && (
        <div className="progress">
          <div className="progress-line">
            {progress.phase}
            {progress.total > 0 && ` — ${progress.page} / ${progress.total}쪽`}
          </div>
          <div className="progress-bar">
            <div
              className="progress-fill"
              style={{ width: progress.total ? `${(progress.page / progress.total) * 100}%` : '10%' }}
            />
          </div>
          <button className="btn" onClick={() => (cancelled.current = true)}>
            멈추기
          </button>
        </div>
      )}

      {error && <p className="banner banner-error">{error}</p>}
      {notice && <p className="banner banner-info">{notice}</p>}

      {docs.length === 0 && !progress && (
        <div className="empty">
          <p>이 자료집에는 아직 자료가 없습니다.</p>
        </div>
      )}

      {docs.length > 0 && (
        <ul className="list">
          {docs.map((d) => (
            <li className="list-item" key={d.id}>
              <div className="list-main">
                <div className="list-name">{d.title}</div>
                <div className="list-sub">
                  <span>{d.pageCount}쪽</span>
                  <span className={'tag tag-' + (d.status === 'ok' ? 'ok' : 'warn')}>
                    {STATUS_LABEL[d.status] ?? d.status}
                  </span>
                  {d.status === 'ok' && d.blankPages > 0 && (
                    <span className="muted">글자 없는 쪽 {d.blankPages}개</span>
                  )}
                </div>
                {statusMessage(d.status as DocStatus) && (
                  <div className="list-warn">⚠ {statusMessage(d.status as DocStatus)}</div>
                )}
              </div>
              <div className="list-actions">
                <button className="btn" onClick={() => void onOpenText(d)}>
                  글자 보기
                </button>
                <button className="btn btn-quiet" onClick={() => void onDelete(d)}>
                  지우기
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {openDoc && (
        <section className="card viewer">
          <div className="viewer-head">
            <h2 className="card-title">{openDoc.title} — 뽑은 글자</h2>
            <button className="btn btn-quiet" onClick={() => setOpenDoc(null)}>
              닫기
            </button>
          </div>

          {pages === null && <p className="muted">불러오는 중…</p>}

          {pages && pages.length > 0 && (
            <>
              <div className="row-form">
                <label className="field-label" htmlFor="pg">
                  쪽
                </label>
                <select
                  id="pg"
                  className="input input-narrow"
                  value={pageNo}
                  onChange={(e) => setPageNo(Number(e.target.value))}
                >
                  {pages.map((p) => (
                    <option key={p.page} value={p.page}>
                      {p.page}쪽{p.isScanned ? ' (스캔)' : ''}
                      {p.text.trim().length === 0 && !p.isScanned ? ' (빈 쪽)' : ''}
                    </option>
                  ))}
                </select>
                <span className="muted">
                  {openDoc.extractor && `추출: ${openDoc.extractor}`}
                </span>
              </div>
              {current && <PageTextView page={current} />}
            </>
          )}
        </section>
      )}
    </div>
  )
}
