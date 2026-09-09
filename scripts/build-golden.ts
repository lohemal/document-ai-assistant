/**
 * 골든 셋의 "정답 문장" 을 지금의 청크 번호로 옮긴다.
 *
 *   npm run golden:build     ->  test/golden/golden.json
 *
 * 왜 이렇게 하는가:
 *   정답을 청크 번호로 적어 두면, 청크 나누는 기준을 손보는 순간 골든 셋이
 *   조용히 거짓이 된다. 번호는 바뀌지만 **원문 문장은 바뀌지 않는다.** 그래서
 *   사람은 문장을 적고, 번호는 매번 여기서 다시 매긴다.
 *
 * 견주는 방법: 공백을 모두 지우고 견준다. 추출한 글에서는 낱말 가운데에서도
 * 줄이 바뀌기 때문에("수용\n비"), 공백을 남기면 못 찾는다.
 */
import { readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const dir = join(root, 'test', 'golden')

type Chunk = { ord: number; text: string; pageStart: number; pageEnd: number; kind: string }
type Doc = { id: number; collectionId: number; title: string; chunks: Chunk[] }
type Negative = {
  id: string
  doc: number
  q: string
  /** 자료에 무엇이 없는지 — 나중에 자료가 바뀌면 이 물음을 다시 봐야 한다 */
  absent: string
  why?: string
}

type Question = {
  id: string
  doc: number
  type: 'A' | 'B' | 'C' | 'D' | 'E' | 'F'
  q: string
  anchors: string[]
  why?: string
}

const corpus = JSON.parse(readFileSync(join(dir, 'chunks.json'), 'utf8')) as {
  documents: Doc[]
}
const { questions, negatives } = JSON.parse(
  readFileSync(join(dir, 'questions.json'), 'utf8'),
) as { questions: Question[]; negatives: Negative[] }

const bare = (s: string) => s.replace(/\s+/g, '')

const problems: string[] = []
const out: unknown[] = []
const seen = new Set<string>()

for (const q of questions) {
  if (seen.has(q.id)) problems.push(`${q.id}: 같은 번호가 두 번 있습니다`)
  seen.add(q.id)

  const doc = corpus.documents.find((d) => d.id === q.doc)
  if (!doc) {
    problems.push(`${q.id}: 자료 ${q.doc} 이 없습니다`)
    continue
  }

  const hits = new Set<number>()
  for (const a of q.anchors) {
    const needle = bare(a)
    const found = doc.chunks.filter((c) => bare(c.text).includes(needle))
    if (found.length === 0) {
      problems.push(`${q.id}: 원문에서 못 찾은 문장 → ${JSON.stringify(a)}`)
      continue
    }
    for (const c of found) hits.add(c.ord)
  }
  if (hits.size === 0) continue

  const chunks = [...hits].sort((a, b) => a - b).map((o) => doc.chunks.find((c) => c.ord === o)!)
  const pages = [...new Set(chunks.flatMap((c) => [c.pageStart, c.pageEnd]))].sort((a, b) => a - b)

  out.push({
    question_id: q.id,
    document_id: doc.id,
    collection_id: doc.collectionId,
    document: doc.title,
    question: q.q,
    question_type: q.type,
    expected_pages: pages,
    expected_chunk_ids: chunks.map((c) => c.ord),
    evidence: q.anchors,
    kinds: chunks.map((c) => c.kind),
    note: q.why ?? '',
  })
}

// 여러 청크가 정답인 경우가 정상이다(앞뒤가 겹쳐 담기거나, 같은 규정이 두 군데
// 나오는 경우). 다만 너무 많으면 질문이 너무 넓다는 뜻이니 알려 준다.
for (const r of out as { question_id: string; expected_chunk_ids: number[] }[]) {
  if (r.expected_chunk_ids.length > 4) {
    problems.push(`${r.question_id}: 정답 청크가 ${r.expected_chunk_ids.length}개입니다 — 질문이 너무 넓습니다`)
  }
}

const byType = new Map<string, number>()
const byDoc = new Map<number, number>()
for (const r of out as { question_type: string; document_id: number }[]) {
  byType.set(r.question_type, (byType.get(r.question_type) ?? 0) + 1)
  byDoc.set(r.document_id, (byDoc.get(r.document_id) ?? 0) + 1)
}

const TYPE_NAME: Record<string, string> = {
  A: '직접 표현',
  B: '조사 변화',
  C: '바꿔쓰기',
  D: '숫자 표기',
  E: '조건·규정',
  F: '구조·요약',
}

console.log(`골든 셋 ${out.length}문항`)
for (const d of corpus.documents) {
  console.log(`  자료 ${d.id} ${d.title.slice(0, 30)} — ${byDoc.get(d.id) ?? 0}문항`)
}
console.log('  유형별:', [...byType].sort().map(([t, n]) => `${TYPE_NAME[t]} ${n}`).join(' · '))
const multi = (out as { expected_chunk_ids: number[] }[]).filter((r) => r.expected_chunk_ids.length > 1)
console.log(`  정답 청크가 둘 이상인 질문 ${multi.length}개`)
const tables = (out as { kinds: string[] }[]).filter((r) => r.kinds.includes('table'))
console.log(`  정답이 표 청크인 질문 ${tables.length}개`)
console.log(`  자료에 답이 없는 물음 ${(negatives ?? []).length}개`)

if (problems.length > 0) {
  console.log(`\n손볼 곳 ${problems.length}건`)
  for (const p of problems) console.log(`  - ${p}`)
}

/** 자료에 답이 없는 물음. 정답 청크가 없으므로 따로 담는다. */
const negativeOut = (negatives ?? []).map((n) => {
  const doc = corpus.documents.find((d) => d.id === n.doc)
  return {
    question_id: n.id,
    document_id: n.doc,
    collection_id: doc?.collectionId ?? 0,
    document: doc?.title ?? '',
    question: n.q,
    question_type: 'N',
    absent: n.absent,
    note: n.why ?? '',
  }
})

writeFileSync(
  join(dir, 'golden.json'),
  JSON.stringify({ questions: out, negatives: negativeOut }, null, 1) + '\n',
)
console.log(`\ntest/golden/golden.json 을 만들었습니다.`)
if (problems.some((p) => p.includes('못 찾은') || p.includes('없습니다'))) process.exit(1)
