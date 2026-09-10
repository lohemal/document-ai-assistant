import { useState } from 'react'
import type { AnswerOut, Evidence } from '@/ipc/answer'
import { modeLabel } from '@/ipc/search'
import PdfHighlight from '@/components/PdfHighlight'

/**
 * 답·초안이 딛고 선 근거 목록 — 규정 해석과 문서 작성이 **같은 부품**을 쓴다.
 *
 * 인용된 근거를 먼저, 나머지를 뒤에. 근거를 누르면 원문 PDF 가 그 자리에 열린다.
 * 거부했을 때는 아무것도 "인용" 으로 표시하지 않는다 — 보여 줄 답이 없는데
 * 근거에 인용 표를 붙이면, 답을 못 냈다는 말과 화면이 어긋난다.
 */
export default function EvidenceList({
  out,
  dev,
  what = '답변',
}: {
  out: AnswerOut
  dev: boolean
  /** "답변이 인용" 의 앞말 — 문서 작성에서는 "초안" */
  what?: string
}) {
  const [open, setOpen] = useState<Evidence | null>(() => {
    // 인용된 첫 근거를 바로 펼쳐 둔다 — 답만 읽고 넘어가지 않게
    const answered = out.decision === 'answer' || out.decision === 'limited'
    return answered ? (out.evidence.find((v) => out.cited.includes(v.sourceId)) ?? null) : null
  })

  const answered = out.decision === 'answer' || out.decision === 'limited'
  const cited = (e: Evidence) => answered && out.cited.includes(e.sourceId)
  const shown = out.evidence.filter((e) => cited(e))
  const rest = out.evidence.filter((e) => !cited(e))

  if (out.evidence.length === 0) return null

  return (
    <section className="card">
      <div className="viewer-head">
        <h2 className="card-title">
          근거{' '}
          <span className="muted small">
            ({shown.length > 0 && `${what}이 인용 ${shown.length}개 · `}
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
            className={'evitem' + (open?.chunkId === e.chunkId ? ' is-on' : '') + (cited(e) ? ' is-cited' : '')}
          >
            <button className="evhead" onClick={() => setOpen(open?.chunkId === e.chunkId ? null : e)}>
              <span className="evname">{e.sourceId}</span>
              <span className="hit-doc">{e.docTitle}</span>
              <span className="muted">
                {e.pageStart === e.pageEnd ? `${e.pageStart}쪽` : `${e.pageStart}~${e.pageEnd}쪽`}
              </span>
              {cited(e) && <span className="tag tag-ok">{what}이 인용</span>}
              {e.neighbor && <span className="tag tag-quiet">앞뒤 문맥</span>}
              {dev && (
                <span className="muted small">
                  청크 {e.chunkId} · 섞기 {e.searchRank ?? '—'}등 · 낱말 {e.keywordRank ?? '—'}등 · 뜻{' '}
                  {e.semanticRank ?? '—'}등
                </span>
              )}
            </button>
            {e.headingPath && <div className="chunkitem-path">{e.headingPath}</div>}
            <div className="chunks-quote selectable">
              {e.text.slice(0, 300)}
              {e.text.length > 300 && '…'}
            </div>
            {open?.chunkId === e.chunkId && <PdfHighlight documentId={e.documentId} spans={e.spans} />}
          </li>
        ))}
      </ul>
    </section>
  )
}
