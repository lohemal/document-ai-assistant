import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { backupNow, getAppInfo, openDataDir, type AppInfo } from '@/ipc/app'
import { aiStatus, gb, type AiStatus } from '@/ipc/ai'
import { message } from '@/lib/err'
import EmbedModelPick from '@/components/EmbedModelPick'
import RetentionPick from '@/components/RetentionPick'
import UpdateCheck from '@/components/UpdateCheck'

/**
 * 설정 (P8 최종).
 *
 * 일반 사용자가 보는 것: 답변 모델 · 검색 모델 · RAM 과 권장 · AI 실행환경 상태 ·
 * 작업 기록 보존기간 · 자료가 어디 있는가 · 백업 · 업데이트 · 버전.
 * 개발용 값은 여기 없다 — 답변 화면의 [개발 정보 보기]에만 있다.
 */
export default function Settings() {
  const [info, setInfo] = useState<AppInfo | null>(null)
  const [ai, setAi] = useState<AiStatus | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [backup, setBackup] = useState<string | null>(null)

  useEffect(() => {
    getAppInfo().then(setInfo).catch((e) => setError(message(e)))
    aiStatus().then(setAi).catch(() => setAi(null))
  }, [])

  const chat = ai?.chatReady ? ai.models.find((m) => m.tag === ai.chatReady) : null
  const embed = ai?.embedReady ? ai.models.find((m) => m.tag === ai.embedReady) : null

  async function doBackup() {
    setBackup(null)
    setError(null)
    try {
      const r = await backupNow()
      setBackup(`${r.path} (${(r.bytes / 1024 / 1024).toFixed(1)}MB)`)
    } catch (e) {
      setError(message(e))
    }
  }

  return (
    <div className="page">
      <h1 className="page-title">설정</h1>
      {error && <p className="banner banner-error">{error}</p>}

      {/* ── AI ─────────────────────────────────────────────────── */}
      <section className="card">
        <h2 className="card-title">AI</h2>
        <dl className="kv">
          <dt>AI 실행환경</dt>
          <dd>
            {ai === null ? (
              '확인하지 못했습니다'
            ) : (
              <>
                <span className={'statuschip ' + (ai.engine === 'ready' ? 'chip-ok' : 'chip-off')}>
                  {ai.engine === 'ready' ? '준비됨' : '준비 안 됨'}
                </span>{' '}
                {ai.engineName}
                {ai.engineVersion ? ` ${ai.engineVersion}` : ''}
                {ai.engine !== 'ready' && <span className="muted"> — {ai.detail}</span>}
              </>
            )}
          </dd>
          <dt>답변 모델</dt>
          <dd>
            {chat ? `${chat.name} (${chat.tag})` : ai?.chatReady ? ai.chatReady : <span className="muted">없음</span>}
          </dd>
          <dt>검색 모델</dt>
          <dd>
            {embed ? `${embed.name} (${embed.tag})` : ai?.embedReady ? ai.embedReady : <span className="muted">없음</span>}
          </dd>
          <dt>이 PC 메모리</dt>
          <dd>
            {ai?.ramGb !== null && ai?.ramGb !== undefined ? `${ai.ramGb}GB` : '알 수 없음'}
            {ai && (
              <span className="muted">
                {' '}
                — 권장: {ai.recommendedChat.name}({ai.recommendedChat.tag}) · {ai.recommendedEmbed.name}(
                {ai.recommendedEmbed.tag})
              </span>
            )}
          </dd>
          {ai?.installed.length ? (
            <>
              <dt>받아 둔 모델</dt>
              <dd className="muted">{ai.installed.map((i) => `${i.tag} (${gb(i.sizeBytes)})`).join(', ')}</dd>
            </>
          ) : null}
        </dl>
        <p className="muted small">
          모델을 받거나 바꾸려면 <Link to="/settings/ai">AI 기능 설치</Link>로 가세요. 8GB PC 는 가벼운 답변
          모델(gemma3:4b), 16GB 이상은 기본 답변 모델(qwen3:8b)을 권합니다 — 이 값은 실제 자료로 재어 정한
          것입니다. AI 가 없어도 자료집 · PDF 등록 · 원문 보기 · 낱말 검색은 그대로 됩니다.
        </p>
      </section>

      <EmbedModelPick />

      <RetentionPick />

      {/* ── 자료가 어디 있나 ────────────────────────────────────── */}
      <section className="card">
        <h2 className="card-title">자료는 이 PC 에만 있습니다</h2>
        {info && (
          <dl className="kv">
            <dt>등록한 PDF</dt>
            <dd>
              <code>{info.filesDir}</code>
            </dd>
            <dt>검색 색인 · 작업 기록</dt>
            <dd>
              <code>{info.dbPath}</code>
            </dd>
            <dt>자동 백업</dt>
            <dd>
              <code>{info.backupsDir}</code>
            </dd>
            <dt>AI 모델</dt>
            <dd>
              <code>{info.modelsDir ?? '(AI 실행환경을 설치하면 생깁니다)'}</code>
              <span className="muted small"> — 모델 파일에는 업무자료가 들어가지 않습니다</span>
            </dd>
          </dl>
        )}
        <div className="row">
          <button className="btn" onClick={() => void openDataDir().catch((e) => setError(message(e)))}>
            자료 폴더 열기
          </button>
          <button className="btn" onClick={() => void doBackup()}>
            지금 백업 (data.db)
          </button>
        </div>
        {backup && <p className="muted small">백업했습니다: {backup}</p>}
        <div className="banner banner-warn small">
          <strong>이 폴더는 업무자료 그 자체입니다.</strong> 등록한 PDF 원문, 검색 색인, 물었던 것과 답이 모두
          여기 있습니다. 개인정보가 담긴 자료를 등록했다면 이 폴더와 백업 파일에도 같은 주의가 필요합니다. 이
          프로그램은 자료를 외부로 보내지 않지만, 폴더 자체를 암호화하지는 않습니다.
        </div>
        <p className="muted small">
          <strong>다른 PC 로 옮기거나 복구하려면</strong> — 프로그램을 끈 뒤 위 자료 폴더 전체(
          <code>data.db</code> 와 <code>files\</code>)를 복사해 새 PC 의 같은 자리에 두면 됩니다. [지금 백업]은{' '}
          <code>data.db</code> 만 <code>backups\</code> 에 복사합니다 — PDF 사본(<code>files\</code>)은 따로
          복사해야 합니다. 복원 화면은 다음 버전에 넣을 예정입니다.
        </p>
      </section>

      {info && <UpdateCheck version={info.version} />}

      {/* ── 프로그램 ──────────────────────────────────────────── */}
      <section className="card">
        <h2 className="card-title">프로그램</h2>
        {info && (
          <dl className="kv">
            <dt>이름</dt>
            <dd>{info.displayName}</dd>
            <dt>버전</dt>
            <dd>{info.version}</dd>
            <dt>자료 상태</dt>
            <dd>{info.storageReady ? '정상' : (info.storageError ?? '열지 못했습니다')}</dd>
          </dl>
        )}
      </section>
    </div>
  )
}
