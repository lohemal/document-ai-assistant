import type { AnswerOut } from '@/ipc/answer'

/**
 * 초안 본문 — **확인되지 않은 것을 눈에 띄게.**
 *
 * 문서는 그대로 나가는 글이다. 인용 근거에서 확인하지 못한 숫자와, 어느 주장에도
 * 적히지 않은 문장(근거 없이 쓴 문장)은 본문 안에서 바로 표시한다. 복사할 때는
 * 표시 없이 글만 복사된다 — 그래서 복사 단추 옆에 같은 내용을 한 줄로 다시 적는다.
 */
export default function DraftBody({ out }: { out: AnswerOut }) {
  const badNumbers = (out.verdict?.numbers ?? []).filter((n) => !n.found).map((n) => n.raw)
  const uncovered = out.uncovered

  // 문장 단위로 나눠, 근거 없는 문장은 통째로 표시하고, 그 안의 숫자도 표시한다
  const parts = out.answer.split(/(?<=[.!?])\s+|\n/)

  return (
    <pre className="draft-body selectable">
      {parts.map((sentence, i) => {
        const bare = sentence.trim().replace(/[.!?]$/, '').replace(/[다요]$/, '')
        const isUncovered = bare.length > 0 && uncovered.some((u) => u === bare || bare.includes(u) || u.includes(bare))
        return (
          <span key={i} className={isUncovered ? 'draft-uncovered' : undefined} title={isUncovered ? '근거에서 온 주장에 없는 문장입니다' : undefined}>
            {markNumbers(sentence, badNumbers)}
            {i < parts.length - 1 ? '\n' : ''}
          </span>
        )
      })}
    </pre>
  )
}

/** 확인되지 않은 숫자 조각을 표시로 감싼다 */
function markNumbers(text: string, bad: string[]) {
  if (bad.length === 0) return text
  const escaped = bad.map((b) => b.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
  const re = new RegExp(`(${escaped.join('|')})`, 'g')
  return text.split(re).map((piece, i) =>
    bad.includes(piece) ? (
      <mark key={i} className="draft-badnum" title="인용 근거에서 확인하지 못한 숫자입니다">
        {piece}
      </mark>
    ) : (
      piece
    ),
  )
}
