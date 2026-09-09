import { useMemo, useState } from 'react'
import { parseItemMap, type PageOut } from '@/ipc/documents'

/**
 * 뽑아 놓은 쪽 텍스트를 그대로 보여 주고, **문자 위치 지도를 눈으로 확인**하게
 * 한다.
 *
 * 글자를 누르면 "이 부분은 pdf.js 항목 몇 번, 이 쪽의 몇 번째 글자" 인지 나온다.
 * P3 에서 이 값으로 원본 PDF 에 형광펜을 칠할 것이므로, 그 전에 사람이 눈으로
 * 맞는지 볼 수 있어야 한다.
 */
export default function PageTextView({ page }: { page: PageOut }) {
  const [picked, setPicked] = useState<{ i: number; s: number; e: number } | null>(null)
  const spans = useMemo(() => parseItemMap(page.itemMap), [page.itemMap])

  const parts = useMemo(() => {
    const out: { key: string; text: string; span?: { i: number; s: number; e: number } }[] = []
    let cursor = 0
    for (const [s, e, i] of spans) {
      if (s > cursor) out.push({ key: `gap${cursor}`, text: page.text.slice(cursor, s) })
      out.push({ key: `it${i}-${s}`, text: page.text.slice(s, e), span: { i, s, e } })
      cursor = e
    }
    if (cursor < page.text.length) out.push({ key: `tail${cursor}`, text: page.text.slice(cursor) })
    return out
  }, [page.text, spans])

  if (page.text.trim().length === 0) {
    return (
      <div className="pagetext">
        <p className="banner banner-warn">
          {page.isScanned
            ? '이 쪽은 그림만 있고 글자가 없습니다 (스캔본).'
            : '이 쪽에서 글자를 하나도 뽑지 못했습니다.'}
        </p>
      </div>
    )
  }

  return (
    <div className="pagetext">
      <div className="pagetext-meta">
        글자 {page.text.length}자 · 항목 {spans.length}개
        {page.label && page.label !== String(page.page) && (
          <> · 문서에 적힌 쪽 이름 “{page.label}”</>
        )}
      </div>

      <div className="pagetext-body selectable">
        {parts.map((p) =>
          p.span ? (
            <span
              key={p.key}
              className={
                'pagetext-item' + (picked && picked.i === p.span.i ? ' is-picked' : '')
              }
              onClick={() => setPicked(p.span!)}
              title={`항목 ${p.span.i} · ${p.span.s}~${p.span.e}`}
            >
              {p.text}
            </span>
          ) : (
            <span key={p.key}>{p.text}</span>
          ),
        )}
      </div>

      {picked ? (
        <div className="pagetext-picked">
          누른 부분 — <strong>{page.page}쪽</strong> 텍스트의{' '}
          <strong>
            {picked.s}~{picked.e}
          </strong>
          번째 글자, pdf.js 항목 <strong>{picked.i}</strong>번
          <div className="pagetext-quote">“{page.text.slice(picked.s, picked.e)}”</div>
          <div className="muted">P3 에서 이 값으로 원본 PDF 에 형광펜을 칠합니다.</div>
        </div>
      ) : (
        <div className="pagetext-hint muted">
          글자를 누르면 그 부분이 원본 PDF 의 어느 항목에서 왔는지 보여 줍니다.
        </div>
      )}
    </div>
  )
}
