import { useCallback, useEffect, useState } from 'react'
import { listChunks, type Chunk } from '@/ipc/chunks'
import { rebuildChunks } from '@/lib/chunk/rebuild'
import { message } from '@/lib/err'
import PdfHighlight, { forgetPdf } from './PdfHighlight'
import type { Document } from '@/ipc/documents'

/**
 * 청크를 고르면 그 자리가 원본 PDF 에서 어디인지 보여 준다.
 *
 * P4 에서 검색이 청크를 돌려주기 시작하면, 사용자는 바로 이 화면으로 근거를
 * 확인하게 된다. 그래서 검색을 붙이기 **전에** 이 연결이 맞는지 눈으로 봐 둔다.
 */
export default function ChunkExplorer({ doc }: { doc: Document }) {
  const [chunks, setChunks] = useState<Chunk[] | null>(null)
  const [picked, setPicked] = useState<Chunk | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const load = useCallback(async () => {
    try {
      const cs = await listChunks(doc.id)
      setChunks(cs)
      setPicked((prev) => cs.find((c) => c.id === prev?.id) ?? cs[0] ?? null)
      setError(null)
    } catch (e) {
      setError(message(e))
    }
  }, [doc.id])

  useEffect(() => {
    setChunks(null)
    setPicked(null)
    void load()
  }, [load])

  async function onRebuild() {
    setBusy(true)
    try {
      const s = await rebuildChunks(doc.id)
      forgetPdf(doc.id)
      await load()
      setError(null)
      if (s.count === 0) setError('나눌 글자가 없습니다.')
    } catch (e) {
      setError(message(e))
    } finally {
      setBusy(false)
    }
  }

  const sizes = (chunks ?? []).map((c) => c.text.length)
  const stat =
    sizes.length > 0
      ? `${sizes.length}개 · 평균 ${Math.round(sizes.reduce((a, b) => a + b, 0) / sizes.length)}자 · ` +
        `최소 ${Math.min(...sizes)} · 최대 ${Math.max(...sizes)}`
      : '없음'

  return (
    <div className="chunks">
      <div className="chunks-bar">
        <span className="muted">청크 {stat}</span>
        <button className="btn btn-tiny" onClick={() => void onRebuild()} disabled={busy}>
          {busy ? '나누는 중…' : '다시 나누기'}
        </button>
      </div>

      {error && <p className="banner banner-error">{error}</p>}

      {chunks !== null && chunks.length === 0 && (
        <div className="empty">
          <p>아직 나뉜 청크가 없습니다.</p>
          <p className="muted">[다시 나누기] 를 누르면 지금 규칙으로 나눕니다.</p>
        </div>
      )}

      {chunks !== null && chunks.length > 0 && (
        <div className="chunks-split">
          <ul className="chunklist">
            {chunks.map((c) => (
              <li key={c.id}>
                <button
                  className={'chunkitem' + (picked?.id === c.id ? ' is-on' : '')}
                  onClick={() => setPicked(c)}
                >
                  <span className="chunkitem-head">
                    <b>#{c.ord}</b>
                    <span className="muted">
                      {c.pageStart === c.pageEnd ? `${c.pageStart}쪽` : `${c.pageStart}~${c.pageEnd}쪽`}
                      {' · '}
                      {c.text.length}자
                    </span>
                    {c.kind === 'table' && <span className="tag tag-warn">표</span>}
                  </span>
                  {c.headingPath && <span className="chunkitem-path">{c.headingPath}</span>}
                  <span className="chunkitem-body">{c.text.slice(0, 90).replace(/\n/g, ' ')}</span>
                </button>
              </li>
            ))}
          </ul>

          <div className="chunks-view">
            {picked ? (
              <>
                <div className="chunks-quote selectable">
                  {picked.text.slice(0, 400)}
                  {picked.text.length > 400 && '…'}
                </div>
                <PdfHighlight documentId={doc.id} spans={picked.spans} />
              </>
            ) : (
              <p className="muted">왼쪽에서 청크를 고르세요.</p>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
