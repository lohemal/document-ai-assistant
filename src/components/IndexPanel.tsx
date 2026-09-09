import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  elapsedText,
  indexAction,
  indexStart,
  indexStatus,
  indexStop,
  onIndexProgress,
  remainingText,
  type DocIndex,
  type IndexEvent,
  type IndexOverview,
} from '@/ipc/index'
import { message } from '@/lib/err'

/** 상태마다 태그 색을 다르게 — 손을 대야 하는 것이 눈에 띄게 */
function tagKind(s: DocIndex['state']): string {
  switch (s) {
    case 'done':
      return 'ok'
    case 'model_mismatch':
    case 'needs_reindex':
    case 'failed':
      return 'warn'
    case 'running':
    case 'queued':
      return 'busy'
    default:
      return 'quiet'
  }
}

type Props = {
  collectionId: number | null
  /** 바깥에서 자료가 바뀌었을 때 올려 주는 수. 바뀌면 상태를 다시 읽는다 */
  version?: number
  /** 색인이 끝나면 바깥 화면도 새로 그리게 */
  onChanged?: () => void
}

/**
 * 의미 검색 색인 — 상태·진행률·멈추기·이어서 하기.
 *
 * 등록과 나뉘어 있는 것이 핵심이다. 등록은 몇 초, 색인은 몇 분 걸린다.
 * 색인을 하지 않아도 낱말 검색은 그대로 되므로, 서두를 일이 아니라는 것을
 * 화면이 말해 준다.
 */
export default function IndexPanel({ collectionId, version, onChanged }: Props) {
  const [view, setView] = useState<IndexOverview | null>(null)
  const [live, setLive] = useState<IndexEvent | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const unlisten = useRef<(() => void) | null>(null)

  const reload = useCallback(async () => {
    if (collectionId === null) return
    try {
      setView(await indexStatus(collectionId))
      setError(null)
    } catch (e) {
      setError(message(e))
    }
  }, [collectionId])

  // 자료집이 바뀌거나, 바깥에서 자료를 등록·삭제했을 때 다시 읽는다
  useEffect(() => {
    void reload()
  }, [reload, version])

  useEffect(() => {
    void onIndexProgress((e) => {
      setLive(e)
      // 한 문서가 끝나거나 멈추면 목록을 다시 읽는다
      if (e.state !== 'running') {
        void reload()
        onChanged?.()
        if (e.state === 'done' && e.queued === 0) setLive(null)
      }
    }).then((f) => (unlisten.current = f))
    return () => unlisten.current?.()
  }, [reload, onChanged])

  async function start(ids: number[], reindex: boolean) {
    setError(null)
    setNotice(null)
    setBusy(true)
    try {
      await indexStart(ids, reindex)
      setNotice('색인을 끝냈습니다.')
    } catch (e) {
      setError(message(e))
    } finally {
      setBusy(false)
      setLive(null)
      await reload()
      onChanged?.()
    }
  }

  async function stop() {
    try {
      await indexStop()
      setNotice('색인을 멈췄습니다. 만들어 둔 것은 그대로 남아 있어, 다음에 이어서 할 수 있습니다.')
    } catch (e) {
      setError(message(e))
    }
  }

  /** 낱말 검색만 쓰겠다 — 쓸 수 없는 벡터를 지워 자리를 비운다 */
  async function dropUnusable(id: number) {
    try {
      const n = await invoke<number>('index_drop_unusable', { documentIds: [id] })
      setNotice(`쓸 수 없는 벡터 ${n}개를 지웠습니다. 이 자료는 낱말로 찾습니다.`)
      await reload()
    } catch (e) {
      setError(message(e))
    }
  }

  if (!view) return null

  const todo = view.documents.filter((d) => indexAction(d.state) !== null)
  const ready = view.documents.filter((d) => d.state === 'done').length

  return (
    <section className="card">
      <div className="viewer-head">
        <h2 className="card-title">의미 검색 색인</h2>
        <span className="muted">
          문서 {view.documents.length}개 중 {ready}개 준비됨
        </span>
      </div>

      <p className="muted small">
        색인하면 <strong>같은 뜻의 다른 말</strong>로 물어도 찾습니다. 색인하지 않아도 낱말
        검색은 그대로 됩니다 — 급하지 않으면 나중에 하셔도 됩니다.
      </p>

      {error && <p className="banner banner-error">{error}</p>}
      {notice && <p className="banner banner-info">{notice}</p>}

      {live && (
        <div className="progress">
          <div className="progress-line">
            <strong>{live.title}</strong>
            {'  '}
            {live.done} / {live.total} 청크 · {live.percent}%
          </div>
          <div className="progress-bar">
            <div className="progress-fill" style={{ width: `${live.percent}%` }} />
          </div>
          <div className="progress-line muted small">
            {elapsedText(live.elapsedMs)} 걸림
            {live.remainingMs !== null && ` · ${remainingText(live.remainingMs)}`}
            {' · '}
            {live.modelName}
            {live.queued > 0 && ` · 남은 자료 ${live.queued}개`}
          </div>
          <button className="btn" onClick={() => void stop()}>
            멈추기
          </button>
        </div>
      )}

      {!live && todo.length > 0 && (
        <div className="row-form">
          <button
            className="btn btn-primary"
            disabled={busy}
            onClick={() =>
              void start(
                todo.map((d) => d.documentId),
                todo.some((d) => indexAction(d.state)?.reindex ?? false),
              )
            }
          >
            {todo.length === 1 ? '이 자료 색인' : `자료 ${todo.length}개 모두 색인`}
          </button>
          <span className="muted small">
            {view.model.name} · 청크당 약 {(view.model.msPerChunk / 1000).toFixed(1)}초
          </span>
        </div>
      )}

      <ul className="list">
        {view.documents.map((raw) => {
          // 도는 중인 문서는 목록에서도 살아 움직이게 한다. 목록은 판이 끝날
          // 때만 다시 읽으므로, 그동안 0 / 255 로 멈춰 보이면 고장 같다.
          const d =
            live && live.documentId === raw.documentId
              ? { ...raw, done: live.done, state: 'running' as const, label: '색인 중' }
              : raw
          const action = indexAction(d.state)
          return (
            <li className="list-item" key={d.documentId}>
              <div className="list-main">
                <div className="list-name">{d.title}</div>
                <div className="list-sub">
                  <span className={'tag tag-' + tagKind(d.state)}>{d.label}</span>
                  {d.total > 0 && (
                    <span className="muted">
                      {d.done} / {d.total} 청크
                    </span>
                  )}
                  {d.indexedWith && d.state === 'model_mismatch' && (
                    <span className="muted">({d.indexedWith} 로 색인됨)</span>
                  )}
                </div>

                {(d.state === 'model_mismatch' || d.state === 'needs_reindex') && (
                  <div className="choice">
                    <p className="list-warn">
                      {d.state === 'model_mismatch' ? (
                        <>
                          이 자료는 이전 검색 모델({d.indexedWith})로 색인되어 있습니다. 지금
                          모델({view.model.name})로 의미 검색을 쓰려면 다시 색인해야 합니다.
                        </>
                      ) : (
                        <>
                          자료가 바뀌어 옛 벡터 {d.stale}개를 쓸 수 없습니다. 다시 색인해야 의미
                          검색이 맞습니다.
                        </>
                      )}
                    </p>
                    {/* 고를 것을 설명 바로 아래에 둔다. 오른쪽 끝에 몰아 두면
                        무엇에 대한 선택인지 읽히지 않는다. */}
                    <div className="choice-row">
                      <button
                        className="btn"
                        disabled={busy || live !== null}
                        onClick={() => void start([d.documentId], true)}
                      >
                        현재 모델로 다시 색인
                      </button>
                      <button
                        className="btn btn-quiet"
                        disabled={busy}
                        onClick={() =>
                          setNotice(
                            '기존 색인을 그대로 두었습니다. 이 자료는 낱말로 찾습니다. 검색 모델을 되돌리면 만들어 둔 벡터를 다시 쓸 수 있습니다.',
                          )
                        }
                      >
                        기존 색인 유지
                      </button>
                      <button
                        className="btn btn-quiet"
                        disabled={busy}
                        onClick={() => void dropUnusable(d.documentId)}
                      >
                        낱말 검색만 쓰기
                      </button>
                    </div>
                  </div>
                )}
                {d.state === 'failed' && d.error && <div className="list-warn">⚠ {d.error}</div>}
              </div>

              <div className="list-actions">
                {action && !live && !action.reindex && (
                  <button
                    className="btn"
                    disabled={busy}
                    onClick={() => void start([d.documentId], action.reindex)}
                  >
                    {action.label}
                  </button>
                )}
              </div>
            </li>
          )
        })}
      </ul>
    </section>
  )
}
