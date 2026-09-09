import { useCallback, useEffect, useState } from 'react'
import {
  createCollection,
  deleteCollection,
  listCollections,
  renameCollection,
  type Collection,
} from '@/ipc/collections'
import { message } from '@/lib/err'
import { indexCollectionSummary, type CollectionIndex } from '@/ipc/index'

export default function Collections() {
  const [items, setItems] = useState<Collection[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [newName, setNewName] = useState('')
  const [busy, setBusy] = useState(false)

  /** 이름을 고치는 중인 자료집 */
  const [editing, setEditing] = useState<{ id: number; name: string } | null>(null)
  /** 지울지 다시 묻는 중인 자료집 */
  const [confirming, setConfirming] = useState<number | null>(null)
  /** 자료집마다 의미 검색이 얼마나 준비됐는가 */
  const [ready, setReady] = useState<Record<number, CollectionIndex>>({})

  const reload = useCallback(async () => {
    try {
      setItems(await listCollections())
      setError(null)
    } catch (e) {
      setError(message(e))
    }
  }, [])

  useEffect(() => {
    void reload()
  }, [reload])

  // 자료집마다 의미 검색 준비 상태를 읽는다. 실패해도 화면은 그대로 돈다 —
  // 이건 곁들이는 정보라, 없으면 안 보이면 된다.
  useEffect(() => {
    if (!items) return
    for (const c of items) {
      if (c.docCount === 0) continue
      indexCollectionSummary(c.id)
        .then((r) => setReady((prev) => ({ ...prev, [c.id]: r })))
        .catch(() => {})
    }
  }, [items])

  /** 한 가지 일을 하고 목록을 다시 읽는다. 실패하면 이유를 남긴다. */
  async function run(work: () => Promise<unknown>) {
    setBusy(true)
    try {
      await work()
      setError(null)
      await reload()
      return true
    } catch (e) {
      setError(message(e))
      return false
    } finally {
      setBusy(false)
    }
  }

  async function onCreate(e: React.FormEvent) {
    e.preventDefault()
    if (!newName.trim()) return
    if (await run(() => createCollection(newName))) setNewName('')
  }

  async function onRename() {
    if (!editing) return
    if (await run(() => renameCollection(editing.id, editing.name))) setEditing(null)
  }

  async function onDelete(id: number) {
    if (await run(() => deleteCollection(id))) setConfirming(null)
  }

  return (
    <div className="page">
      <h1 className="page-title">자료집 관리</h1>
      <p className="page-lead">
        업무자료를 주제별로 묶습니다. 질문할 때 자료집을 골라 검색 범위를 좁히면 더 정확한
        근거를 찾습니다.
      </p>

      <form className="row-form" onSubmit={onCreate}>
        <input
          className="input"
          placeholder="새 자료집 이름 (예: 늘봄학교)"
          value={newName}
          maxLength={40}
          onChange={(e) => setNewName(e.target.value)}
        />
        <button className="btn btn-primary" disabled={busy || !newName.trim()}>
          자료집 만들기
        </button>
      </form>

      {error && <p className="banner banner-error">{error}</p>}

      {items === null && <p className="muted">불러오는 중…</p>}

      {items !== null && items.length === 0 && (
        <div className="empty">
          <p>아직 자료집이 없습니다.</p>
          <p className="muted">
            위에서 자료집을 하나 만든 뒤 PDF 를 등록하면 검색할 수 있습니다.
          </p>
        </div>
      )}

      {items !== null && items.length > 0 && (
        <ul className="list">
          {items.map((c) => (
            <li className="list-item" key={c.id}>
              {editing?.id === c.id ? (
                <div className="row-form">
                  <input
                    className="input"
                    autoFocus
                    value={editing.name}
                    maxLength={40}
                    onChange={(e) => setEditing({ id: c.id, name: e.target.value })}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') void onRename()
                      if (e.key === 'Escape') setEditing(null)
                    }}
                  />
                  <button className="btn btn-primary" disabled={busy} onClick={() => void onRename()}>
                    바꾸기
                  </button>
                  <button className="btn" onClick={() => setEditing(null)}>
                    취소
                  </button>
                </div>
              ) : (
                <>
                  <div className="list-main">
                    <div className="list-name">{c.name}</div>
                    <div className="list-sub">
                      {c.docCount === 0 ? (
                        <span className="muted">자료 없음</span>
                      ) : (
                        <>
                          <span>자료 {c.docCount}개</span>
                          {ready[c.id] && (
                            <span
                              className={
                                'tag tag-' +
                                (ready[c.id].ready === ready[c.id].documents ? 'ok' : 'quiet')
                              }
                              title="의미 검색은 색인해야 쓸 수 있습니다. 색인하지 않아도 낱말 검색은 됩니다."
                            >
                              {ready[c.id].summary}
                            </span>
                          )}
                          {ready[c.id]?.needsAction ? (
                            <span className="tag tag-warn">
                              손볼 자료 {ready[c.id].needsAction}개
                            </span>
                          ) : null}
                        </>
                      )}
                    </div>
                  </div>

                  {confirming === c.id ? (
                    <div className="list-actions">
                      <span className="confirm-ask">
                        안에 든 자료 {c.docCount}개도 함께 지웁니다. 지울까요?
                      </span>
                      <button className="btn btn-danger" disabled={busy} onClick={() => void onDelete(c.id)}>
                        지웁니다
                      </button>
                      <button className="btn" onClick={() => setConfirming(null)}>
                        취소
                      </button>
                    </div>
                  ) : (
                    <div className="list-actions">
                      <button className="btn" onClick={() => setEditing({ id: c.id, name: c.name })}>
                        이름 바꾸기
                      </button>
                      <button className="btn btn-quiet" onClick={() => setConfirming(c.id)}>
                        지우기
                      </button>
                    </div>
                  )}
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
