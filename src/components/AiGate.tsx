import { Link } from 'react-router-dom'
import { blockedReason, type AiStatus } from '@/ipc/ai'

/**
 * AI 가 있어야 하는 화면 앞에 세우는 안내.
 *
 * **AI 가 없다고 앱이 막히지는 않는다.** 이 안내는 그 기능을 눌렀을 때만
 * 나오고, 지금도 되는 일을 함께 알려 준다 (설계안 2-12).
 */
export default function AiGate({
  status,
  need,
  what,
  children,
}: {
  status: AiStatus | null
  need: 'embed' | 'chat'
  /** 이 화면이 하려는 일 ("AI 답변") */
  what: string
  children?: React.ReactNode
}) {
  const blocked = blockedReason(status, need)
  if (!blocked) return <>{children}</>

  return (
    <div className="aigate">
      <div className="aigate-title">{what} 기능을 쓰려면 AI 설치가 필요합니다</div>
      <p className="aigate-why">{blocked}</p>
      <p className="muted">
        등록한 업무자료와 AI 처리는 이 PC 에서 이루어집니다. 모델 설치에는 최초 1회 인터넷 연결과
        저장 공간이 필요합니다.
      </p>

      <div className="aigate-actions">
        <Link className="btn btn-primary" to="/settings/ai">
          AI 기능 설치
        </Link>
        <Link className="btn" to="/search">
          낱말로 찾기로 가기
        </Link>
      </div>

      {status && status.worksWithoutAi.length > 0 && (
        <div className="aigate-still">
          지금도 되는 일:{' '}
          {status.worksWithoutAi.map((w) => (
            <span className="pill" key={w}>
              {w}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}
