import { useEffect, useState } from 'react'
import { indexModels, indexSetModel, type EmbedModel } from '@/ipc/index'
import { message } from '@/lib/err'

/**
 * 의미 검색에 쓸 모델을 고른다.
 *
 * **품질 차이를 숨기지 않는다.** 가벼운 모델은 8.7배 빠르지만 실제 자료
 * 골든 셋에서 찾는 솜씨가 크게 떨어졌다(R@5 78.8% → 46.2%, 숫자가 든 물음은
 * 1등을 하나도 못 맞혔다). 사양 때문에 그 쪽을 골라야 하는 사람도 있으니
 * 막지는 않되, 무엇을 잃는지는 화면에 적는다.
 */
const QUALITY: Record<string, { good: string[]; bad: string[] }> = {
  'embed-standard': {
    good: ['찾는 정확도 권장', '실제 업무자료에서 R@5 78.8%'],
    bad: ['약 1.2GB', '색인 속도 느림 (청크당 약 0.8초)'],
  },
  'embed-light': {
    good: ['메모리를 적게 씀 (약 0.6GB)', '색인 속도 빠름 (청크당 약 0.1초)'],
    bad: ['찾는 정확도 낮음 — R@5 46.2%', '숫자가 든 물음에 특히 약함'],
  },
}

export default function EmbedModelPick() {
  const [models, setModels] = useState<EmbedModel[] | null>(null)
  const [current, setCurrent] = useState<string>('')
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)

  useEffect(() => {
    indexModels()
      .then((r) => {
        setModels(r.models)
        setCurrent(r.current)
      })
      .catch((e) => setError(message(e)))
  }, [])

  async function pick(id: string) {
    if (id === current) return
    try {
      await indexSetModel(id)
      setCurrent(id)
      setNotice(
        '검색 모델을 바꿨습니다. 이미 색인해 둔 자료는 "다른 모델로 색인됨" 으로 표시됩니다 — ' +
          '[자료 등록] 화면에서 다시 색인하거나, 그대로 두고 낱말 검색을 쓸 수 있습니다.',
      )
      setError(null)
    } catch (e) {
      setError(message(e))
    }
  }

  if (!models) return null

  return (
    <section className="card">
      <h2 className="card-title">의미 검색 모델</h2>
      <p className="muted small">
        같은 뜻의 다른 말로 물어도 찾게 해 주는 모델입니다. 바꾸면 이미 만들어 둔 벡터는 쓸 수
        없으므로 다시 색인해야 합니다.
      </p>

      {error && <p className="banner banner-error">{error}</p>}
      {notice && <p className="banner banner-info">{notice}</p>}

      <ul className="list">
        {models.map((m) => {
          const q = QUALITY[m.id]
          const on = m.id === current
          return (
            <li className={'list-item' + (on ? ' is-on' : '')} key={m.id}>
              <div className="list-main">
                <div className="list-name">
                  {m.name}
                  {on && <span className="tag tag-ok"> 지금 쓰는 모델</span>}
                </div>
                <div className="list-sub">
                  <span className="muted">{m.dim}차원</span>
                  <span className="muted">약 {m.downloadGb}GB</span>
                  <span className="muted">메모리 {m.minRamGb}GB 이상</span>
                </div>
                {q && (
                  <ul className="pros">
                    {q.good.map((t) => (
                      <li key={t}>+ {t}</li>
                    ))}
                    {q.bad.map((t) => (
                      <li className="con" key={t}>
                        − {t}
                      </li>
                    ))}
                  </ul>
                )}
                <div className="muted small">{m.note}</div>
              </div>
              <div className="list-actions">
                {!on && (
                  <button className="btn" onClick={() => void pick(m.id)}>
                    이 모델 쓰기
                  </button>
                )}
              </div>
            </li>
          )
        })}
      </ul>
    </section>
  )
}
