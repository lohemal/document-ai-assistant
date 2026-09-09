/**
 * 가짜 Ollama. **개발용 도구다.**
 *
 *   node scripts/fake-ollama.mjs ok          모델까지 다 갖춘 정상 상태
 *   node scripts/fake-ollama.mjs empty       실행 중이지만 모델이 없음
 *   node scripts/fake-ollama.mjs notollama   11434 에 다른 프로그램이 있음
 *   node scripts/fake-ollama.mjs hang        붙기는 하는데 답이 없음
 *   node scripts/fake-ollama.mjs disk        모델 받기가 저장공간 부족으로 실패
 *   node scripts/fake-ollama.mjs network     모델 받기가 인터넷 문제로 실패
 *
 * 왜 만들었나: AI 실행환경 상태를 여섯 가지로 가르는데, 진짜 Ollama 로는
 * 그 가운데 몇 가지밖에 만들어 볼 수 없다. "포트에 다른 프로그램이 있는 경우"
 * 나 "받다가 저장공간이 모자란 경우" 를 실제로 만들려면 디스크를 채워야 한다.
 *
 * 그리고 이 도구 덕분에 **Ollama 를 깔지 않고도** 설치 화면 전체를 눈으로
 * 확인할 수 있다.
 *
 * 진짜 Ollama 를 대신하지는 않는다. 답변 품질을 시험하는 데는 쓸 수 없다.
 */
import { createServer } from 'node:http'

const mode = process.argv[2] ?? 'ok'
const PORT = 11434

const MODELS = {
  ok: [
    { name: 'bge-m3:latest', size: 1_228_000_000, modified_at: '2026-09-01T00:00:00Z' },
    { name: 'gemma3:4b', size: 3_338_801_152, modified_at: '2026-09-01T00:00:00Z' },
  ],
  empty: [],
}

const send = (res, code, body) => {
  const text = JSON.stringify(body)
  res.writeHead(code, { 'Content-Type': 'application/json' })
  res.end(text)
}

const server = createServer((req, res) => {
  const path = (req.url ?? '/').split('?')[0]
  console.log(`  ${req.method} ${path}`)

  if (mode === 'hang') {
    // 붙기는 하지만 아무 답도 하지 않는다
    return
  }

  if (mode === 'notollama') {
    res.writeHead(200, { 'Content-Type': 'text/html' })
    res.end('<html><body>여기는 다른 프로그램입니다</body></html>')
    return
  }

  if (path === '/api/version') {
    return send(res, 200, { version: '0.0.0-fake' })
  }

  if (path === '/api/tags') {
    return send(res, 200, { models: MODELS[mode === 'empty' ? 'empty' : 'ok'] })
  }

  if (path === '/api/pull') {
    res.writeHead(200, { 'Content-Type': 'application/x-ndjson' })
    const line = (o) => res.write(JSON.stringify(o) + '\n')

    line({ status: 'pulling manifest' })

    if (mode === 'disk') {
      setTimeout(() => {
        line({ error: 'write /root/.ollama/models: no space left on device' })
        res.end()
      }, 600)
      return
    }
    if (mode === 'network') {
      setTimeout(() => {
        line({
          error:
            'Get "https://registry.ollama.ai/v2/library/gemma3/manifests/4b": dial tcp: lookup registry.ollama.ai: no such host',
        })
        res.end()
      }, 600)
      return
    }

    // 층 두 개를 조금씩 받는 것처럼 흉내 낸다
    const layers = [
      { digest: 'sha256:aaaa', total: 3_200_000_000 },
      { digest: 'sha256:bbbb', total: 140_000_000 },
    ]
    let tick = 0
    const timer = setInterval(() => {
      tick++
      const frac = Math.min(1, tick / 12)
      for (const l of layers) {
        line({
          status: `pulling ${l.digest.slice(7, 19)}`,
          digest: l.digest,
          total: l.total,
          completed: Math.floor(l.total * frac),
        })
      }
      if (frac >= 1) {
        clearInterval(timer)
        line({ status: 'verifying sha256 digest' })
        line({ status: 'writing manifest' })
        line({ status: 'success' })
        res.end()
      }
    }, 500)

    req.on('close', () => clearInterval(timer))
    return
  }

  if (path === '/api/generate') {
    return send(res, 200, { model: 'fake', response: '2', done: true })
  }

  if (path === '/api/embed') {
    return send(res, 200, { embeddings: [Array.from({ length: 1024 }, () => 0.01)] })
  }

  send(res, 404, { error: 'not found' })
})

server.listen(PORT, '127.0.0.1', () => {
  console.log(`가짜 Ollama (${mode}) — http://127.0.0.1:${PORT}`)
  console.log('Ctrl+C 로 멈춥니다.\n')
})
