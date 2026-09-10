import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  collectionNames,
  deleteJob,
  jobRetention,
  listJobs,
  pinJob,
  statusClass,
  statusLabel,
  when,
  type JobRow,
} from '@/ipc/jobs'
import { modeLabel } from '@/ipc/search'
import { message } from '@/lib/err'

/**
 * 작업 기록 (P6) — 최근 · 중요.
 *
 * 기록 하나는 "그때 무엇을 물었고 어떤 근거로 어떻게 답했는가" 다. 여기서는
 * 목록만 보이고, 누르면 당시 답과 근거가 **그때 그대로** 열린다 (JobDetail).
 * 중요 표시(★)한 기록은 보존기간이 지나도 지우지 않는다.
 */
export default function Jobs() {
  const [tab, setTab] = useState<'recent' | 'pinned'>('recent')
  const [rows, setRows] = useState<JobRow[] | null>(null)
  const [retention, setRetention] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [confirm, setConfirm] = useState<number | null>(null)

  const load = useCallback(() => {
    listJobs(tab === 'pinned')
      .then(setRows)
      .catch((e) => setError(message(e)))
  }, [tab])

  useEffect(load, [load])
  useEffect(() => {
    jobRetention().then(setRetention).catch(() => setRetention(null))
  }, [])

  async function togglePin(r: JobRow) {
    try {
      await pinJob(r.id, !r.pinned)
      load()
    } catch (e) {
      setError(message(e))
    }
  }

  async function remove(id: number) {
    try {
      await deleteJob(id)
      setConfirm(null)
      load()
    } catch (e) {
      setError(message(e))
    }
  }

  return (
    <div className="page page-wide">
      <h1 className="page-title">작업 기록</h1>
      <p className="page-lead">
        물었던 것과 <strong>그때의 답·근거</strong>가 그대로 남습니다. 자료가 나중에 바뀌어도 기록은
        바뀌지 않습니다.
      </p>

      <div className="tabs">
        <button className={'tab' + (tab === 'recent' ? ' is-on' : '')} onClick={() => setTab('recent')}>
          최근
        </button>
        <button className={'tab' + (tab === 'pinned' ? ' is-on' : '')} onClick={() => setTab('pinned')}>
          ★ 중요
        </button>
      </div>

      {error && <p className="error">{error}</p>}

      {rows && rows.length === 0 && (
        <section className="card">
          <p className="muted">
            {tab === 'pinned'
              ? '중요 표시한 기록이 없습니다. 목록에서 ★ 을 누르면 여기 모입니다.'
              : '아직 기록이 없습니다. 규정 해석에서 물어보면 여기 남습니다.'}
          </p>
        </section>
      )}

      {rows && rows.length > 0 && (
        <ul className="joblist">
          {rows.map((r) => (
            <li key={r.id} className={'jobrow' + (r.pinned ? ' is-pinned' : '')}>
              <button
                className={'pinbtn' + (r.pinned ? ' is-on' : '')}
                title={r.pinned ? '중요 표시 지우기' : '중요 표시'}
                onClick={() => void togglePin(r)}
              >
                {r.pinned ? '★' : '☆'}
              </button>
              <div className="jobmain">
                <Link to={`/jobs/${r.id}`} className="jobq">
                  {r.question}
                </Link>
                <div className="jobmeta muted small">
                  {when(r.createdAt)} · {collectionNames(r.collectionsJson)} · 근거 {r.evidenceCount}개 ·{' '}
                  {modeLabel(r.searchMode)}
                  {r.llmModel ? ` · ${r.llmModel}` : ''}
                </div>
              </div>
              <span className={'statuschip ' + statusClass(r.status)}>{statusLabel(r.status)}</span>
              {confirm === r.id ? (
                <span className="confirm">
                  지울까요?
                  <button className="btn btn-danger btn-sm" onClick={() => void remove(r.id)}>
                    지웁니다
                  </button>
                  <button className="btn btn-sm" onClick={() => setConfirm(null)}>
                    취소
                  </button>
                </span>
              ) : (
                <button className="btn btn-sm" onClick={() => setConfirm(r.id)}>
                  지우기
                </button>
              )}
            </li>
          ))}
        </ul>
      )}

      <p className="muted small">
        {retention === null
          ? ''
          : retention === 0
            ? '기록은 직접 지울 때까지 남습니다.'
            : `중요 표시하지 않은 기록은 ${retention}일이 지나면 자동으로 지워집니다.`}{' '}
        보존기간은 <Link to="/settings">설정</Link>에서 바꿉니다.
      </p>
    </div>
  )
}
