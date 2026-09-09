/**
 * Rust 쪽 오류는 사용자에게 그대로 보여도 되는 한글 문장으로 온다.
 * 혹시 그렇지 않은 것이 섞여 오더라도 화면이 깨지지 않게 감싼다.
 */
export function message(e: unknown): string {
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  return '알 수 없는 문제가 생겼습니다.'
}
