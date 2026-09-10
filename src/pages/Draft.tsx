import { useEffect, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { listCollections, type Collection } from '@/ipc/collections'
import {
  answerExpect,
  cancelAsk,
  decisionLabel,
  expectLine,
  makeDraft,
  onAnswerProgress,
  type AnswerEvent,
  type AnswerOut,
} from '@/ipc/answer'
import { message } from '@/lib/err'
import EvidenceList from '@/components/EvidenceList'
import VerificationCard from '@/components/VerificationCard'
import DevPanel from '@/components/DevPanel'
import DraftBody from '@/components/DraftBody'

type Format = 'letter' | 'sms'

/**
 * 문서 작성 (P7) — 가정통신문 · 문자 초안.
 *
 * 규정 해석과 **같은 길**이다: 자료집에서 먼저 찾고, 찾은 근거만으로 쓰고, 인용·숫자를
 * 검증하고, 근거가 모자라면 초안을 만들지 않는다. 다른 것은 형식(옷)과 이 화면뿐이다.
 * 근거 없는 일반 생성이 아니다 (설계안 5-6).
 *
 * 학교명·날짜·연락처처럼 자료에 없는 값은 [학교명] 처럼 사용자가 채울 자리로 남는다.
 */
export default function Draft() {
  const [collections, setCollections] = useState<Collection[] | null>(null)
  const [collectionId, setCollectionId] = useState<number | 'all'>('all')
  const [text, setText] = useState('')
  const [format, setFormat] = useState<Format>('letter')
  const [out, setOut] = useState<AnswerOut | null>(null)
  const [busy, setBusy] = useState(false)
  const [live, setLive] = useState<AnswerEvent | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [dev, setDev] = useState(false)
  const [expect, setExpect] = useState<string | null>(null)
  const unlisten = useRef<(() => void) | null>(null)

  useEffect(() => {
    answerExpect()
      .then((e) => setExpect(expectLine(e)))
      .catch(() => setExpect(null))
  }, [])

  useEffect(() => {
    listCollections()
      .then(setCollections)
      .catch((e) => setError(message(e)))
  }, [])

  useEffect(() => {
    void onAnswerProgress(setLive).then((f) => (unlisten.current = f))
    return () => unlisten.current?.()
  }, [])

  async function run(e?: React.FormEvent) {
    e?.preventDefault()
    if (!text.trim()) return
    setBusy(true)
    setOut(null)
    setError(null)
    setCopied(false)
    setLive({ phase: 'searching', note: '자료를 찾고 있습니다…', chars: 0 })
    try {
      setOut(await makeDraft(text, collectionId === 'all' ? [] : [collectionId], format))
    } catch (err) {
      setError(message(err))
    } finally {
      setBusy(false)
      setLive(null)
    }
  }

  const made = out && (out.decision === 'answer' || out.decision === 'limited')
  const clipboardText = out
    ? out.task === 'letter' && out.title
      ? `${out.title}\n\n${out.answer}`
      : out.answer
    : ''

  async function copy() {
    try {
      await navigator.clipboard.writeText(clipboardText)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (e) {
      setError(message(e))
    }
  }

  // 한글 문자는 90바이트(SMS)를 넘으면 장문(LMS)으로 간다. 대강 45자다.
  const chars = out?.answer.length ?? 0
  const smsNote = chars <= 45 ? 'SMS 한 건 분량' : chars <= 1000 ? '장문(LMS) 분량' : '너무 깁니다'

  return (
    <div className="page page-wide">
      <h1 className="page-title">문서 작성</h1>
      <p className="page-lead">
        등록한 자료에서 <strong>근거를 먼저 찾고</strong>, 그 근거로만 초안을 씁니다. 자료에 없는 학교명·날짜·
        연락처는 <code>[학교명]</code>처럼 비워 둡니다 — 직접 채워 주세요.
      </p>

      <form className="draftform" onSubmit={run}>
        <div className="row">
          <select
            className="input"
            value={collectionId}
            disabled={busy}
            onChange={(e) => setCollectionId(e.target.value === 'all' ? 'all' : Number(e.target.value))}
          >
            <option value="all">전체 자료</option>
            {collections?.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
          <div className="segmented" role="radiogroup" aria-label="형식">
            <button
              type="button"
              className={'seg' + (format === 'letter' ? ' is-on' : '')}
              disabled={busy}
              onClick={() => setFormat('letter')}
            >
              가정통신문
            </button>
            <button
              type="button"
              className={'seg' + (format === 'sms' ? ' is-on' : '')}
              disabled={busy}
              onClick={() => setFormat('sms')}
            >
              문자
            </button>
          </div>
        </div>
        <textarea
          className="input textarea"
          rows={3}
          placeholder="작성할 내용 (예: 3학년 지원금 관련 내용을 검토해서 학부모에게 안내할 가정통신문을 간략하게 작성해줘)"
          value={text}
          disabled={busy}
          onChange={(e) => setText(e.target.value)}
        />
        <div className="row">
          {busy ? (
            <button type="button" className="btn" onClick={() => void cancelAsk()}>
              멈추기
            </button>
          ) : (
            <button className="btn btn-primary" disabled={!text.trim()}>
              초안 만들기
            </button>
          )}
        </div>
      </form>

      {live && (
        <div className="progress">
          <div className="progress-line">{live.note}</div>
          <div className="progress-bar">
            <div className="progress-fill progress-idle" />
          </div>
          {live.chars > 0 && <div className="progress-line muted small">{live.chars}자</div>}
          {expect && <div className="progress-line muted small">{expect}</div>}
        </div>
      )}

      {error && <p className="banner banner-error">{error}</p>}

      {out && (
        <>
          {/* ── 초안 ───────────────────────────────────────────── */}
          <section className={'card answer answer-' + out.decision}>
            <div className="viewer-head">
              <h2 className="card-title">{out.task === 'letter' ? '가정통신문 초안' : '문자 초안'}</h2>
              <span
                className={
                  'tag tag-' + (out.decision === 'answer' ? 'ok' : out.decision === 'limited' ? 'warn' : 'quiet')
                }
              >
                {decisionLabel(out.decision)}
              </span>
            </div>

            {out.decision === 'refuse' && (
              <>
                <p className="answer-refusal">
                  등록된 자료에서는 이 내용을 쓸 수 있는 근거를 충분히 찾지 못했습니다. 초안을 만들지 않았습니다.
                </p>
                {out.judgement.reasons.length > 0 && (
                  <ul className="notes">
                    {out.judgement.reasons.map((r) => (
                      <li key={r}>{r}</li>
                    ))}
                  </ul>
                )}
              </>
            )}
            {out.decision === 'no_model' && (
              <p className="answer-refusal">
                {out.cancelled
                  ? '초안 만들기를 멈췼습니다. 찾은 근거는 그대로 있습니다.'
                  : (out.modelNote ?? out.judgement.reasons[0])}
              </p>
            )}

            {made && (
              <>
                {out.task === 'letter' && out.title && <h3 className="draft-title selectable">{out.title}</h3>}
                <DraftBody out={out} />
                <div className="row">
                  <button className="btn" onClick={() => void copy()}>
                    {copied ? '복사했습니다' : out.task === 'letter' ? '제목·본문 복사' : '문자 복사'}
                  </button>
                  <span className="muted small">
                    {chars}자{out.task === 'sms' && ` · ${smsNote}`}
                  </span>
                </div>
                {(() => {
                  const bad = (out.verdict?.numbers ?? []).filter((n) => !n.found).map((n) => n.raw)
                  if (bad.length === 0 && out.uncovered.length === 0) return null
                  return (
                    <p className="banner banner-warn small">
                      <strong>보내기 전에 확인하세요.</strong>{' '}
                      {bad.length > 0 && `인용 근거에서 확인하지 못한 숫자 ${bad.length}개: ${bad.join(', ')}. `}
                      {out.uncovered.length > 0 &&
                        `근거에서 온 주장에 없는 문장 ${out.uncovered.length}개 (본문에 표시). `}
                      표시된 부분은 자료에 없는 내용일 수 있습니다.
                    </p>
                  )
                })()}
                {/\[[^\]]+\]/.test(out.answer + (out.title ?? '')) && (
                  <p className="banner banner-info small">
                    <code>[ ]</code> 안은 자료에 없어 비워 둔 자리입니다. 보내기 전에 채워 주세요.
                  </p>
                )}
              </>
            )}

            {out.interpretation && (
              <p className="banner banner-info small">자료 해석이 포함되어 있습니다. 근거 원문을 함께 확인해 주세요.</p>
            )}

            {out.jobId !== null && (
              <p className="muted small record-link">
                이 요청과 그때의 근거는 작업 기록에 남았습니다. <Link to={`/jobs/${out.jobId}`}>기록 보기</Link>
              </p>
            )}
          </section>

          <EvidenceList out={out} dev={dev} what="초안" />
          <VerificationCard out={out} what="초안" />
          <DevPanel out={out} dev={dev} setDev={setDev} />
        </>
      )}
    </div>
  )
}
