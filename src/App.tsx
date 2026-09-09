import { useEffect, useState } from 'react'
import { Navigate, Route, Routes } from 'react-router-dom'
import Sidebar from '@/components/Sidebar'
import Placeholder from '@/components/Placeholder'
import Collections from '@/pages/Collections'
import Documents from '@/pages/Documents'
import Search from '@/pages/Search'
import Settings from '@/pages/Settings'
import { getAppInfo, type AppInfo } from '@/ipc/app'
import { AI_NONE, type AiStatus } from '@/lib/aiStatus'

export default function App() {
  // P4b 에서 Ollama 를 실제로 확인해 채운다. 그때까지는 "AI 없음" 상태로 둔다 —
  // AI 가 없는 상태가 이 프로그램의 정상 상태 중 하나이기 때문이다 (설계안 2-12).
  const [ai] = useState<AiStatus>(AI_NONE)
  const [info, setInfo] = useState<AppInfo | null>(null)

  useEffect(() => {
    getAppInfo().then(setInfo).catch(() => setInfo(null))
  }, [])

  return (
    <div className="app">
      <Sidebar ai={ai} />
      <main className="content">
        {/* 자료를 열지 못했다면 무엇을 하든 안 되므로 맨 위에서 알린다. */}
        {info && !info.storageReady && (
          <div className="banner banner-error banner-top">
            <strong>자료를 열지 못했습니다.</strong>
            <div>{info.storageError}</div>
            <div className="muted">자료 폴더: {info.dataDir}</div>
          </div>
        )}

        <Routes>
          <Route path="/" element={<Navigate to="/collections" replace />} />
          <Route path="/collections" element={<Collections />} />
          <Route path="/documents" element={<Documents />} />
          <Route path="/search" element={<Search />} />
          <Route
            path="/ask"
            element={<Placeholder title="규정 해석" phase="P5" note="근거를 읽고 답합니다. 답변 모델이 필요합니다." />}
          />
          <Route
            path="/draft"
            element={<Placeholder title="문서 작성" phase="P7" note="가정통신문 · 문자메시지 초안." />}
          />
          <Route path="/jobs" element={<Placeholder title="작업 기록" phase="P6" note="최근 작업과 중요 기록." />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="*" element={<Navigate to="/collections" replace />} />
        </Routes>
      </main>
    </div>
  )
}
