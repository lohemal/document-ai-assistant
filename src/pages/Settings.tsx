import { useEffect, useState } from 'react'
import { getAppInfo, type AppInfo } from '@/ipc/app'

export default function Settings() {
  const [info, setInfo] = useState<AppInfo | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    getAppInfo().then(setInfo).catch((e) => setError(String(e)))
  }, [])

  return (
    <div className="page">
      <h1 className="page-title">설정</h1>

      <section className="card">
        <h2 className="card-title">프로그램</h2>
        {error && <p className="error">{error}</p>}
        {info && (
          <dl className="kv">
            <dt>이름</dt>
            <dd>{info.displayName}</dd>
            <dt>버전</dt>
            <dd>{info.version}</dd>
            <dt>자료 폴더</dt>
            <dd>
              <code>{info.dataDir}</code>
            </dd>
            <dt>자료 상태</dt>
            <dd>{info.storageReady ? '정상' : (info.storageError ?? '열지 못했습니다')}</dd>
          </dl>
        )}
      </section>

      <section className="card card-warn">
        <h2 className="card-title">자료가 어디에 있나요</h2>
        <p>
          등록한 문서와 검색 자료는 위 폴더 안에만 저장됩니다. 프로그램을 업데이트해도,
          지웠다 다시 깔아도 그대로 남습니다.
        </p>
        <p>
          <strong>이 폴더는 업무자료 그 자체입니다.</strong> 개인정보가 담긴 자료를 등록했다면
          이 폴더와 백업 파일도 같은 주의가 필요합니다.
        </p>
      </section>

      <section className="card">
        <h2 className="card-title">아직 만들지 않은 설정</h2>
        <ul className="todo">
          <li>AI 모델 선택 · 설치 — P4b</li>
          <li>기록 보존기간 — P6</li>
          <li>자료 폴더 열기 · 백업 · 복원 — P8</li>
          <li>프로그램 업데이트 — P8</li>
        </ul>
      </section>
    </div>
  )
}
