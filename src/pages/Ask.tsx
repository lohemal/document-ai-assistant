import { useEffect, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { listCollections, type Collection } from '@/ipc/collections'
import {
  answerExpect,
  ask,
  cancelAsk,
  decisionLabel,
  expectLine,
  onAnswerProgress,
  type AnswerEvent,
  type AnswerOut,
} from '@/ipc/answer'
import { message } from '@/lib/err'
import EvidenceList from '@/components/EvidenceList'
import VerificationCard from '@/components/VerificationCard'
import DevPanel from '@/components/DevPanel'

/**
 * 근거를 읽고 답한다 (P5).
 *
 * **이 화면이 지켜야 하는 것은 "답을 잘 보여 주는 것" 이 아니다.** 답과 근거를
 * 나란히 두어, 사용자가 답을 믿을지 스스로 정할 수 있게 하는 것이다. 그래서
 *
 *   - 근거를 접어 두지 않는다. 답 바로 아래에 늘 보인다.
 *   - 확인하지 못한 것은 확인하지 못했다고 적는다.
 *   - "검증 완료" 같은 말은 쓰지 않는다 (설계안 5-5).
 *   - AI 답변을 못 쓸 때도 **검색과 근거는 그대로 보여 준다** (요구사항 13).
 *
 * 근거 목록·검증 상태·개발 정보는 문서 작성 화면과 같은 부품(`components/`)이다.
 */
export default function Ask() {
  const [collections, setCollections] = useState<Collection[] | null>(null)
  const [collectionId, setCollectionId] = useState<number | 'all'>('all')
  const [text, setText] = useState('')
  const [out, setOut] = useState<AnswerOut | null>(null)
  const [busy, setBusy] = useState(false)
  const [live, setLive] = useState<AnswerEvent | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [dev, setDev] = useState(false)
  // 이 PC 에서 이 모델이 실제로 걸리는 시간 — 기다리는 동안 "보통 N분" 으로 보여 준다
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
    setLive({ phase: 'searching', note: '자료를 찾고 있습니다…', chars: 0 })
    // 지난 실측은 물을 때마다 다시 읽는다 — 첫 답이 끝나면 그 시간이 바로 다음 안내가 된다
    answerExpect()
      .then((e) => setExpect(expectLine(e)))
      .catch(() => {})
    try {
      setOut(await ask(text, collectionId === 'all' ? [] : [collectionId]))
    } catch (err) {
      setError(message(err))
    } finally {
      setBusy(false)
      setLive(null)
    }
  }

  return (
    <div className="page page-wide">
      <h1 className="page-title">규정 해석</h1>
      <p className="page-lead">
        등록한 자료에서 <strong>근거를 찾아</strong> 답합니다. 자료에 없는 내용은 답하지 않습니다.
      </p>

      <form className="searchbar" onSubmit={run}>
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
        <input
          className="input"
          placeholder="물어보세요 (예: 경조사비는 1인당 얼마까지 집행할 수 있어?)"
          value={text}
          disabled={busy}
          onChange={(e) => setText(e.target.value)}
        />
        {busy ? (
          <button type="button" className="btn" onClick={() => void cancelAsk()}>
            멈추기
          </button>
        ) : (
          <button className="btn btn-primary" disabled={!text.trim()}>
            물어보기
          </button>
        )}
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
          {/* ── 답 ─────────────────────────────────────────────── */}
          <section className={'card answer answer-' + out.decision}>
            <div className="viewer-head">
              <h2 className="card-title">답변</h2>
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
                  등록된 자료에서는 이 질문에 답할 수 있는 근거를 충분히 찾지 못했습니다.
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
                  ? '답변 만들기를 멈췼습니다. 찾은 근거는 그대로 있습니다.'
                  : (out.modelNote ?? out.judgement.reasons[0])}
              </p>
            )}
            {(out.decision === 'answer' || out.decision === 'limited') && (
              <p className="answer-body selectable">{out.answer}</p>
            )}

            {out.interpretation && (
              <p className="banner banner-info small">
                자료 해석이 포함된 답변입니다. 아래 근거 원문을 함께 확인해 주세요.
              </p>
            )}
            {out.interpretationNotes.length > 0 && (
              <ul className="notes">
                {out.interpretationNotes.map((n) => (
                  <li key={n}>{n}</li>
                ))}
              </ul>
            )}

            {/* 기록 링크는 답 바로 아래 — 긴 근거 목록 아래에 두면 찾지 못한다 (P6 검증) */}
            {out.jobId !== null && (
              <p className="muted small record-link">
                이 물음과 그때의 근거는 작업 기록에 남았습니다. <Link to={`/jobs/${out.jobId}`}>기록 보기</Link>
              </p>
            )}
          </section>

          <EvidenceList out={out} dev={dev} />
          <VerificationCard out={out} />
          <DevPanel out={out} dev={dev} setDev={setDev} />
        </>
      )}
    </div>
  )
}
