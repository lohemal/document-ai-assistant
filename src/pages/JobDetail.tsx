import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import {
  collectionNames,
  getJob,
  pinJob,
  statusClass,
  statusLabel,
  when,
  type JobDetail as Detail,
  type JobEvidence,
} from '@/ipc/jobs'
import type { AnswerOut } from '@/ipc/answer'
import type { Span } from '@/ipc/chunks'
import { modeLabel } from '@/ipc/search'
import { message } from '@/lib/err'
import PdfHighlight from '@/components/PdfHighlight'

/**
 * 과거 기록 하나 — **그때 그대로.**
 *
 * 답·검증·판단은 그때 담아 둔 JSON(answerJson)에서 읽고, 근거는 복사해 둔
 * 스냅샷(원문·쪽·형광펜 자리)에서 읽는다. 지금 자료를 다시 검색하지 않는다.
 *
 * 원문 보기는 **당시 파일이 그대로 있을 때만** 연다. 파일이 바뀌었거나 지워졌으면
 * "자료가 변경되었습니다" 를 앞에 놓고, 당시 위치를 지금 원문에 억지로 잇지 않는다.
 */
export default function JobDetail() {
  const { id } = useParams()
  const [d, setD] = useState<Detail | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [open, setOpen] = useState<number | null>(null)

  const load = () => {
    getJob(Number(id))
      .then(setD)
      .catch((e) => setError(message(e)))
  }
  useEffect(load, [id])

  if (error) {
    return (
      <div className="page">
        <p className="error">{error}</p>
        <Link to="/jobs">← 작업 기록</Link>
      </div>
    )
  }
  if (!d) return <div className="page muted">읽고 있습니다…</div>

  let then: AnswerOut | null = null
  try {
    then = JSON.parse(d.answerJson) as AnswerOut
  } catch {
    then = null
  }
  const answered = d.job.status === 'answer' || d.job.status === 'limited'

  const spansOf = (e: JobEvidence): Span[] => {
    try {
      return JSON.parse(e.spansJson) as Span[]
    } catch {
      return []
    }
  }

  return (
    <div className="page page-wide">
      <p className="muted small">
        <Link to="/jobs">← 작업 기록</Link>
      </p>
      <h1 className="page-title">{d.job.question}</h1>
      <p className="page-lead">
        <span className={'statuschip ' + statusClass(d.job.status)}>{statusLabel(d.job.status)}</span>{' '}
        <span className="muted">{when(d.job.createdAt)}</span>
        <button
          className={'pinbtn inline' + (d.job.pinned ? ' is-on' : '')}
          title={d.job.pinned ? '중요 표시 지우기' : '중요 표시'}
          onClick={() => void pinJob(d.job.id, !d.job.pinned).then(load).catch((e) => setError(message(e)))}
        >
          {d.job.pinned ? '★ 중요' : '☆ 중요 표시'}
        </button>
      </p>

      {d.changedCount > 0 && (
        <div className="banner banner-warn">
          <strong>자료가 변경되었습니다.</strong> 근거 {d.evidence.length}개 가운데 {d.changedCount}개의 자료가
          그 뒤에 바뀌거나 지워졌습니다. 아래 답과 근거 원문은 <strong>그때 것 그대로</strong>이고, 지금
          자료와 다를 수 있습니다.
        </div>
      )}

      <section className="card">
        <dl className="kv">
          <dt>자료집</dt>
          <dd>{collectionNames(d.job.collectionsJson)}</dd>
          <dt>검색 방식</dt>
          <dd>
            {modeLabel(d.job.searchMode)}
            {then?.search.modeNote ? ` — ${then.search.modeNote}` : ''}
          </dd>
          <dt>답변 모델</dt>
          <dd>{d.job.llmModel ?? '없음 (근거만 보여 줌)'}</dd>
          {then && (
            <>
              <dt>걸린 시간</dt>
              <dd>
                전체 {(then.totalMs / 1000).toFixed(1)}초
                {then.llmMs > 0 ? ` · 답 쓰기 ${(then.llmMs / 1000).toFixed(1)}초` : ''}
              </dd>
            </>
          )}
        </dl>
      </section>

      {/* ── 답변 (그때 것) ───────────────────────────────────────── */}
      <section className="card card-answer">
        <h2 className="card-title">답변</h2>
        {d.job.status === 'cancelled' && (
          <p className="muted">
            답변 만들기를 멈춘 기록입니다. 물음과 그때 찾은 근거만 남았습니다.
          </p>
        )}
        {d.job.status === 'no_model' && (
          <p className="muted">{then?.modelNote ?? 'AI 답변 없이 검색 결과와 근거만 본 기록입니다.'}</p>
        )}
        {d.job.status !== 'cancelled' && d.job.status !== 'no_model' && (
          <p className="answer-body selectable">{then?.answer ?? '(답을 읽지 못했습니다)'}</p>
        )}
        {then?.interpretation && (
          <div className="banner banner-warn small">자료 해석이 포함된 답변입니다.</div>
        )}
        {then && then.judgement.reasons.length > 0 && (
          <ul className="notes">
            {then.judgement.reasons.map((r) => (
              <li key={r}>{r}</li>
            ))}
          </ul>
        )}
      </section>

      {/* ── 검증 상태 (그때 것) ──────────────────────────────────── */}
      {then?.verdict && (
        <section className="card">
          <h2 className="card-title">검증 상태 (당시)</h2>
          <ul className="checks">
            <li className={then.verdict.citationsOk ? 'ok' : 'bad'}>{then.verdict.citationMessage}</li>
            <li className={then.verdict.numbers.every((n) => n.found) ? 'ok' : 'bad'}>
              {then.verdict.numberMessage}
            </li>
          </ul>
        </section>
      )}

      {/* ── 근거 스냅샷 ──────────────────────────────────────────── */}
      <section className="card">
        <h2 className="card-title">
          당시 근거 <span className="muted small">({d.evidence.length}개 · 그때 복사해 둔 원문)</span>
        </h2>
        {d.evidence.length === 0 && <p className="muted">근거가 없었습니다.</p>}
        <ul className="evlist">
          {d.evidence.map((e) => (
            <li key={e.ord} className={'evitem' + (open === e.ord ? ' is-on' : '')}>
              <div className="evhead">
                <span className="evname">{e.sourceId}</span>
                <span className="hit-doc">{e.docTitle}</span>
                <span className="muted">
                  {e.collectionName && `${e.collectionName} · `}
                  {e.pageStart === e.pageEnd ? `${e.pageStart}쪽` : `${e.pageStart}~${e.pageEnd}쪽`}
                </span>
                {answered && e.cited && <span className="tag tag-ok">답변이 인용</span>}
                {e.docState !== 'same' && (
                  <span className="tag tag-warn">
                    {e.docState === 'deleted' ? '자료 삭제됨' : '자료 변경됨'}
                  </span>
                )}
              </div>
              {e.headingPath && <div className="chunkitem-path">{e.headingPath}</div>}
              <div className="chunks-quote selectable">{e.quotedText}</div>
              {e.note && <div className="banner banner-warn small">{e.note}</div>}
              {e.canOpen && e.documentId !== null ? (
                <button className="btn btn-sm" onClick={() => setOpen(open === e.ord ? null : e.ord)}>
                  {open === e.ord ? '원문 닫기' : e.docState === 'superseded' ? '당시 판 원문 보기' : '원문 보기'}
                </button>
              ) : (
                <span className="muted small">원문 PDF 를 열 수 없습니다 — 위 복사본이 당시 원문입니다.</span>
              )}
              {open === e.ord && e.canOpen && e.documentId !== null && (
                <PdfHighlight documentId={e.documentId} spans={spansOf(e)} page={e.pageStart} />
              )}
            </li>
          ))}
        </ul>
      </section>
    </div>
  )
}
