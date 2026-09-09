//! 숫자 뽑기·견주기 시험.
//!
//! 여기 든 표기들은 **실제 업무자료 3종에서 가져온 것**이다. 지어낸 것이
//! 아니라, P4b 에서 골든 셋을 만들며 실제로 마주친 꼴들이다.

use super::*;

fn one(text: &str) -> Num {
    let v = extract(text);
    assert_eq!(v.len(), 1, "{text} 에서 {}개가 나왔습니다: {v:?}", v.len());
    v.into_iter().next().unwrap()
}

#[test]
fn 쉼표가_있든_없든_같은_값이다() {
    assert_eq!(one("500,000원").value, 500000.0);
    assert_eq!(one("500000원").value, 500000.0);
    assert_eq!(one("500,000원").kind, Kind::Money);
}

#[test]
fn 만_단위를_셈한다() {
    // P4a 에서 낱말 검색이 못 잇던 바로 그 짝
    assert_eq!(one("50만원").value, 500000.0);
    assert!(same(&one("50만원"), &one("500,000원")));
    assert!(same(&one("4만원"), &one("40,000원")));
    assert!(same(&one("3만원"), &one("30,000원")));
}

#[test]
fn 빈칸이_끼어도_읽는다() {
    assert_eq!(one("50만 원").value, 500000.0);
    assert_eq!(one("50만 원").raw, "50만 원");
}

#[test]
fn 자릿수가_이어_붙는다() {
    assert_eq!(one("5만 2천원").value, 52000.0);
    assert!(same(&one("5만 2천원"), &one("52,000원")));
    assert_eq!(one("1억 1775만원").value, 117_750_000.0);
    assert!(same(&one("1억 1775만원"), &one("117,750,000원")));
}

#[test]
fn 종류가_다르면_다른_숫자다() {
    // 5% 와 5회 를 같다고 보면 검증이 거짓말을 한다
    assert!(!same(&one("5%"), &one("5회")));
    assert_eq!(one("5%").kind, Kind::Percent);
    assert_eq!(one("5퍼센트").kind, Kind::Percent);
    assert!(same(&one("5%"), &one("5퍼센트")));
}

#[test]
fn 단위를_긴_것부터_읽는다() {
    assert_eq!(one("10개월").kind, Kind::Date);
    assert_eq!(one("10개월").raw, "10개월");
    assert_eq!(one("2차시").kind, Kind::Count);
    assert_eq!(one("1시간").kind, Kind::Date);
    assert_eq!(one("3학년").kind, Kind::Grade);
}

#[test]
fn 맨_숫자는_값만_견준다() {
    // 표 안에는 단위 없이 값만 적힌 칸이 많다
    let bare = one("500000");
    assert_eq!(bare.kind, Kind::Plain);
    assert!(same(&bare, &one("500,000원")));
}

#[test]
fn 조항을_알아본다() {
    let v = extract("제56조 제4항");
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].kind, Kind::Article);
    assert_eq!(v[0].value, 56.0);
    assert_eq!(v[1].kind, Kind::Article);
    assert_eq!(v[1].value, 4.0);
}

#[test]
fn 제_가_붙으면_단위가_없어도_조항이다() {
    let v = extract("제8조의2");
    assert_eq!(v[0].kind, Kind::Article);
    assert_eq!(v[0].value, 8.0);
}

#[test]
fn 실제_문장에서_여러_개를_뽑는다() {
    // 학교회계 지침 100쪽에 실제로 있는 문장들
    let text = "간담회 및 협의회 경비는 1인 1회당 4만원 이하 범위에서 집행함. \
                건당 50만원 이상의 경우에는 성명을 기재. 경조사비는 1인당 50,000원 이내";
    let v = extract(text);
    let money: Vec<f64> = v.iter().filter(|n| n.kind == Kind::Money).map(|n| n.value).collect();
    assert!(money.contains(&40000.0), "{v:?}");
    assert!(money.contains(&500000.0), "{v:?}");
    assert!(money.contains(&50000.0), "{v:?}");
}

#[test]
fn 날짜를_뽑는다() {
    let v = extract("2026. 3. 1. 부터 4월~11월 운영");
    let dates: Vec<f64> = v.iter().filter(|n| n.kind == Kind::Date).map(|n| n.value).collect();
    // 2026 은 뒤에 단위가 없어 맨 숫자, 3·1 도 마찬가지. 4월·11월 은 날짜.
    assert!(dates.contains(&4.0), "{v:?}");
    assert!(dates.contains(&11.0), "{v:?}");
}

#[test]
fn 근거에_있는지_본다() {
    let evidence = extract("경조사비는 1인당 50,000원 이내 집행 가능");
    // 답변이 다른 표기로 적어도 찾아야 한다
    assert!(contains(&evidence, &one("5만원")));
    // 근거에 없는 금액은 못 찾아야 한다
    assert!(!contains(&evidence, &one("70,000원")));
}

#[test]
fn 숫자가_없으면_빈_목록이다() {
    assert!(extract("학교운영위원회 심의를 거쳐야 합니다.").is_empty());
}

#[test]
fn 천원_단위_예산표도_값으로_읽는다() {
    // 정보통신윤리교육 계획의 예산표는 천원 단위로 적혀 있다: 85,000
    // 답변이 "8500만원" 이라고 쓰면 값이 다르므로 확인되지 않는다 —
    // **이것은 옳은 동작이다.** 단위가 다른 것을 같다고 하면 안 된다.
    let table = extract("총액 85,000");
    assert!(!contains(&table, &one("8500만원")), "{table:?}");
    assert!(contains(&table, &one("85,000")));
}

#[test]
fn 빈칸으로_갈린_숫자_조각을_버린다() {
    // ★ 실제 화면에서 나온 고장이다.
    //
    // PDF 의 쪽 번호가 자간 벌려 적혀 있으면(`- 1 0 0 -`) 조각이 나뉘어
    // `00` 같은 값이 뽑히고, 검증 표에 `숫자 00 · 근거7 에서 확인` 이 떴다.
    // 뜻 없는 값을 확인했다고 늘어놓으면 사용자가 표를 못 읽는다.
    let v = extract("- 1 00 -");
    assert!(
        v.iter().all(|n| n.value != 0.0),
        "뜻 없는 0 조각이 남았습니다: {v:?}"
    );

    // 단위가 붙은 0 은 뜻이 있으므로 남긴다
    let z = extract("연 0회");
    assert_eq!(z.len(), 1);
    assert_eq!(z[0].kind, Kind::Count);
    assert_eq!(z[0].value, 0.0);
}
