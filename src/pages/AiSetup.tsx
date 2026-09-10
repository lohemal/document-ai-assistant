import { useCallback, useEffect, useRef, useState } from 'react'
import { openUrl } from '@tauri-apps/plugin-opener'
import {
  aiCancelPull,
  aiInstallHelp,
  aiPullModel,
  aiStatus,
  aiTestModel,
  gb,
  onPullProgress,
  type AiStatus,
  type InstallHelp,
  type ModelSpec,
  type PullEvent,
} from '@/ipc/ai'
import { message } from '@/lib/err'

/**
 * AI 기능 설치 안내.
 *
 * 이 화면은 **없어도 앱이 도는 기능**을 준비하는 곳이다. 그래서 맨 위에
 * "지금도 되는 일" 을 먼저 적는다 — 여기서 막혀도 프로그램을 못 쓰는 것이
 * 아니라는 것을 사용자가 알아야 한다.
 *
 * 기술 용어(Ollama)는 아래 상세에만 적는다.
 */
export default function AiSetup() {
  const [status, setStatus] = useState<AiStatus | null>(null)
  const [help, setHelp] = useState<InstallHelp | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [checking, setChecking] = useState(false)
  const [pulling, setPulling] = useState<PullEvent | null>(null)
  const [tested, setTested] = useState<Record<string, string>>({})
  const [copied, setCopied] = useState(false)
  const unlisten = useRef<(() => void) | null>(null)
  /** 받기 속도를 재는 표본 (시각, 받은 바이트) */
  const samples = useRef<{ t: number; b: number }[]>([])
  const [eta, setEta] = useState<{ bps: number; left: number } | null>(null)

  const refresh = useCallback(async () => {
    setChecking(true)
    try {
      setStatus(await aiStatus())
      setError(null)
    } catch (e) {
      setError(message(e))
    } finally {
      setChecking(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
    aiInstallHelp().then(setHelp).catch(() => {})
    onPullProgress((e) => {
      setPulling(e.done ? null : e)
      // 남은 시간 — 최근 20초 남짓의 실제 속도로 어림한다. 크기가 큰 모델(qwen3:8b 5.2GB,
      // bge-m3 1.2GB)은 몇 분 걸리므로, 멈춘 것으로 오해하지 않게 숫자를 보여 준다.
      if (e.done || e.totalBytes === 0) {
        samples.current = []
        setEta(null)
        return
      }
      const now = Date.now()
      samples.current.push({ t: now, b: e.completedBytes })
      samples.current = samples.current.filter((s) => now - s.t < 25_000)
      const first = samples.current[0]
      if (first && now - first.t > 3_000 && e.completedBytes > first.b) {
        const bps = ((e.completedBytes - first.b) * 1000) / (now - first.t)
        const left = (e.totalBytes - e.completedBytes) / bps
        setEta({ bps, left })
      }
    }).then((un) => {
      unlisten.current = un
    })
    return () => unlisten.current?.()
  }, [refresh])

  async function onPull(m: ModelSpec) {
    setError(null)
    setPulling({
      modelId: m.id,
      tag: m.tag,
      step: '시작하는 중',
      status: '',
      completedBytes: 0,
      totalBytes: 0,
      percent: null,
      done: false,
    })
    try {
      await aiPullModel(m.id)
      await refresh()
      // 받은 것만으로 끝내지 않는다. 정말 도는지 시켜 본다.
      await onTest(m)
    } catch (e) {
      setError(message(e))
    } finally {
      setPulling(null)
    }
  }

  async function onTest(m: ModelSpec) {
    try {
      const r = await aiTestModel(m.id)
      setTested((t) => ({ ...t, [m.id]: (r.ok ? '✓ ' : '✗ ') + r.detail }))
    } catch (e) {
      setTested((t) => ({ ...t, [m.id]: '✗ ' + message(e) }))
    }
  }

  const installed = (m: ModelSpec) =>
    status?.installed.some((i) => i.tag === m.tag || i.tag.split(':')[0] === m.tag.split(':')[0])

  const ready = status?.engine === 'ready'

  return (
    <div className="page">
      <h1 className="page-title">AI 기능 설치</h1>

      {/* 여기서 막혀도 프로그램을 못 쓰는 것이 아니라는 것을 먼저 말한다 */}
      <section className="card card-calm">
        <h2 className="card-title">AI 없이도 되는 일</h2>
        <ul className="pill-list">
          {(status?.worksWithoutAi ?? []).map((w) => (
            <li key={w} className="pill">
              {w}
            </li>
          ))}
        </ul>
        <p>
          AI 답변 기능을 쓰려면 <strong>로컬 AI 실행환경과 AI 모델</strong>이 필요합니다. 등록한
          업무자료와 AI 처리는 <strong>이 PC 에서</strong> 이루어집니다. 모델 설치에는 최초 1회
          인터넷 연결과 저장 공간이 필요합니다.
        </p>
      </section>

      {error && <p className="banner banner-error">{error}</p>}

      {/* ── 1단계: 실행환경 ─────────────────────────────────────── */}
      <section className="card">
        <div className="step-head">
          <h2 className="card-title">1. AI 실행환경</h2>
          <span className={'statuschip chip-' + (ready ? 'ok' : 'off')}>
            {ready ? '준비됨' : '준비 안 됨'}
          </span>
          <button className="btn btn-tiny" onClick={() => void refresh()} disabled={checking}>
            {checking ? '확인 중…' : '다시 확인'}
          </button>
        </div>

        {status && (
          <>
            <p>{status.detail}</p>
            {status.hint && <p className="muted">{status.hint}</p>}
          </>
        )}

        {status && (status.engine === 'not_installed' || status.engine === 'not_running') && help && (
          <div className="installbox">
            {status.engine === 'not_installed' ? (
              <>
                <div className="row-form">
                  <button className="btn btn-primary" onClick={() => void openUrl(help.downloadUrl)}>
                    AI 실행환경 내려받기
                  </button>
                  <span className="muted">공식 내려받기 쪽이 웹 브라우저로 열립니다.</span>
                </div>

                {help.hasWinget && (
                  <div className="row-form">
                    <code className="cmd">{help.wingetCommand}</code>
                    <button
                      className="btn btn-tiny"
                      onClick={() => {
                        void navigator.clipboard.writeText(help.wingetCommand)
                        setCopied(true)
                        setTimeout(() => setCopied(false), 1500)
                      }}
                    >
                      {copied ? '복사했습니다' : '명령 복사'}
                    </button>
                    <span className="muted">명령 프롬프트에 붙여 넣어도 됩니다.</span>
                  </div>
                )}

                <p className="muted small">{help.whyManual}</p>
                <p className="muted small">{help.offlineNote}</p>
              </>
            ) : (
              <p className="muted small">
                설치는 되어 있습니다{status.binaryPath && <> ({status.binaryPath})</>}. 시작 메뉴에서
                실행한 뒤 [다시 확인] 을 눌러 주세요.
              </p>
            )}
          </div>
        )}
      </section>

      {/* ── 2단계: 모델 ─────────────────────────────────────────── */}
      <section className={'card' + (ready ? '' : ' card-dim')}>
        <div className="step-head">
          <h2 className="card-title">2. AI 모델</h2>
          <span
            className={'statuschip chip-' + (status?.chatReady && status?.embedReady ? 'ok' : 'off')}
          >
            {status?.chatReady && status?.embedReady ? '준비됨' : '준비 안 됨'}
          </span>
        </div>

        {status && (
          <p className="muted">
            {status.recommendReason} 권장: <strong>{status.recommendedChat.name}</strong> ·{' '}
            <strong>{status.recommendedEmbed.name}</strong>
            {status.freeGb !== null && <> · 남은 저장공간 {status.freeGb.toFixed(1)}GB</>}
          </p>
        )}

        {!ready && <p className="muted">실행환경이 준비되면 여기서 모델을 받을 수 있습니다.</p>}

        {pulling && (
          <div className="progress">
            <div className="progress-line">
              {pulling.step}
              {pulling.totalBytes > 0 && (
                <>
                  {' '}
                  — {gb(pulling.completedBytes)} / {gb(pulling.totalBytes)}
                </>
              )}
            </div>
            <div className="progress-bar">
              <div
                className="progress-fill"
                style={{ width: pulling.percent !== null ? `${pulling.percent}%` : '8%' }}
              />
            </div>
            <span className="muted">
              {pulling.percent !== null ? `${pulling.percent}%` : ''}
              {eta && (
                <>
                  {' '}
                  · {(eta.bps / 1024 / 1024).toFixed(1)}MB/s · 남은 시간 약{' '}
                  {eta.left < 90 ? `${Math.max(5, Math.round(eta.left / 5) * 5)}초` : `${Math.ceil(eta.left / 60)}분`}
                </>
              )}
            </span>
            <button className="btn btn-tiny" onClick={() => void aiCancelPull()}>
              멈추기
            </button>
            <span className="muted small">멈춰도 받은 부분은 남아, 다시 누르면 이어서 받습니다.</span>
          </div>
        )}

        {status && (
          <ul className="modellist">
            {status.models.map((m) => {
              const rec =
                m.id === status.recommendedChat.id || m.id === status.recommendedEmbed.id
              const has = installed(m)
              return (
                <li key={m.id} className={'modelitem' + (rec ? ' is-rec' : '')}>
                  <div className="modelitem-main">
                    <div className="modelitem-name">
                      {m.name}
                      <span className="tag tag-role">{m.role === 'chat' ? '답변' : '검색'}</span>
                      {rec && <span className="tag tag-rec">권장</span>}
                      {has && <span className="tag tag-ok">받음</span>}
                    </div>
                    <div className="modelitem-note">{m.note}</div>
                    <div className="modelitem-meta muted">
                      약 {m.downloadGb}GB · 메모리 {m.minRamGb}GB 이상 권장
                    </div>
                    {tested[m.id] && <div className="modelitem-test">{tested[m.id]}</div>}
                  </div>
                  <div className="list-actions">
                    {has ? (
                      <button className="btn btn-tiny" onClick={() => void onTest(m)} disabled={!ready}>
                        확인
                      </button>
                    ) : (
                      <button
                        className="btn btn-tiny btn-primary"
                        onClick={() => void onPull(m)}
                        disabled={!ready || pulling !== null}
                      >
                        받기
                      </button>
                    )}
                  </div>
                </li>
              )
            })}
          </ul>
        )}

      </section>

      {/* ── 상세 (기술 용어는 여기에만) ─────────────────────────── */}
      <details className="card">
        <summary className="card-title">상세 정보</summary>
        <dl className="kv">
          <dt>AI 엔진</dt>
          <dd>
            {status?.engineName ?? '—'}
            {status?.engineVersion && <> {status.engineVersion}</>}
          </dd>
          <dt>연결 주소</dt>
          <dd>
            <code>http://127.0.0.1:11434</code> (이 PC 안)
          </dd>
          <dt>실행 파일</dt>
          <dd>{status?.binaryPath ?? '찾지 못함'}</dd>
          <dt>메모리</dt>
          <dd>{status?.ramGb ? `${status.ramGb}GB` : '알 수 없음'}</dd>
          <dt>winget</dt>
          <dd>{status?.hasWinget ? '있음' : '없음'}</dd>
          <dt>받아 둔 모델</dt>
          <dd>
            {status?.installed.length
              ? status.installed.map((i) => `${i.tag} (${gb(i.sizeBytes)})`).join(', ')
              : '없음'}
          </dd>
        </dl>
      </details>
    </div>
  )
}
