import type { AnswerOut } from '@/ipc/answer'

/** 개발 정보 — 검색 방식·순위·chunk_id·주장·초점 낱말·모델 원문. 두 화면이 같이 쓴다. */
export default function DevPanel({
  out,
  dev,
  setDev,
}: {
  out: AnswerOut
  dev: boolean
  setDev: (v: boolean) => void
}) {
  return (
    <section className="card">
      <label className="scoretoggle">
        <input type="checkbox" checked={dev} onChange={(e) => setDev(e.target.checked)} />
        개발 정보 보기
      </label>
      {dev && (
        <dl className="kv">
          <dt>일</dt>
          <dd>{out.task}</dd>
          <dt>검색 방식</dt>
          <dd>
            {out.search.mode} {out.search.modeNote && `— ${out.search.modeNote}`}
          </dd>
          <dt>찾아 본 말</dt>
          <dd>{out.search.terms.join(', ') || '—'}</dd>
          <dt>모델의 자기 평가</dt>
          <dd>
            {out.confidence ?? '—'} <span className="muted small">(판단에 쓰지 않습니다)</span>
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
                  [{c.kind === 'fact' ? '사실' : c.kind === 'style' ? '문체' : '해석'}] {c.text} ←{' '}
                  {c.sources.join(', ') || '(근거 없음)'}
                </li>
              ))}
            </ul>
          </dd>
          <dt>초점 낱말</dt>
          <dd>
            {out.focus?.word ? (
              <>
                <code>{out.focus.word}</code> · 자료집 {out.focus.inCollection ? '있음' : '없음'} · 넘긴 근거{' '}
                {out.focus.inEvidence ? '있음' : '없음'} · 인용 청크{' '}
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
                  {c.kind === 'fact' ? (c.supported ? '뒷받침됨' : '확인 안 됨') : `${c.kind}(검사 안 함)`} ·{' '}
                  {c.sources.join(', ') || '인용 없음'}
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
  )
}
