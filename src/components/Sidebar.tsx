import { NavLink } from 'react-router-dom'
import type { AiStatus } from '@/ipc/ai'

type Group = { title: string; items: { to: string; label: string }[] }

const GROUPS: Group[] = [
  {
    title: '📚 자료집',
    items: [
      { to: '/collections', label: '자료집 관리' },
      { to: '/documents', label: '자료 등록' },
    ],
  },
  {
    title: '🤖 AI 업무',
    items: [
      { to: '/search', label: '자료 검색' },
      { to: '/ask', label: '규정 해석' },
      { to: '/draft', label: '문서 작성' },
    ],
  },
  {
    title: '🕘 작업 기록',
    items: [{ to: '/jobs', label: '최근 · 중요' }],
  },
  {
    title: '⚙ 설정',
    items: [
      { to: '/settings', label: '설정' },
      { to: '/settings/ai', label: 'AI 기능 설치' },
    ],
  },
]

/**
 * 아래 상태줄은 **늘 무엇이 되는지**를 말한다.
 * AI 가 없어도 "낱말로 찾기는 됩니다" 를 붙여, 프로그램이 못 쓰는 상태가
 * 아니라는 것을 알린다 (설계안 2-12).
 */
function statusLine(ai: AiStatus | null): { dot: string; text: string; tone: string } {
  if (!ai) return { dot: '·', text: 'AI 상태 확인 중…', tone: 'off' }
  if (ai.engine !== 'ready') {
    return { dot: '○', text: 'AI 없음 — 낱말로 찾기는 됩니다', tone: 'off' }
  }
  if (ai.chatReady && ai.embedReady) return { dot: '●', text: 'AI 준비됨', tone: 'ok' }
  if (ai.embedReady) return { dot: '◑', text: '검색 모델만 있음', tone: 'partial' }
  if (ai.chatReady) return { dot: '◑', text: '답변 모델만 있음', tone: 'partial' }
  return { dot: '○', text: '모델 없음 — 낱말로 찾기는 됩니다', tone: 'off' }
}

export default function Sidebar({ ai }: { ai: AiStatus | null }) {
  const s = statusLine(ai)

  return (
    <nav className="sidebar">
      <div className="sidebar-brand">
        <div className="sidebar-brand-name">업무자료 AI 도우미</div>
        <div className="sidebar-brand-sub">등록한 자료에서 근거를 찾아 답합니다</div>
      </div>

      <div className="sidebar-groups">
        {GROUPS.map((g) => (
          <div className="sidebar-group" key={g.title}>
            <div className="sidebar-group-title">{g.title}</div>
            {g.items.map((it) => (
              <NavLink
                key={it.to}
                to={it.to}
                end={it.to === '/settings'}
                className={({ isActive }) => 'sidebar-link' + (isActive ? ' is-active' : '')}
              >
                {it.label}
              </NavLink>
            ))}
          </div>
        ))}
      </div>

      <div className={'sidebar-status tone-' + s.tone}>
        <span className="sidebar-status-dot">{s.dot}</span>
        <span>{s.text}</span>
      </div>
    </nav>
  )
}
