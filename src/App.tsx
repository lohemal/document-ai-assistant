import { useCallback, useEffect, useRef, useState } from 'react'
import { Navigate, Route, Routes, useLocation } from 'react-router-dom'
import Sidebar from '@/components/Sidebar'
import Placeholder from '@/components/Placeholder'
import Ask from '@/pages/Ask'
import AiGate from '@/components/AiGate'
import Collections from '@/pages/Collections'
import Documents from '@/pages/Documents'
import Search from '@/pages/Search'
import Settings from '@/pages/Settings'
import AiSetup from '@/pages/AiSetup'
import { getAppInfo, type AppInfo } from '@/ipc/app'
import { aiStatus, type AiStatus } from '@/ipc/ai'

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null)
  const [ai, setAi] = useState<AiStatus | null>(null)
  const where = useLocation()

  useEffect(() => {
    getAppInfo().then(setInfo).catch(() => setInfo(null))
  }, [])

  /**
   * AI 상태를 알아본다.
   *
   * **여기서 실패해도 앱은 그대로 돈다.** Ollama 가 없거나 말썽이어도 화면이
   * 하얗게 되면 안 된다 — 자료집·등록·낱말 검색은 AI 와 아무 상관이 없다.
   */
  const refreshAi = useCallback(() => {
    aiStatus()
      .then(setAi)
      .catch(() => setAi(null))
  }, [])

  useEffect(refreshAi, [refreshAi])

  // AI 화면을 들렀다 **나올 때만** 다시 본다.
  //
  // 화면을 옮길 때마다 확인하지 않는 까닭은, 실행환경이 없을 때 붙어 보는 데
  // 2초쯤 걸리기 때문이다 (Windows 가 거부를 늦게 알려 준다 — ollama.rs 참고).
  const wasOnAiPage = useRef(false)
  useEffect(() => {
    const onAi = where.pathname.startsWith('/settings/ai')
    if (wasOnAiPage.current && !onAi) refreshAi()
    wasOnAiPage.current = onAi
  }, [where.pathname, refreshAi])

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
          <Route path="/ask" element={<Ask />} />
          <Route
            path="/draft"
            element={
              <div className="page">
                <h1 className="page-title">문서 작성</h1>
                <AiGate status={ai} need="chat" what="문서 초안">
                  <Placeholder title="문서 작성" phase="P7" note="가정통신문 · 문자메시지 초안." />
                </AiGate>
              </div>
            }
          />
          <Route path="/jobs" element={<Placeholder title="작업 기록" phase="P6" note="최근 작업과 중요 기록." />} />
          <Route path="/settings/ai" element={<AiSetup />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="*" element={<Navigate to="/collections" replace />} />
        </Routes>
      </main>
    </div>
  )
}
