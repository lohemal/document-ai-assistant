import { useEffect, useState } from 'react'
import { listCollections, type Collection } from '@/ipc/collections'
import { modeLabel, searchQuery, type Hit, type SearchResult } from '@/ipc/search'
import { message } from '@/lib/err'
import PdfHighlight from '@/components/PdfHighlight'

/**
 * 찾기. **사용자는 방식을 고르지 않는다.**
 *
 * 의미 색인이 있고 AI 가 돌면 섞어 찾기(Hybrid), 아니면 낱말로 찾기.
 * 어느 쪽이었는지는 결과 위에 작게 적어 준다 — 품질이 갑자기 달라졌을 때
 * 까닭을 알 수 있어야 한다.
 *
 * **AI 모델이 없어도 된다** (설계안 2-12).
 *
 * 결과를 누르면 P3 의 형광펜으로 이어진다 —
 * 검색 → 청크 → 문서·쪽 → 원문 → 그 자리 형광펜.
 * 이 흐름을 P5 의 근거 카드에서 그대로 다시 쓴다.
 */
export default function Search() {
  const [collections, setCollections] = useState<Collection[] | null>(null)
  const [collectionId, setCollectionId] = useState<number | 'all'>('all')
  const [text, setText] = useState('')
  const [result, setResult] = useState<SearchResult | null>(null)
  const [picked, setPicked] = useState<Hit | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  /** 점수는 일반 사용자에게 뜻이 없다. 개발할 때만 켠다. */
  const [showScore, setShowScore] = useState(false)

  useEffect(() => {
    listCollections()
      .then(setCollections)
      .catch((e) => setError(message(e)))
  }, [])

  async function run(e?: React.FormEvent) {
    e?.preventDefault()
    if (!text.trim()) return
    setBusy(true)
    setPicked(null)
    try {
      const r = await searchQuery(text, collectionId === 'all' ? [] : [collectionId])
      setResult(r)
      setPicked(r.hits[0] ?? null)
      setError(null)
    } catch (err) {
      setError(message(err))
    } finally {
      setBusy(false)
    }
  }

  const empty = result !== null && result.hits.length === 0

  return (
    <div className="page page-wide">
      <h1 className="page-title">자료 검색</h1>
      <p className="page-lead">
        등록한 자료에서 근거를 찾습니다. <strong>AI 모델이 없어도</strong> 낱말로 찾습니다.
      </p>

      <form className="searchbar" onSubmit={run}>
        <select
          className="input"
          value={collectionId}
          onChange={(e) => setCollectionId(e.target.value === 'all' ? 'all' : Number(e.target.value))}
        >
          <option value="all">전체 자료</option>
          {collections?.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
        <input
          className="input"
          placeholder="찾을 말이나 질문 (예: 초등학교 3학년 지원액)"
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
        <button className="btn btn-primary" disabled={busy || !text.trim()}>
          {busy ? '찾는 중…' : '찾기'}
        </button>
      </form>

      {error && <p className="banner banner-error">{error}</p>}

      {result && (
        <div className="searchmeta">
          <span>
            {result.hits.length > 0
              ? `${result.hits.length}건 · ${result.elapsedMs}ms`
              : `찾지 못했습니다 · ${result.elapsedMs}ms`}
          </span>
          <span className={'tag tag-' + (result.mode === 'hybrid' ? 'ok' : 'quiet')}>
            검색 방식: {modeLabel(result.mode)}
          </span>
          {result.terms.length > 0 && (
            <span className="muted">찾아 본 말: {result.terms.join(', ')}</span>
          )}
          {result.shortTerms.length > 0 && (
            <span className="muted">훑어서 찾음: {result.shortTerms.join(', ')}</span>
          )}
          {result.droppedTerms.length > 0 && (
            <span className="muted">뺀 말: {result.droppedTerms.join(', ')}</span>
          )}
          <label className="scoretoggle">
            <input
              type="checkbox"
              checked={showScore}
              onChange={(e) => setShowScore(e.target.checked)}
            />
            점수 보기
          </label>
        </div>
      )}

      {result?.modeNote && <p className="banner banner-info small">{result.modeNote}</p>}

      {empty && (
        <div className="empty">
          <p>등록된 자료에서 해당 내용을 확인할 수 없습니다.</p>
          {result?.note && <p className="muted">{result.note}</p>}
          {!result?.note && (
            <p className="muted">
              다른 낱말로 찾아 보세요. 자료집을 좁혀 두었다면 넓혀 볼 수도 있습니다.
            </p>
          )}
        </div>
      )}

      {result && result.hits.length > 0 && (
        <div className="chunks-split">
          <ul className="chunklist">
            {result.hits.map((h) => (
              <li key={h.chunkId}>
                <button
                  className={'chunkitem' + (picked?.chunkId === h.chunkId ? ' is-on' : '')}
                  onClick={() => setPicked(h)}
                >
                  <span className="chunkitem-head">
                    <b>{h.rank}.</b>
                    <span className="hit-doc">{h.docTitle}</span>
                    <span className="muted">
                      {h.pageStart === h.pageEnd ? `${h.pageStart}쪽` : `${h.pageStart}~${h.pageEnd}쪽`}
                    </span>
                    {h.kind === 'table' && <span className="tag tag-warn">표</span>}
                  </span>
                  {h.headingPath && <span className="chunkitem-path">{h.headingPath}</span>}
                  <span className="chunkitem-body">{h.text.slice(0, 100).replace(/\n/g, ' ')}</span>
                  <span className="hit-why">
                    <span className={'tag tag-' + (h.relevance === '높음' ? 'ok' : 'quiet')}>
                      관련도 {h.relevance}
                    </span>
                    {h.matchedTerms.length > 0 && <> 걸린 말: {h.matchedTerms.join(', ')}</>}
                    {showScore && (
                      <span className="muted">
                        {' '}· 낱말 {h.keywordRank ?? '—'}등 · 뜻 {h.semanticRank ?? '—'}등
                        {result.mode === 'hybrid' ? (
                          <> · RRF {h.score.toFixed(4)}</>
                        ) : (
                          <>
                            {' '}
                            · 든 낱말 {h.matched}개 · BM25 {h.bm25.toFixed(2)}
                          </>
                        )}
                      </span>
                    )}
                  </span>
                </button>
              </li>
            ))}
          </ul>

          <div className="chunks-view">
            {picked ? (
              <>
                <div className="hit-head">
                  <strong>{picked.docTitle}</strong>{' '}
                  <span className="muted">
                    {picked.pageStart === picked.pageEnd
                      ? `${picked.pageStart}쪽`
                      : `${picked.pageStart}~${picked.pageEnd}쪽`}
                  </span>
                </div>
                <div className="chunks-quote selectable">
                  {picked.text.slice(0, 400)}
                  {picked.text.length > 400 && '…'}
                </div>
                <PdfHighlight documentId={picked.documentId} spans={picked.spans} />
              </>
            ) : (
              <p className="muted">왼쪽에서 결과를 고르세요.</p>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
