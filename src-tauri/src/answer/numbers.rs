//! 답변에 든 **숫자**를 뽑아 근거와 견준다.
//!
//! 업무자료에서 숫자가 틀리면 답변 전체가 쓸모없어진다. 금액·기한·횟수·학년은
//! 사람이 그대로 옮겨 쓰는 값이라, 하나만 어긋나도 결재가 잘못 올라간다.
//!
//! **표기가 갈리는 것이 여기서 가장 어려운 일이다.** 같은 값을 학교 자료는
//! 이렇게 여러 가지로 적는다.
//!
//!     500,000원   500000원   50만원   50만 원   5십만원
//!     4만원       40,000원
//!     1억 1775만원  117,750,000원
//!
//! 그래서 글자를 견주지 않고 **값으로 견준다.** 뽑을 때 곧바로 셈해 두고,
//! 근거 쪽에서도 같은 방법으로 뽑아 값이 같은지 본다.
//!
//! 정규식을 쓰지 않고 손으로 훑는다. 규칙이 한국어에 맞아야 하고(만·억·천이
//! 이어 붙는다), 이 프로그램은 밖에서 가져오는 것을 되도록 늘리지 않는다.

use serde::Serialize;

/// 숫자가 무엇을 가리키는가. **종류가 다르면 값이 같아도 다른 것이다** —
/// `5%` 와 `5회` 를 같은 것으로 보면 검증이 거짓말을 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// 금액 — 원
    Money,
    /// 비율 — %, 퍼센트
    Percent,
    /// 횟수·사람 수 — 회, 차시, 명, 인, 개, 부
    Count,
    /// 학년
    Grade,
    /// 날짜·기간 — 년, 월, 일, 개월, 주, 시간
    Date,
    /// 법령 자리 — 조, 항, 호, 편, 장, 절
    Article,
    /// 단위를 못 알아본 맨 숫자
    Plain,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Money => "금액",
            Kind::Percent => "비율",
            Kind::Count => "횟수",
            Kind::Grade => "학년",
            Kind::Date => "날짜",
            Kind::Article => "조항",
            Kind::Plain => "숫자",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Num {
    /// 글에 적혀 있던 그대로
    pub raw: String,
    pub kind: Kind,
    /// 셈해 둔 값. 표기가 달라도 이 값이 같으면 같은 숫자다
    pub value: f64,
    /// 글에서 몇 번째 글자에 있었나 (문맥 창을 볼 때 쓴다)
    pub at: usize,
}

/// 숫자 뒤에 붙는 단위. 긴 것을 먼저 봐야 한다 — `개월` 을 `개` 로 읽으면 안 된다.
const UNITS: &[(&str, Kind)] = &[
    ("퍼센트", Kind::Percent),
    ("%", Kind::Percent),
    ("학년", Kind::Grade),
    ("개월", Kind::Date),
    ("차시", Kind::Count),
    ("시간", Kind::Date),
    ("원", Kind::Money),
    ("회", Kind::Count),
    ("명", Kind::Count),
    ("인", Kind::Count),
    ("부", Kind::Count),
    ("개", Kind::Count),
    ("년", Kind::Date),
    ("월", Kind::Date),
    ("일", Kind::Date),
    ("주", Kind::Date),
    ("조", Kind::Article),
    ("항", Kind::Article),
    ("호", Kind::Article),
    ("편", Kind::Article),
    ("장", Kind::Article),
    ("절", Kind::Article),
];

/// 한국어 자릿수. 큰 것부터 본다.
const SCALES: &[(&str, f64)] = &[("억", 100_000_000.0), ("만", 10_000.0), ("천", 1_000.0), ("백", 100.0)];

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// 글에서 숫자를 모두 뽑는다.
pub fn extract(text: &str) -> Vec<Num> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Num> = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        if !is_digit(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        let mut raw = String::new();
        // 값은 조각을 이어 더한다: "1억 1775만" = 1e8 + 1775e4
        let mut total = 0f64;
        let mut had_scale = false;

        loop {
            // ① 숫자 (쉼표는 자릿수 구분이므로 지운다)
            let mut digits = String::new();
            while i < chars.len() && (is_digit(chars[i]) || chars[i] == ',') {
                if is_digit(chars[i]) {
                    digits.push(chars[i]);
                }
                raw.push(chars[i]);
                i += 1;
            }
            if digits.is_empty() {
                break;
            }
            let mut part: f64 = digits.parse().unwrap_or(0.0);

            // ② 자릿수 말 (만·억·천). 사이의 빈칸은 넘어간다: "50만 원"
            let save = i;
            let mut spaces = 0;
            while i < chars.len() && chars[i] == ' ' {
                i += 1;
                spaces += 1;
            }
            let scale = SCALES.iter().find(|(w, _)| {
                chars.get(i).map(|c| c.to_string() == *w).unwrap_or(false)
            });
            match scale {
                Some((w, mul)) => {
                    // 빈칸을 넘어 자릿수 말을 만났으면 그 빈칸도 raw 에 담는다
                    for _ in 0..spaces {
                        raw.push(' ');
                    }
                    raw.push_str(w);
                    i += 1;
                    part *= mul;
                    had_scale = true;
                }
                None => {
                    i = save;
                }
            }
            total += part;

            // ③ 이어지는 조각이 있는가: "1억 1775만" / "5만 2천"
            let save = i;
            let mut spaces = 0;
            while i < chars.len() && chars[i] == ' ' {
                i += 1;
                spaces += 1;
            }
            if had_scale && i < chars.len() && is_digit(chars[i]) {
                for _ in 0..spaces {
                    raw.push(' ');
                }
                continue; // 다시 ① 로
            }
            i = save;
            break;
        }

        // ④ 단위
        let save = i;
        let mut spaces = 0;
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
            spaces += 1;
        }
        let rest: String = chars[i..].iter().collect();
        let mut kind = Kind::Plain;
        for (u, k) in UNITS {
            if rest.starts_with(u) {
                for _ in 0..spaces {
                    raw.push(' ');
                }
                raw.push_str(u);
                i += u.chars().count();
                kind = *k;
                break;
            }
        }
        if kind == Kind::Plain {
            i = save;
        }

        // `제56조` 처럼 앞에 '제' 가 붙으면 조항이다 (단위가 없어도)
        if kind == Kind::Plain && start > 0 && chars[start - 1] == '제' {
            kind = Kind::Article;
        }

        // 단위 없는 0 은 버린다.
        //
        // PDF 에서 뽑은 글은 숫자 가운데에 빈칸이 끼어 있을 때가 있다 —
        // 쪽 번호를 자간 벌려 적은 `- 1 0 0 -` 같은 것. 그러면 `00` 같은
        // 조각이 나오고, 화면의 검증 표에 `숫자 00` 이 뜬다. 뜻이 없는 값을
        // 확인했다고 늘어놓으면 표를 읽을 수 없다.
        //
        // 단위가 붙은 0 은 남긴다 — `0회` 는 뜻이 있다.
        if kind == Kind::Plain && total == 0.0 {
            continue;
        }

        out.push(Num {
            raw,
            kind,
            value: total,
            at: start,
        });
    }

    out
}

/// 두 숫자가 같은 것을 가리키는가.
///
/// 종류가 다르면 다른 것으로 본다. 다만 한쪽이 단위를 못 알아본 맨 숫자면
/// 값만 견준다 — `500,000` 과 `500,000원` 을 다른 것으로 보면 놓친다.
pub fn same(a: &Num, b: &Num) -> bool {
    let kind_ok = a.kind == b.kind || a.kind == Kind::Plain || b.kind == Kind::Plain;
    kind_ok && (a.value - b.value).abs() < 0.001
}

/// `needle` 의 숫자가 `haystack` 안에 있는가.
pub fn contains(haystack: &[Num], needle: &Num) -> bool {
    haystack.iter().any(|h| same(h, needle))
}

#[cfg(test)]
#[path = "numbers_tests.rs"]
mod tests;
