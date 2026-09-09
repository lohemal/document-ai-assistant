//! 두 검색 결과를 **순위로** 섞는다 (Reciprocal Rank Fusion).
//!
//! 낱말 검색은 BM25 를, 의미 검색은 코사인을 돌려준다. 두 숫자는 단위가 다르고
//! 자료마다 퍼지는 폭도 다르다. 그래서 점수를 더하면 안 된다 — 어느 쪽이 이길지
//! 자료가 정해 버린다. 대신 "몇 등이었는가" 만 쓴다.
//!
//!     점수(청크) = Σ  1 / (k + 그 목록에서의 등수)
//!
//! `k` 는 1등과 2등의 차이를 얼마나 크게 볼지 정한다. 작으면 1등을 몰아주고,
//! 크면 여러 목록에 두루 걸린 것을 밀어 준다. 원 논문의 60 을 기본으로 둔다.

/// 원 논문(Cormack 2009)의 값. 골든 셋으로 확인한 뒤에도 이대로 두었다 —
/// 자세히 조정해서 시험 문항에만 맞추면, 실제 자료에서 어떻게 될지 알 수 없다.
pub const DEFAULT_K: f64 = 60.0;

/// 각 목록은 **좋은 것부터** 늘어놓은 청크 id 다.
///
/// `weights` 가 있으면 목록마다 무게를 달리 준다(없으면 1). 지금은 쓰지 않지만,
/// 자료집 성격에 따라 한쪽을 더 믿어야 할 때가 올 수 있어 자리를 비워 둔다.
pub fn fuse(lists: &[Vec<i64>], k: f64, weights: Option<&[f64]>) -> Vec<(i64, f64)> {
    let mut acc: Vec<(i64, f64)> = Vec::new();
    for (li, list) in lists.iter().enumerate() {
        let w = weights.and_then(|ws| ws.get(li).copied()).unwrap_or(1.0);
        for (i, id) in list.iter().enumerate() {
            let add = w / (k + (i as f64 + 1.0));
            match acc.iter_mut().find(|(cid, _)| cid == id) {
                Some(slot) => slot.1 += add,
                None => acc.push((*id, add)),
            }
        }
    }
    // 점수가 같으면 id 가 작은 것부터 — 시험이 흔들리지 않게 못을 박는다
    acc.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 두_목록에_다_있으면_위로_올라간다() {
        // 1등 하나만 한 청크(10)와, 두 목록에서 2·2등 한 청크(20)
        let a = vec![10, 20, 30];
        let b = vec![40, 20, 50];
        let out = fuse(&[a, b], DEFAULT_K, None);
        assert_eq!(out[0].0, 20, "{out:?}");
    }

    #[test]
    fn 한_목록만_있으면_그_순서를_지킨다() {
        let out = fuse(&[vec![7, 8, 9]], DEFAULT_K, None);
        assert_eq!(out.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![7, 8, 9]);
    }

    #[test]
    fn k_가_작으면_1등을_더_밀어_준다() {
        // 1등만 한 청크 10 vs 2등 두 번 한 청크 20
        let lists = [vec![10, 99, 98], vec![97, 20, 96]];
        let lists2 = lists.clone();
        let small = fuse(&lists, 1.0, None);
        let big = fuse(&lists2, 60.0, None);
        assert_eq!(small[0].0, 10, "k 가 작으면 1등이 이겨야 합니다: {small:?}");
        // k 가 크면 둘의 차이가 줄어든다
        let gap_small = small[0].1 - small.iter().find(|(id, _)| *id == 20).unwrap().1;
        let gap_big = big[0].1 - big.iter().find(|(id, _)| *id == 20).unwrap().1;
        assert!(gap_big < gap_small, "작을 때 {gap_small}, 클 때 {gap_big}");
    }

    #[test]
    fn 빈_목록이_섞여도_괜찮다() {
        let out = fuse(&[vec![], vec![5]], DEFAULT_K, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 5);
    }
}
