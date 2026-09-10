import { useEffect, useRef, useState } from 'react'
import { listCollections, type Collection } from '@/ipc/collections'
import {
  ask,
  cancelAsk,
  decisionLabel,
  onAnswerProgress,
  type AnswerEvent,
  type AnswerOut,
  type Evidence,
} from '@/ipc/answer'
import { modeLabel } from '@/ipc/search'
import { message } from '@/lib/err'
import PdfHighlight from '@/components/PdfHighlight'

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
 */
export default function Ask() {
  const [collections, setCollections] = useState<Collection[] | null>(null)
  const [collectionId, setCollectionId] = useState<number | 'all'>('all')
  const [text, setText] = useState('')
  const [out, setOut] = useState<AnswerOut | null>(null)
  const [busy, setBusy] = useState(false)
  const [live, setLive] = useState<AnswerEvent | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [open, setOpen] = useState<Evidence | null>(null)
  const [dev, setDev] = useState(false)
  const unlisten = useRef<(() => void) | null>(null)

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
    setOpen(null)
    setError(null)
    setLive({ phase: 'searching', note: '자료를 찾고 있습니다…', chars: 0 })
    try {
      const r = await ask(text, collectionId === 'all' ? [] : [collectionId])
      setOut(r)
      // 인용된 근거를 바로 펼쳐 둔다 — 답만 읽고 넘어가지 않게
      const first = r.evidence.find((v) => r.cited.includes(v.sourceId))
      setOpen(first ?? null)
    } catch (err) {
      setError(message(err))
    } finally {
      setBusy(false)
      setLive(null)
    }
  }

  // 거부했으면 아무것도 "답변이 인용" 으로 표시하지 않는다. 보여 줄 답이 없는데
  // 근거에 인용 표를 붙이면, 답을 못 냈다는 말과 화면이 어긋난다.
  const answered = out?.decision === 'answer' || out?.decision === 'limited'
  const cited = (e: Evidence) => (answered && out?.cited.includes(e.sourceId)) ?? false
  const shown = out?.evidence.filter((e) => cited(e)) ?? []
  const rest = out?.evidence.filter((e) => !cited(e)) ?? []

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
          onChange={(e) =>
            setCollectionId(e.target.value === 'all' ? 'all' : Number(e.target.value))
          }
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
        </div>
      )}

      {error && <p className="banner banner-error">{error}</p>}

      {out && (
        <>
          {/* ── 답 ─────────────────────────────────────────────── */}
          <section className={'card answer answer-' + out.decision}>
            <div className="viewer-head">
              <h2 className="card-title">답변</h2>
              <span className={'tag tag-' + (out.decision === 'answer' ? 'ok' : out.decision === 'limited' ? 'warn' : 'quiet')}>
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
              <p className="answer-refusal">{out.modelNote ?? out.judgement.reasons[0]}</p>
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
          </section>

          {/* ── 근거 ───────────────────────────────────────────── */}
          {out.evidence.length > 0 && (
            <section className="card">
              <div className="viewer-head">
                <h2 className="card-title">
                  근거{' '}
                  <span className="muted small">
                    ({shown.length > 0 && `답변이 인용 ${shown.length}개 · `}
                    AI 에게 넘긴 것 {out.evidence.length}개)
                  </span>
                </h2>
                <span className="muted small">
                  검색 방식: {modeLabel(out.search.mode)} · {out.search.hits}건 중에서 골랐습니다
                </span>
              </div>
              {(out.decision === 'refuse' || out.decision === 'no_model') && rest.length > 0 && (
                <p className="muted">다만 다음 내용은 관련이 있습니다.</p>
              )}

              <ul className="evlist">
                {[...shown, ...rest].map((e) => (
                  <li
                    key={e.chunkId}
                    className={
                      'evitem' +
                      (open?.chunkId === e.chunkId ? ' is-on' : '') +
                      (cited(e) ? ' is-cited' : '')
                    }
                  >
                    <button className="evhead" onClick={() => setOpen(open?.chunkId === e.chunkId ? null : e)}>
                      <span className="evname">{e.sourceId}</span>
                      <span className="hit-doc">{e.docTitle}</span>
                      <span className="muted">
                        {e.pageStart === e.pageEnd
                          ? `${e.pageStart}쪽`
                          : `${e.pageStart}~${e.pageEnd}쪽`}
                      </span>
                      {cited(e) && <span className="tag tag-ok">답변이 인용</span>}
                      {e.neighbor && <span className="tag tag-quiet">앞뒤 문맥</span>}
                      {dev && (
                        <span className="muted small">
                          청크 {e.chunkId} · 섞기 {e.searchRank ?? '—'}등 · 낱말{' '}
                          {e.keywordRank ?? '—'}등 · 뜻 {e.semanticRank ?? '—'}등
                        </span>
                      )}
                    </button>
                    {e.headingPath && <div className="chunkitem-path">{e.headingPath}</div>}
                    <div className="chunks-quote selectable">
                      {e.text.slice(0, 300)}
                      {e.text.length > 300 && '…'}
                    </div>
                    {open?.chunkId === e.chunkId && (
                      <PdfHighlight documentId={e.documentId} spans={e.spans} />
                    )}
                  </li>
                ))}
              </ul>
            </section>
          )}

          {/* ── 검증 상태 ──────────────────────────────────────── */}
          {out.verdict && (
            <section className="card">
              <h2 className="card-title">검증 상태</h2>
              <ul className="checks">
                <li className={out.verdict.citationsOk ? 'ok' : 'bad'}>
                  {out.verdict.citationMessage}
                </li>
                <li className={out.verdict.numbers.every((n) => n.found) ? 'ok' : 'bad'}>
                  {out.verdict.numberMessage}
                </li>
                {/* 해석은 세지 않는다 — 근거의 말을 옮긴 것이 아니므로 낱말이
                    겹치지 않는 것이 정상이다 (Rust 쪽 `unsupported_claims` 와 같은 자) */}
                {(() => {
                  const facts = out.verdict.claims.filter((c) => c.kind === 'fact')
                  const weak = facts.filter((c) => !c.supported)
                  return (
                    <li className={facts.length === 0 ? 'warn' : weak.length === 0 ? 'ok' : 'bad'}>
                      {facts.length === 0
                        ? 'AI 가 사실 주장을 따로 적지 않아 뒷받침을 확인하지 못했습니다.'
                        : weak.length === 0
                          ? '답변의 주장이 모두 인용한 근거 안에서 확인되었습니다.'
                          : `주장 ${weak.length}개를 인용한 근거에서 확인하지 못했습니다.`}
                    </li>
                  )
                })()}
                <li className={out.interpretation ? 'warn' : 'ok'}>
                  {out.interpretation
                    ? '자료 해석이 포함되어 있습니다.'
                    : '자료에 적힌 내용만 담겨 있습니다.'}
                </li>
              </ul>

              {out.verdict.numbers.length > 0 && (
                <table className="numtable">
                  <tbody>
                    {out.verdict.numbers.map((n) => (
                      <tr key={n.raw + n.kind}>
                        <td className="muted">{n.kind}</td>
                        <td>
                          <code>{n.raw}</code>
                        </td>
                        <td>
                          {n.found ? (
                            <span className="tag tag-ok">{n.sourceId} 에서 확인</span>
                          ) : (
                            <span className="tag tag-warn">인용 근거에서 확인 안 됨</span>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}

              <p className="muted small">
                숫자 확인은 <strong>그 값이 인용한 근거 안에 있는지</strong>만 봅니다. 값이 그
                자리에 맞게 쓰였는지까지는 확인하지 못합니다 — 중요한 숫자는 위 근거 원문을 꼭
                직접 확인해 주세요.
              </p>

              {out.judgement.reasons.length > 0 && (
                <ul className="notes">
                  {out.judgement.reasons.map((r) => (
                    <li key={r}>{r}</li>
                  ))}
                </ul>
              )}
            </section>
          )}

          {/* ── 개발용 ─────────────────────────────────────────── */}
          <section className="card">
            <label className="scoretoggle">
              <input type="checkbox" checked={dev} onChange={(e) => setDev(e.target.checked)} />
              개발 정보 보기
            </label>
            {dev && (
              <dl className="kv">
                <dt>검색 방식</dt>
                <dd>
                  {out.search.mode} {out.search.modeNote && `— ${out.search.modeNote}`}
                </dd>
                <dt>찾아 본 말</dt>
                <dd>{out.search.terms.join(', ') || '—'}</dd>
                <dt>모델의 자기 평가</dt>
                <dd>
                  {out.confidence ?? '—'}{' '}
                  <span className="muted small">(판단에 쓰지 않습니다)</span>
                </dd>
                <dt>답변 모델</dt>
                <dd>{out.model ?? '없음'}</dd>
                <dt>걸린 시간</dt>
                <dd>
                  전체 {(out.totalMs / 1000).toFixed(1)}초 · 검색 {out.search.elapsedMs}ms · 생성{' '}
                  {(out.llmMs / 1000).toFixed(1)}초 · {out.tokens}토큰
                </dd>
                <dt>인용한 근거</dt>
                <dd>{out.cited.join(', ') || '없음'}</dd>
                <dt>주장</dt>
                <dd>
                  <ul className="notes">
                    {out.claims.map((c, i) => (
                      <li key={i}>
                        [{c.kind === 'fact' ? '사실' : '해석'}] {c.text} ← {c.sources.join(', ')}
                      </li>
                    ))}
                  </ul>
                </dd>
                <dt>초점 낱말</dt>
                <dd>
                  {out.focus?.word ? (
                    <>
                      <code>{out.focus.word}</code> · 자료집 {out.focus.inCollection ? '있음' : '없음'} · 넘긴
                      근거 {out.focus.inEvidence ? '있음' : '없음'} · 인용 청크{' '}
                      {out.focus.inCited === null ? '—' : out.focus.inCited ? '있음' : '없음'}
                    </>
                  ) : (
                    '물음에서 개념 낱말을 고르지 못했습니다'
                  )}
                </dd>
                <dt>뒷받침 검사</dt>
                <dd>
                  <ul className="notes">
                    {(out.verdict?.claims ?? []).map((c, i) => (
                      <li key={i}>
                        겹침 {(c.overlap * 100).toFixed(0)}% ·{' '}
                        {c.kind === 'fact' ? (c.supported ? '뒷받침됨' : '확인 안 됨') : '해석(검사 안 함)'}{' '}
                        · {c.sources.join(', ') || '인용 없음'}
                        {c.repairedFrom && ` (AI 는 ${c.repairedFrom} 라고 적었다)`}
                      </li>
                    ))}
                  </ul>
                </dd>
                <dt>모델이 내놓은 글</dt>
                <dd>
                  <pre className="raw selectable">{out.raw ?? '—'}</pre>
                </dd>
              </dl>
            )}
          </section>
        </>
      )}
    </div>
  )
}
