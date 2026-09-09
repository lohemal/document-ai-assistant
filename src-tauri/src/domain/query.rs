//! 사용자가 입력한 말을 검색어로 바꾼다.
//!
//! ## 왜 이 일이 필요한가
//!
//! trigram 색인은 **한쪽으로만 관대하다.**
//!
//! - 문서에 `이용권을`, 찾는 말이 `이용권` → **걸린다** (부분 일치)
//! - 문서에 `이용권`, 찾는 말이 `이용권을` → **안 걸린다**
//!
//! 앞의 경우 때문에 trigram 을 골랐는데(설계안 2-5), 뒤의 경우가 남는다.
//! 사람이 자연스럽게 묻는 말에는 조사가 붙어 있다 — "지원액은 얼마야?" 의
//! `지원액은` 으로는 `지원액` 이 든 문서를 못 찾는다.
//!
//! 그래서 **찾는 말에서 조사를 떼어 낸 꼴도 함께** 찾는다.

/// 찾을 낱말 하나. 원래 꼴과, 조사를 떼어 낸 꼴을 함께 들고 다닌다.
#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    /// 사용자가 적은 그대로 (화면에 보여 줄 때 쓴다)
    pub raw: String,
    /// 실제로 찾아 볼 꼴들. 첫 번째가 가장 그럴듯한 것
    pub forms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub terms: Vec<Term>,
    /// 너무 짧아서 trigram 색인으로는 못 찾는 낱말들 (LIKE 로 대신 찾는다)
    pub short: Vec<String>,
    /// 물음말처럼 뜻이 없어 버린 낱말들. 왜 버렸는지 보여 줄 때 쓴다
    pub dropped: Vec<String>,
}

/// trigram 색인이 다룰 수 있는 가장 짧은 길이
pub const MIN_TRIGRAM: usize = 3;

/// 뒤에 붙는 조사. 긴 것을 먼저 본다.
const PARTICLES: &[&str] = &[
    "에서는", "에서도", "으로는", "이라는", "에게는", "한테는", "에게서", "으로써", "으로서",
    "라는", "에서", "으로", "로서", "로써", "부터", "까지", "보다", "에게", "한테", "처럼",
    "만큼", "이나", "라도", "이란", "이라", "마다", "조차", "밖에", "께서",
    "은", "는", "이", "가", "을", "를", "의", "에", "와", "과", "도", "만", "나", "랑", "야",
];

/// 물어보는 말. 뜻이 없어서 찾는 데 방해만 된다.
const ASKING: &[&str] = &[
    "얼마", "얼마야", "얼마인가", "얼마인가요", "얼마죠", "무엇", "무엇인가", "무엇인가요",
    "뭐", "뭐야", "뭔가요", "어떻게", "언제", "어디", "어디서", "누구", "누가", "왜",
    "있나", "있나요", "있어", "있어요", "있습니까", "되나요", "되나", "될까요", "하나요",
    "인가요", "일까요", "알려줘", "알려주세요", "말해줘", "해줘", "해주세요", "가능한가",
    "가능한가요", "쓸수", "쓸", "수", "것", "지", "좀", "그리고", "또는", "및",
];

fn is_asking(s: &str) -> bool {
    ASKING.iter().any(|a| *a == s)
}

/// 물어보는 말인가. **조사를 떼고도 본다** — `얼마까지` 는 `얼마` 다.
///
/// `parse` 는 조사를 뗀 꼴이 물음말이면 그 꼴만 버리고 원래 낱말은 남긴다
/// (`얼마까지` 로 찾을 수는 있으므로). 물음의 핵심어를 고를 때는 그렇게
/// 남은 것도 버려야 한다 — `얼마까지` 가 근거에 없다고 답을 거부하면 안 된다.
pub fn is_question_word(s: &str) -> bool {
    if is_asking(s) {
        return true;
    }
    matches!(strip_particle(s), Some(stem) if is_asking(&stem))
}

/// 뒤에 붙은 조사를 뗀다. 떼고 남는 것이 두 글자보다 짧으면 그냥 둔다 —
/// `종이` 에서 `이` 를 떼면 `종` 이 되어 엉뚱한 것이 걸린다.
pub fn strip_particle(term: &str) -> Option<String> {
    let chars: Vec<char> = term.chars().collect();
    for p in PARTICLES {
        if !term.ends_with(p) {
            continue;
        }
        let cut = p.chars().count();
        if chars.len() - cut >= 2 {
            return Some(chars[..chars.len() - cut].iter().collect());
        }
    }
    None
}

/// 낱말을 가르는 글자. 한글·영문·숫자·`%`·`~` 가 아니면 자른다.
fn is_wordish(c: char) -> bool {
    c.is_alphanumeric() || c == '%' || c == '~' || c == '·' || c == ','
}

/// 숫자 사이의 쉼표만 없앤다. `500,000` → `500000`, `가형, 나형` 은 그대로.
///
/// 색인 쪽(`normalize`)과 **똑같은 규칙**이어야 한다. 한쪽만 바꾸면
/// `500,000원` 으로 찾을 때 조용히 아무것도 안 나온다.
pub fn strip_digit_commas(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, c) in chars.iter().enumerate() {
        if *c == ',' {
            let before = i > 0 && chars[i - 1].is_ascii_digit();
            let after = chars.get(i + 1).is_some_and(|n| n.is_ascii_digit());
            if before && after {
                continue;
            }
        }
        out.push(*c);
    }
    out
}

pub fn parse(text: &str) -> Parsed {
    let cleaned = strip_digit_commas(&text.replace('　', " "));

    let mut terms: Vec<Term> = Vec::new();
    let mut short: Vec<String> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();

    for raw in cleaned.split(|c: char| !is_wordish(c)) {
        let raw = raw.trim_matches(',').trim();
        if raw.is_empty() {
            continue;
        }
        let n = raw.chars().count();
        if n < 2 {
            continue; // 한 글자는 아무 데나 걸린다
        }
        if is_asking(raw) {
            dropped.push(raw.to_string());
            continue;
        }

        let mut forms = vec![raw.to_string()];
        if let Some(stem) = strip_particle(raw) {
            if !is_asking(&stem) {
                forms.push(stem);
            }
        }

        // trigram 이 다룰 수 있는 꼴만 남긴다
        let usable: Vec<String> = forms
            .iter()
            .filter(|f| f.chars().count() >= MIN_TRIGRAM)
            .cloned()
            .collect();

        if usable.is_empty() {
            short.push(raw.to_string());
            continue;
        }

        // 같은 낱말을 두 번 넣지 않는다
        if terms.iter().any(|t| t.forms == usable) {
            continue;
        }
        terms.push(Term {
            raw: raw.to_string(),
            forms: usable,
        });
    }

    Parsed { terms, short, dropped }
}

/// FTS5 가 알아듣는 식으로 바꾼다.
///
/// 낱말은 **OR** 로 잇는다. AND 로 이으면 낱말 하나만 든 청크가 아예 빠지고,
/// 그러면 "그래도 이런 것들을 봤습니다" 를 보여 줄 수 없다. 어느 것이 더
/// 그럴듯한지는 순위에서 가린다 (`rank` 모듈).
pub fn fts_expression(p: &Parsed) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for t in &p.terms {
        for f in &t.forms {
            parts.push(format!("\"{}\"", f.replace('"', "\"\"")));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" OR "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forms(text: &str) -> Vec<Vec<String>> {
        parse(text).terms.into_iter().map(|t| t.forms).collect()
    }

    #[test]
    fn 낱말_하나를_그대로_찾는다() {
        assert_eq!(forms("자유수강권"), vec![vec!["자유수강권".to_string()]]);
    }

    #[test]
    fn 조사를_떼어_낸_꼴도_함께_찾는다() {
        // 이게 이 모듈이 있는 이유다. trigram 은 찾는 말에 붙은 조사를 못 넘는다.
        assert_eq!(
            forms("지원액은"),
            vec![vec!["지원액은".to_string(), "지원액".to_string()]]
        );
        assert_eq!(
            forms("자유수강권을"),
            vec![vec!["자유수강권을".to_string(), "자유수강권".to_string()]]
        );
    }

    #[test]
    fn 떼고_남는_것이_너무_짧으면_그냥_둔다() {
        // `종이` 에서 `이` 를 떼면 `종` 이 되어 엉뚱한 것이 걸린다
        assert_eq!(strip_particle("종이"), None);
        assert_eq!(strip_particle("지원액의"), Some("지원액".to_string()));
    }

    #[test]
    fn 여러_낱말을_가른다() {
        assert_eq!(
            forms("자유수강권 지원액"),
            vec![
                vec!["자유수강권".to_string()],
                vec!["지원액".to_string()]
            ]
        );
    }

    #[test]
    fn 자연어_질문에서_핵심어만_남긴다() {
        let p = parse("초등학교 3학년의 1인당 지원액은 얼마야?");
        let raws: Vec<&str> = p.terms.iter().map(|t| t.raw.as_str()).collect();
        assert_eq!(raws, vec!["초등학교", "3학년의", "1인당", "지원액은"]);
        assert!(p.dropped.contains(&"얼마야".to_string()));
        // 조사를 뗀 꼴이 함께 들어가야 한다
        assert!(p.terms[1].forms.contains(&"3학년".to_string()));
        assert!(p.terms[3].forms.contains(&"지원액".to_string()));
    }

    #[test]
    fn 숫자_사이의_쉼표만_없앤다() {
        assert_eq!(strip_digit_commas("500,000원"), "500000원");
        assert_eq!(strip_digit_commas("가형, 나형"), "가형, 나형");
        assert_eq!(strip_digit_commas("1,2,3"), "123");
    }

    #[test]
    fn 쉼표를_없애어_두_표기가_같아진다() {
        // 문서에 `500,000원`, 찾는 말이 `500000원` — 둘 다 `500000원` 이 된다
        assert_eq!(forms("500,000원"), forms("500000원"));
    }

    #[test]
    fn 두_글자_낱말은_따로_모은다() {
        // trigram 은 세 글자 아래를 색인하지 못한다
        let p = parse("교재");
        assert!(p.terms.is_empty());
        assert_eq!(p.short, vec!["교재".to_string()]);
    }

    #[test]
    fn 한_글자는_버린다() {
        let p = parse("이 것 을");
        assert!(p.terms.is_empty());
        assert!(p.short.is_empty());
    }

    #[test]
    fn fts_식은_or_로_잇는다() {
        let p = parse("자유수강권 지원액은");
        let e = fts_expression(&p).unwrap();
        assert_eq!(e, "\"자유수강권\" OR \"지원액은\" OR \"지원액\"");
    }

    #[test]
    fn 찾을_것이_없으면_식을_만들지_않는다() {
        assert!(fts_expression(&parse("얼마야?")).is_none());
        assert!(fts_expression(&parse("   ")).is_none());
    }

    #[test]
    fn 겹치는_따옴표를_막는다() {
        // FTS5 식을 깨뜨리지 못하게 한다
        let e = fts_expression(&parse("어쩌구\"저쩌구")).unwrap();
        assert!(!e.contains("\"어쩌구\"저쩌구\""));
    }
}
