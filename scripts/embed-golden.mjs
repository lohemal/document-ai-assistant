// 골든 셋의 청크와 물음을 벡터로 만들어 저장한다.
//
//   npm run golden:embed                    (기본: bge-m3)
//   npm run golden:embed -- --model paraphrase-multilingual
//
//   -> test/golden/vectors/<모델>.json  (어느 청크가 어느 순서인지)
//      test/golden/vectors/<모델>.bin   (f32 리틀엔디언으로 이어 붙인 벡터)
//
// **한 번 만들어 저장소에 넣어 둔다.** 그래야 Ollama 가 없는 곳에서도(CI,
// 다른 PC) 검색 품질 시험을 똑같이 다시 돌릴 수 있다. 벡터를 만들 때만
// Ollama 가 필요하다.
//
// 이 스크립트는 **개발 도구**다. 앱이 도는 길과는 별개다 — 앱은 Rust 쪽
// `ai::ollama::embed` 로 이 PC 안(127.0.0.1)에만 말을 건다.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import { join, resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const dir = join(root, 'test', 'golden')
const outDir = join(dir, 'vectors')

const arg = (name, fallback) => {
  const hit = process.argv.find((a) => a.startsWith(`--${name}=`))
  if (hit) return hit.slice(name.length + 3)
  const i = process.argv.indexOf(`--${name}`)
  return i >= 0 ? process.argv[i + 1] : fallback
}

const model = arg('model', 'bge-m3')
const batch = Number(arg('batch', '8'))
const host = 'http://127.0.0.1:11434'

const corpus = JSON.parse(readFileSync(join(dir, 'chunks.json'), 'utf8'))
const golden = JSON.parse(readFileSync(join(dir, 'golden.json'), 'utf8'))

/** 청크는 색인용 정규화본을 넣는다 — 낱말 검색과 같은 글을 본다 */
const chunkKeys = []
const chunkTexts = []
for (const d of corpus.documents) {
  for (const c of d.chunks) {
    chunkKeys.push([d.id, c.ord])
    chunkTexts.push(c.textNorm)
  }
}
const questionIds = golden.questions.map((q) => q.question_id)
const questionTexts = golden.questions.map((q) => q.question)

async function embed(texts) {
  const res = await fetch(`${host}/api/embed`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ model, input: texts }),
  })
  if (!res.ok) {
    throw new Error(`${res.status} ${await res.text()}`)
  }
  const json = await res.json()
  if (!Array.isArray(json.embeddings) || json.embeddings.length !== texts.length) {
    throw new Error(`벡터가 ${json.embeddings?.length}개 왔습니다 (${texts.length}개여야 함)`)
  }
  return json.embeddings
}

// Ollama 가 살아 있는지 먼저 본다. 없으면 무엇을 하면 되는지 말한다.
try {
  const v = await fetch(`${host}/api/version`).then((r) => r.json())
  console.log(`Ollama ${v.version} 에 붙었습니다.`)
} catch {
  console.error(
    'Ollama 에 붙지 못했습니다 (127.0.0.1:11434).\n' +
      '  1) Ollama 를 켜세요.\n' +
      `  2) 모델을 받으세요:  ollama pull ${model}\n` +
      '  그런 뒤 다시 돌리세요.',
  )
  process.exit(1)
}

const all = [...chunkTexts, ...questionTexts]
console.log(`${model} 로 ${chunkTexts.length}개 청크 + ${questionTexts.length}개 물음 = ${all.length}개`)

const vectors = []
const started = Date.now()
for (let i = 0; i < all.length; i += batch) {
  const part = all.slice(i, i + batch)
  const got = await embed(part)
  vectors.push(...got)
  const done = Math.min(i + batch, all.length)
  const per = (Date.now() - started) / done
  process.stdout.write(
    `\r  ${done}/${all.length}  (${(per).toFixed(0)}ms/개, 남은 시간 ${(((all.length - done) * per) / 1000).toFixed(0)}초)   `,
  )
}
const elapsed = (Date.now() - started) / 1000
console.log(`\n  ${elapsed.toFixed(1)}초 걸렸습니다 (${(elapsed / all.length * 1000).toFixed(0)}ms/개)`)

const dim = vectors[0].length
if (vectors.some((v) => v.length !== dim)) throw new Error('벡터 길이가 서로 다릅니다')

const buf = Buffer.allocUnsafe(vectors.length * dim * 4)
let at = 0
for (const v of vectors) {
  for (const x of v) {
    buf.writeFloatLE(x, at)
    at += 4
  }
}

mkdirSync(outDir, { recursive: true })
const stem = model.replace(/[^A-Za-z0-9._-]/g, '-')
writeFileSync(
  join(outDir, `${stem}.json`),
  JSON.stringify({ model, dim, chunks: chunkKeys, questions: questionIds }, null, 1) + '\n',
)
writeFileSync(join(outDir, `${stem}.bin`), buf)

console.log(
  `test/golden/vectors/${stem}.{json,bin} 을 만들었습니다 — ${dim}차원 · ${(buf.length / 1024 / 1024).toFixed(1)}MB`,
)
console.log('이제 검색 품질을 잴 수 있습니다:  npm run golden:eval')
