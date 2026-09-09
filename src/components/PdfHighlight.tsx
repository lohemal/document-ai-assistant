import { useEffect, useRef, useState } from 'react'
import type { PDFDocumentLoadingTask, PDFDocumentProxy, RenderTask } from 'pdfjs-dist'
import { openPdf } from '@/lib/pdf/loader'
import { rectsForRange, type HighlightRect, type ItemGeom } from '@/lib/pdf/highlight'
import { parseItemMap, documentBytes, documentPages, type PageOut } from '@/ipc/documents'
import type { Span } from '@/ipc/chunks'
import { message } from '@/lib/err'

/**
 * PDF 한 쪽을 그리고, 그 위에 근거 자리를 형광펜으로 칠한다.
 *
 * 칠할 자리는 **저장해 둔 문자 위치**에서 나온다. 글자를 다시 찾지 않는다.
 */

/** 한 번 연 문서는 붙잡아 둔다. 청크를 바꿔 누를 때마다 다시 열면 느리다. */
const opened = new Map<number, { task: PDFDocumentLoadingTask; doc: Promise<PDFDocumentProxy> }>()

async function getDoc(documentId: number): Promise<PDFDocumentProxy> {
  const held = opened.get(documentId)
  if (held) return held.doc
  const bytes = await documentBytes(documentId)
  const task = openPdf(bytes)
  const entry = { task, doc: task.promise }
  opened.set(documentId, entry)
  return entry.doc
}

/** 문서를 지우거나 다시 나눌 때 붙잡아 둔 것을 놓는다 */
export function forgetPdf(documentId: number) {
  const held = opened.get(documentId)
  opened.delete(documentId)
  held?.task.destroy().catch(() => {})
}

type Props = {
  documentId: number
  /** 칠할 자리. 여러 쪽에 걸쳐 있을 수 있다 */
  spans: Span[]
  /** 처음 보여 줄 쪽. 없으면 첫 구간의 쪽 */
  page?: number
  width?: number
}

export default function PdfHighlight({ documentId, spans, page, width = 560 }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const boxRef = useRef<HTMLDivElement>(null)
  const rendering = useRef<RenderTask | null>(null)
  const [rects, setRects] = useState<HighlightRect[]>([])
  const [shown, setShown] = useState<number>(page ?? spans[0]?.page ?? 1)
  const [size, setSize] = useState({ w: 0, h: 0 })
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(true)
  const [pageCache, setPageCache] = useState<PageOut[] | null>(null)

  // 청크가 바뀌면 그 청크의 첫 쪽으로 옮긴다
  useEffect(() => {
    setShown(page ?? spans[0]?.page ?? 1)
  }, [page, spans])

  // 이 문서의 쪽 정보(문자 위치 지도)를 받아 둔다
  useEffect(() => {
    let alive = true
    documentPages(documentId)
      .then((ps) => alive && setPageCache(ps))
      .catch((e) => alive && setError(message(e)))
    return () => {
      alive = false
    }
  }, [documentId])

  useEffect(() => {
    let alive = true
    setBusy(true)
    ;(async () => {
      try {
        const pdf = await getDoc(documentId)
        if (!alive) return
        const pg = await pdf.getPage(shown)
        const base = pg.getViewport({ scale: 1 })
        const scale = width / base.width
        const viewport = pg.getViewport({ scale })

        const canvas = canvasRef.current
        if (!canvas || !alive) return
        canvas.width = Math.floor(viewport.width)
        canvas.height = Math.floor(viewport.height)
        setSize({ w: canvas.width, h: canvas.height })

        const ctx = canvas.getContext('2d')
        if (!ctx) return

        // 앞서 그리던 것이 있으면 먼저 세운다.
        //
        // pdf.js 는 같은 쪽을 동시에 두 번 그리지 못한다. 청크를 빠르게 바꿔
        // 누르면 늦게 온 렌더가 캔버스를 지운 채로 끝나, **형광펜만 뜨고 종이는
        // 하얗게** 남는다. 실제로 그랬다.
        rendering.current?.cancel()
        const task = pg.render({ canvas, canvasContext: ctx, viewport })
        rendering.current = task
        try {
          await task.promise
        } catch (e) {
          // 취소는 잘못이 아니다. 다음 렌더가 이어서 그린다.
          if ((e as { name?: string })?.name === 'RenderingCancelledException') return
          throw e
        }
        if (!alive) return

        // 형광펜 자리 계산 — 저장해 둔 문자 위치를 되짚는다
        const info = pageCache?.find((p) => p.page === shown)
        const here = spans.filter((s) => s.page === shown)
        if (info && here.length > 0) {
          const content = await pg.getTextContent()
          const items = content.items as unknown as ItemGeom[]
          const itemMap = parseItemMap(info.itemMap)
          const all: HighlightRect[] = []
          for (const s of here) {
            all.push(
              ...rectsForRange(itemMap, items, viewport.transform, scale, s.charStart, s.charEnd),
            )
          }
          if (alive) setRects(all)
        } else if (alive) {
          setRects([])
        }
        if (alive) setError(null)
      } catch (e) {
        if (alive) setError(message(e))
      } finally {
        if (alive) setBusy(false)
      }
    })()
    return () => {
      alive = false
    }
  }, [documentId, shown, spans, width, pageCache])

  // 형광펜이 화면 밖에 있으면 그리로 굴려 준다
  useEffect(() => {
    if (rects.length === 0 || !boxRef.current) return
    const top = Math.min(...rects.map((r) => r.y))
    boxRef.current.scrollTo({ top: Math.max(0, top - 120), behavior: 'smooth' })
  }, [rects])

  const pagesOfChunk = [...new Set(spans.map((s) => s.page))].sort((a, b) => a - b)

  return (
    <div className="pdfview">
      <div className="pdfview-bar">
        <span className="muted">원본 PDF</span>
        {pagesOfChunk.length > 1 && (
          <span className="pdfview-pages">
            이 근거는 {pagesOfChunk.join(', ')}쪽에 걸쳐 있습니다:
            {pagesOfChunk.map((p) => (
              <button
                key={p}
                className={'btn btn-tiny' + (p === shown ? ' is-on' : '')}
                onClick={() => setShown(p)}
              >
                {p}쪽
              </button>
            ))}
          </span>
        )}
        {pagesOfChunk.length <= 1 && <span>{shown}쪽</span>}
        {busy && <span className="muted">그리는 중…</span>}
        {rects.length === 0 && !busy && !error && (
          <span className="pdfview-warn">이 쪽에서 칠할 자리를 찾지 못했습니다</span>
        )}
      </div>

      {error && <p className="banner banner-error">{error}</p>}

      <div className="pdfview-scroll" ref={boxRef}>
        <div className="pdfview-stage" style={{ width: size.w, height: size.h }}>
          <canvas ref={canvasRef} className="pdfview-canvas" />
          {rects.map((r, i) => (
            <div
              key={i}
              className="pdfview-mark"
              style={{ left: r.x, top: r.y, width: r.w, height: r.h }}
            />
          ))}
        </div>
      </div>
    </div>
  )
}
