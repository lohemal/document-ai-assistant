/**
 * AI 가 지금 어디까지 준비되어 있는가.
 *
 * 이 프로그램은 AI 없이도 돌아간다 (설계안 2-12). 그래서 상태를 두 단계로
 * 나누어 들고 다닌다 — 임베딩 모델이 있으면 뜻으로 찾기가 되고, 답변 모델까지
 * 있으면 질문하기가 된다.
 *
 * P4b 에서 Ollama 를 실제로 확인하도록 채운다. 지금은 화면 뼈대가 이 값을
 * 어떻게 쓰는지만 정해 둔다.
 */
export type AiStatus = {
  /** Ollama 가 127.0.0.1:11434 에서 응답하는가 */
  ollama: boolean
  /** 임베딩 모델이 있는가 → 뜻으로 찾기 */
  embed: boolean
  /** 답변 모델이 있는가 → 질문하기 */
  llm: boolean
  /** 모델을 받는 중이면 0~100 */
  pulling: number | null
}

export const AI_NONE: AiStatus = { ollama: false, embed: false, llm: false, pulling: null }

/** 이 기능을 지금 쓸 수 있는가. 못 쓰면 왜 못 쓰는지 한 줄로 알려 준다. */
export function can(status: AiStatus, need: 'keyword' | 'embed' | 'llm'): string | null {
  if (need === 'keyword') return null // 낱말로 찾기는 언제나 된다
  if (!status.ollama) return 'Ollama 가 실행되고 있지 않습니다.'
  if (need === 'embed' && !status.embed) return '검색용 AI 모델이 설치되지 않았습니다.'
  if (need === 'llm' && !status.llm) return '답변용 AI 모델이 설치되지 않았습니다.'
  return null
}
