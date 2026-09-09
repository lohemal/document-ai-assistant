//! 찾아낸 청크를 그럴듯한 차례로 늘어놓는다.
//!
//! ## 왜 BM25 만으로는 부족한가
//!
//! trigram 색인의 BM25 는 **세 글자 조각의 잦기**를 센다. 그래서 긴 낱말
//! 하나가 여러 번 나오는 청크가, 짧은 낱말 여럿이 골고루 든 청크를 이긴다.
//! 하지만 사람이 `자유수강권 지원액` 이라고 물었을 때 원하는 것은
//! **두 낱말이 함께 있는** 청크다.
//!
//! 그래서 먼저 **몇 낱말이 들어 있는가**로 가르고, 같으면 BM25 로 가린다.
//! 규칙이 단순해서 화면에 "3개 중 3개 일치" 라고 그대로 보여 줄 수 있고,
//! P4b 에서 RRF 로 의미 검색과 합칠 때도 순위만 쓰면 된다.

use super::query::Term;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub chunk_id: i64,
    /// 검색 색인에 들어간 꼴 (소문자·공백 정리된 것)
    pub text_norm: String,
    /// SQLite 가 준 값. **작을수록**(음수일수록) 잘 맞는다
    pub bm25: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    pub chunk_id: i64,
    /// 찾는 낱말 가운데 몇 개가 들어 있는가
    pub matched: usize,
    /// 어느 낱말이 들어 있었는가 (화면에서 보여 준다)
    pub matched_terms: Vec<String>,
    pub bm25: f64,
}

/// 이 낱말이 글 안에 있는가. 여러 꼴 가운데 하나만 있어도 있는 것으로 본다.
fn contains(text: &str, term: &Term) -> bool {
    term.forms.iter().any(|f| text.contains(f.as_str()))
}

pub fn rank(candidates: &[Candidate], terms: &[Term]) -> Vec<Ranked> {
    let mut out: Vec<Ranked> = candidates
        .iter()
        .map(|c| {
            let hit: Vec<String> = terms
                .iter()
                .filter(|t| contains(&c.text_norm, t))
                .map(|t| t.raw.clone())
                .collect();
            Ranked {
                chunk_id: c.chunk_id,
                matched: hit.len(),
                matched_terms: hit,
                bm25: c.bm25,
            }
        })
        .collect();

    out.sort_by(|a, b| {
        b.matched
            .cmp(&a.matched)
            .then(a.bm25.partial_cmp(&b.bm25).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.chunk_id.cmp(&b.chunk_id)) // 같으면 늘 같은 차례가 되게
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::query;

    fn cand(id: i64, text: &str, bm: f64) -> Candidate {
        Candidate {
            chunk_id: id,
            text_norm: text.into(),
            bm25: bm,
        }
    }

    #[test]
    fn 낱말이_더_많이_든_청크가_앞에_온다() {
        // 사용자가 요청한 바로 그 경우:
        // 자유수강권만 / 지원액만 / 둘 다 + 금액  ->  셋째가 가장 앞
        let terms = query::parse("자유수강권 지원액").terms;
        let cands = vec![
            cand(1, "자유수강권 운영지침을 알려 드립니다", -9.0),
            cand(2, "1인당 지원액은 학기마다 정한다", -8.0),
            cand(3, "자유수강권 지원액은 500000원", -1.0),
        ];
        let r = rank(&cands, &terms);
        assert_eq!(r[0].chunk_id, 3, "두 낱말이 함께 든 것이 앞이어야 한다");
        assert_eq!(r[0].matched, 2);
        // 나머지 둘은 낱말 수가 같으므로 BM25 로 가린다 (작을수록 앞)
        assert_eq!(r[1].chunk_id, 1);
        assert_eq!(r[2].chunk_id, 2);
    }

    #[test]
    fn bm25_가_좋아도_낱말_수를_이기지_못한다() {
        let terms = query::parse("자유수강권 지원액").terms;
        let cands = vec![
            cand(1, "자유수강권 자유수강권 자유수강권", -99.0),
            cand(2, "자유수강권 지원액", -0.1),
        ];
        let r = rank(&cands, &terms);
        assert_eq!(r[0].chunk_id, 2);
    }

    #[test]
    fn 조사가_붙어_있어도_들어_있는_것으로_센다() {
        let terms = query::parse("자유수강권").terms;
        let r = rank(&[cand(1, "자유수강권을 사용할 수 있습니다", -1.0)], &terms);
        assert_eq!(r[0].matched, 1);
    }

    #[test]
    fn 어느_낱말이_걸렸는지_알려_준다() {
        let terms = query::parse("자유수강권 지원액").terms;
        let r = rank(&[cand(1, "자유수강권 안내", -1.0)], &terms);
        assert_eq!(r[0].matched_terms, vec!["자유수강권".to_string()]);
    }

    #[test]
    fn 같은_값이면_늘_같은_차례가_된다() {
        let terms = query::parse("지원액").terms;
        let cands = vec![cand(7, "지원액", -1.0), cand(3, "지원액", -1.0)];
        let r = rank(&cands, &terms);
        assert_eq!(r[0].chunk_id, 3);
        assert_eq!(r[1].chunk_id, 7);
    }
}
