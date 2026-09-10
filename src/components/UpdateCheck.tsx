import { useState } from 'react'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { message } from '@/lib/err'

type Phase =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'none'; version: string }
  | { kind: 'found'; update: Update }
  | { kind: 'downloading'; done: number; total: number | null }
  | { kind: 'installed' }
  | { kind: 'error'; text: string }

/**
 * 프로그램 업데이트 (P8).
 *
 * **사용자가 [업데이트 확인]을 눌렀을 때만** GitHub 으로 나간다 — 앱을 켠다고 나가지
 * 않는다 (설계안 8-3). 받은 파일은 서명이 맞아야만 설치된다(업데이터 플러그인이 pubkey 로
 * 검증). 실패는 어느 단계였는지 그대로 보여 준다 — "확인 못 함" 과 "받다 끊김" 은 다르다.
 */
export default function UpdateCheck({ version }: { version: string }) {
  const [phase, setPhase] = useState<Phase>({ kind: 'idle' })

  async function run() {
    setPhase({ kind: 'checking' })
    try {
      const u = await check({ timeout: 15_000 })
      if (!u) setPhase({ kind: 'none', version })
      else setPhase({ kind: 'found', update: u })
    } catch (e) {
      setPhase({
        kind: 'error',
        text:
          '업데이트 정보를 가져오지 못했습니다. 인터넷 연결을 확인하거나, 아직 배포된 버전이 없을 수 있습니다. ' +
          `(${message(e)})`,
      })
    }
  }

  async function install(u: Update) {
    setPhase({ kind: 'downloading', done: 0, total: null })
    let done = 0
    try {
      await u.downloadAndInstall((ev) => {
        if (ev.event === 'Started') setPhase({ kind: 'downloading', done: 0, total: ev.data.contentLength ?? null })
        else if (ev.event === 'Progress') {
          done += ev.data.chunkLength
          setPhase((p) => (p.kind === 'downloading' ? { ...p, done } : p))
        } else if (ev.event === 'Finished') setPhase({ kind: 'installed' })
      })
      setPhase({ kind: 'installed' })
    } catch (e) {
      setPhase({
        kind: 'error',
        text: `업데이트를 받거나 설치하지 못했습니다. 지금 버전은 그대로 쓸 수 있습니다. (${message(e)})`,
      })
    }
  }

  return (
    <section className="card">
      <h2 className="card-title">프로그램 업데이트</h2>
      <p className="muted">
        지금 버전 <strong>{version}</strong>. [업데이트 확인]을 누를 때만 GitHub 에 확인합니다 — 업무자료는
        보내지 않습니다. 새 버전은 서명이 맞을 때만 설치됩니다.
      </p>
      <div className="row">
        <button className="btn" disabled={phase.kind === 'checking' || phase.kind === 'downloading'} onClick={() => void run()}>
          {phase.kind === 'checking' ? '확인 중…' : '업데이트 확인'}
        </button>
        {phase.kind === 'none' && <span className="muted">최신 버전입니다.</span>}
        {phase.kind === 'found' && (
          <>
            <span>
              새 버전 <strong>{phase.update.version}</strong>이 있습니다.
            </span>
            <button className="btn btn-primary" onClick={() => void install(phase.update)}>
              받아서 설치
            </button>
          </>
        )}
        {phase.kind === 'downloading' && (
          <span className="muted">
            받는 중… {(phase.done / 1024 / 1024).toFixed(1)}MB
            {phase.total ? ` / ${(phase.total / 1024 / 1024).toFixed(1)}MB` : ''}
          </span>
        )}
        {phase.kind === 'installed' && <span>설치했습니다. 프로그램을 다시 시작하면 새 버전이 됩니다.</span>}
      </div>
      {phase.kind === 'found' && phase.update.body && <pre className="raw">{phase.update.body}</pre>}
      {phase.kind === 'error' && <p className="banner banner-warn small">{phase.text}</p>}
    </section>
  )
}
