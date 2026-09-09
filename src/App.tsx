import { useState } from 'react'
import { Navigate, Route, Routes } from 'react-router-dom'
import Sidebar from '@/components/Sidebar'
import Placeholder from '@/components/Placeholder'
import Settings from '@/pages/Settings'
import { AI_NONE, type AiStatus } from '@/lib/aiStatus'

export default function App() {
  // P4b 에서 Ollama 를 실제로 확인해 채운다. 그때까지는 "AI 없음" 상태로 둔다 —
  // AI 가 없는 상태가 이 프로그램의 정상 상태 중 하나이기 때문이다 (설계안 2-12).
  const [ai] = useState<AiStatus>(AI_NONE)

  return (
    <div className="app">
      <Sidebar ai={ai} />
      <main className="content">
        <Routes>
          <Route path="/" element={<Navigate to="/collections" replace />} />
          <Route
            path="/collections"
            element={<Placeholder title="자료집 관리" phase="P1" note="자료집을 만들고 이름을 바꾸고 지웁니다." />}
          />
          <Route
            path="/documents"
            element={
              <Placeholder
                title="자료 등록"
                phase="P2"
                note="PDF 를 등록하고 텍스트를 뽑습니다. AI 모델 없이도 됩니다."
              />
            }
          />
          <Route
            path="/search"
            element={
              <Placeholder
                title="자료 검색"
                phase="P4a"
                note="낱말로 찾기(AI 없이) → 뜻으로 찾기(임베딩 모델 필요) 순서로 만듭니다."
              />
            }
          />
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
