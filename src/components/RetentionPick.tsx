import { useEffect, useState } from 'react'
import { jobRetention, purgeJobs, RETENTION_CHOICES, setJobRetention } from '@/ipc/jobs'
import { message } from '@/lib/err'

/**
 * 작업 기록 보존기간 (P6).
 *
 * 기본 30일. 중요 표시(★)한 기록은 기간이 지나도 지우지 않는다. 지우는 일은 앱을
 * 켠 뒤 몇 초 있다가 따로 돌므로 시작이 느려지지 않는다. 여기서 [지금 정리]를
 * 누르면 바로 돈다.
 */
export default function RetentionPick() {
  const [days, setDays] = useState<number | null>(null)
  const [note, setNote] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    jobRetention().then(setDays).catch((e) => setError(message(e)))
  }, [])

  async function change(v: number) {
    setError(null)
    setNote(null)
    try {
      await setJobRetention(v)
      setDays(v)
      setNote(v === 0 ? '기록을 직접 지울 때까지 남깁니다.' : `${v}일이 지난 기록은 자동으로 지웁니다 (중요 표시는 제외).`)
    } catch (e) {
      setError(message(e))
    }
  }

  async function purgeNow() {
    setError(null)
    try {
      const n = await purgeJobs()
      setNote(n === 0 ? '지울 기록이 없습니다.' : `기록 ${n}개를 지웠습니다.`)
    } catch (e) {
      setError(message(e))
    }
  }

  return (
    <section className="card">
      <h2 className="card-title">작업 기록 보존기간</h2>
      <p className="muted">
        물었던 것과 그때의 답·근거를 얼마나 오래 남길지 정합니다. <strong>중요 표시(★)한 기록은
        기간이 지나도 지우지 않습니다.</strong>
      </p>
      {error && <p className="error">{error}</p>}
      {days !== null && (
        <div className="row">
          <select className="input" value={days} onChange={(e) => void change(Number(e.target.value))}>
            {RETENTION_CHOICES.map((c) => (
              <option key={c.days} value={c.days}>
                {c.label}
              </option>
            ))}
          </select>
          <button className="btn" onClick={() => void purgeNow()}>
            지금 정리
          </button>
        </div>
      )}
      {note && <p className="muted small">{note}</p>}
    </section>
  )
}
