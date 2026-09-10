import type { AnswerOut } from '@/ipc/answer'

/**
 * 검증 상태 — 인용 · 숫자 · 주장 뒷받침 · 해석 여부. 규정 해석과 문서 작성이 같은 부품.
 *
 * **"검증 완료" 라고 쓰지 않는다** (설계안 5-5). 숫자 확인은 값이 인용 근거 안에
 * 있는지만 본다. 통과는 신뢰의 근거로 쓰지 않고 실패만 경고로 쓴다.
 */
export default function VerificationCard({ out, what = '답변' }: { out: AnswerOut; what?: string }) {
  const v = out.verdict
  if (!v) return null

  // 해석·문체는 세지 않는다 — 근거의 말을 옮긴 것이 아니므로 낱말이 겹치지 않는 것이
  // 정상이다 (Rust 쪽 `unsupported_claims` 와 같은 자). 숫자는 문장 종류를 가리지 않고 본다.
  const facts = v.claims.filter((c) => c.kind === 'fact')
  const weak = facts.filter((c) => !c.supported)
  const styles = v.claims.filter((c) => c.kind === 'style').length

  return (
    <section className="card">
      <h2 className="card-title">검증 상태</h2>
      <ul className="checks">
        <li className={v.citationsOk ? 'ok' : 'bad'}>{v.citationMessage}</li>
        <li className={v.numbers.every((n) => n.found) ? 'ok' : 'bad'}>{v.numberMessage}</li>
        <li className={facts.length === 0 ? 'warn' : weak.length === 0 ? 'ok' : 'bad'}>
          {facts.length === 0
            ? `AI 가 사실 주장을 따로 적지 않아 뒷받침을 확인하지 못했습니다.`
            : weak.length === 0
              ? `${what}의 사실 주장 ${facts.length}개가 모두 인용한 근거 안에서 확인되었습니다.`
              : `사실 주장 ${weak.length}개를 인용한 근거에서 확인하지 못했습니다.`}
          {styles > 0 && (
            <span className="muted small"> (인사말·연결 문구 {styles}개는 사실이 아니라 검사하지 않습니다)</span>
          )}
        </li>
        <li className={out.interpretation ? 'warn' : 'ok'}>
          {out.interpretation ? '자료 해석이 포함되어 있습니다.' : '자료에 적힌 내용만 담겨 있습니다.'}
        </li>
      </ul>

      {v.numbers.length > 0 && (
        <table className="numtable">
          <tbody>
            {v.numbers.map((n) => (
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
        숫자 확인은 <strong>그 값이 인용한 근거 안에 있는지</strong>만 봅니다. 값이 그 자리에 맞게
        쓰였는지까지는 확인하지 못합니다 — 중요한 숫자는 근거 원문을 꼭 직접 확인해 주세요.
      </p>

      {out.judgement.reasons.length > 0 && (
        <ul className="notes">
          {out.judgement.reasons.map((r) => (
            <li key={r}>{r}</li>
          ))}
        </ul>
      )}
    </section>
  )
}
