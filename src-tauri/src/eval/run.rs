//! 실제로 재고, 표를 찍는다.
//!
//!     cargo test --lib eval -- --nocapture
//!
//! 시험이 "통과" 만 하고 아무 말도 안 하면 품질을 알 수 없다. 그래서 이
//! 시험들은 숫자를 찍는다. 못을 박는 곳은 **아래로 내려가면 안 되는 선**만이다
//! (목표 점수를 시험에 적어 두면, 나중에 그 문항에만 맞추게 된다).

use super::*;

/// 낱말 검색만은 벡터 없이도 언제나 잴 수 있다 — CI 에서도 돈다.
#[test]
fn 낱말_검색을_실제_자료로_잰다() {
    let corpus = load_corpus();
    let (conn, ids) = build(&corpus, None);
    let qs = load_golden();

    let total: usize = corpus.documents.iter().map(|d| d.chunks.len()).sum();
    println!("\n── 실제 업무자료 {}종 · 청크 {total}개 · 물음 {}개", corpus.documents.len(), qs.len());

    let mut all = Score::default();
    let mut filtered = Score::default();
    let mut by_doc: HashMap<i64, Score> = HashMap::new();
    let mut by_type: HashMap<String, Score> = HashMap::new();

    for q in &qs {
        let want = wanted(q, &ids);
        assert!(!want.is_empty(), "{}: 정답 청크를 못 찾았습니다", q.question_id);

        let r = run_keyword(&conn, q, vec![]);
        let rank = first_rank(&r.order, &want);
        all.add(rank, r.ms);
        by_doc.entry(q.document_id).or_default().add(rank, r.ms);
        by_type.entry(q.question_type.clone()).or_default().add(rank, r.ms);

        // 자료집을 골랐을 때는 얼마나 쉬워지는가
        let f = run_keyword(&conn, q, vec![q.collection_id]);
        filtered.add(first_rank(&f.order, &want), f.ms);
    }

    println!("\n[낱말 검색]");
    println!("  {}", all.line("전체"));
    println!("  {}", filtered.line("자료집 고름"));

    println!("\n  자료별");
    for d in &corpus.documents {
        if let Some(s) = by_doc.get(&d.id) {
            println!("    {}", s.line(&d.title.chars().take(10).collect::<String>()));
        }
    }
    println!("\n  물음 유형별");
    for (code, name) in TYPE_NAME {
        if let Some(s) = by_type.get(code) {
            println!("    {}", s.line(name));
        }
    }

    // 내려가면 안 되는 선. 실제 자료에서 재어 본 값보다 넉넉히 아래에 둔다.
    assert!(all.r5() >= 40.0, "R@5 가 {:.1}% 로 떨어졌습니다", all.r5());
    assert!(all.avg_ms() < 300.0, "검색이 느려졌습니다: 평균 {:.1}ms", all.avg_ms());
}

/// 뜻·섞기까지 세 방법을 **같은 골든 셋, 같은 자료**로 견준다.
#[test]
fn 세_방법을_같은_조건으로_견준다() {
    let all = match load_all_vectors() {
        Ok(v) => v,
        Err(why) => {
            println!("\n[뜻·섞기] 재지 못했습니다 — {why}");
            return;
        }
    };
    let corpus = load_corpus();
    let qs = load_golden();

    // 검색 모델을 여러 벌 만들어 두었으면 모두 재고 견준다
    for vs in all.iter() {
    let (conn, ids) = build(&corpus, Some(vs));

    println!("\n── 검색 모델 {} · {}차원", vs.model, vs.dim);

    let mut kw = Score::default();
    let mut sem = Score::default();
    let mut hyb = Score::default();
    let mut by_type: HashMap<String, (Score, Score, Score)> = HashMap::new();
    let mut by_doc: HashMap<i64, (Score, Score, Score)> = HashMap::new();

    for q in &qs {
        let want = wanted(q, &ids);
        let qv = vs
            .questions
            .get(&q.question_id)
            .unwrap_or_else(|| panic!("{} 의 물음 벡터가 없습니다", q.question_id));

        let a = run_keyword(&conn, q, vec![]);
        let b = run_semantic(&conn, qv, vec![]);
        let c = run_hybrid(&conn, q, qv, vec![], hybrid::DEFAULT_DEPTH);

        let (ra, rb, rc) = (
            first_rank(&a.order, &want),
            first_rank(&b.order, &want),
            first_rank(&c.order, &want),
        );
        kw.add(ra, a.ms);
        sem.add(rb, b.ms);
        hyb.add(rc, c.ms);

        let t = by_type.entry(q.question_type.clone()).or_default();
        t.0.add(ra, a.ms);
        t.1.add(rb, b.ms);
        t.2.add(rc, c.ms);
        let d = by_doc.entry(q.document_id).or_default();
        d.0.add(ra, a.ms);
        d.1.add(rb, b.ms);
        d.2.add(rc, c.ms);
    }

    println!("\n[세 방법 · 물음 {}개]", qs.len());
    println!("  {}", kw.line("낱말"));
    println!("  {}", sem.line("뜻"));
    println!("  {}", hyb.line("섞기(RRF)"));

    println!("\n  물음 유형별 (P@1 / R@5)");
    println!("    {:<10} {:^15} {:^15} {:^15}", "", "낱말", "뜻", "섞기");
    for (code, name) in TYPE_NAME {
        if let Some((a, b, c)) = by_type.get(code) {
            println!(
                "    {name:<10} {:>6.1}% {:>6.1}%  {:>6.1}% {:>6.1}%  {:>6.1}% {:>6.1}%   ({}문항)",
                a.p1(), a.r5(), b.p1(), b.r5(), c.p1(), c.r5(), a.n
            );
        }
    }
    println!("\n  자료별 (P@1 / R@5)");
    for d in &corpus.documents {
        if let Some((a, b, c)) = by_doc.get(&d.id) {
            println!(
                "    {:<12} {:>6.1}% {:>6.1}%  {:>6.1}% {:>6.1}%  {:>6.1}% {:>6.1}%   ({}문항)",
                d.title.chars().take(11).collect::<String>(),
                a.p1(), a.r5(), b.p1(), b.r5(), c.p1(), c.r5(), a.n
            );
        }
    }

    println!("\n  한 판에 걸린 시간: 낱말 {:.1}ms · 뜻 {:.1}ms · 섞기 {:.1}ms",
             kw.avg_ms(), sem.avg_ms(), hyb.avg_ms());

    // 섞기가 두 방법보다 크게 나쁘면 그건 고장이다
    assert!(
        hyb.r5() + 5.0 >= kw.r5().max(sem.r5()),
        "섞기 R@5 {:.1}% 가 한쪽만 쓴 것({:.1}% / {:.1}%)보다 못합니다",
        hyb.r5(), kw.r5(), sem.r5()
    );
    }
}

/// 못 찾은 물음을 하나씩 들여다본다. 평균보다 이쪽이 더 중요하다.
#[test]
fn 못_찾은_물음을_들여다본다() {
    let vs = load_vectors().ok();
    let corpus = load_corpus();
    let (conn, ids) = build(&corpus, vs.as_ref());
    let qs = load_golden();

    println!("\n[못 찾은 물음]  (— 은 10등 안에 없음)");
    let mut bad = 0;
    for q in &qs {
        let want = wanted(q, &ids);
        let kwr = first_rank(&run_keyword(&conn, q, vec![]).order, &want);
        let (semr, hybr) = match (&vs, vs.as_ref().and_then(|v| v.questions.get(&q.question_id))) {
            (Some(_), Some(qv)) => (
                first_rank(&run_semantic(&conn, qv, vec![]).order, &want),
                first_rank(&run_hybrid(&conn, q, qv, vec![], hybrid::DEFAULT_DEPTH).order, &want),
            ),
            _ => (None, None),
        };

        // 벡터가 없으면 낱말만 보고 판단한다 (없는 것을 실패로 세면 전부 나온다)
        let ok = |r: Option<usize>| r.is_some_and(|n| n <= 5);
        if ok(kwr) && (vs.is_none() || ok(hybr)) {
            continue; // 쓸 수 있는 방법이 다 잘 찾았다
        }
        bad += 1;
        let show = |r: Option<usize>| r.map(|n| n.to_string()).unwrap_or("—".into());
        println!(
            "  {} [{}] {}\n      낱말 {} · 뜻 {} · 섞기 {} · 정답 {}쪽 {} 청크 {:?}",
            q.question_id,
            q.question_type,
            q.question,
            show(kwr), show(semr), show(hybr),
            q.expected_pages.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(","),
            if q.kinds.iter().any(|k| k == "table") { "(표)" } else { "" },
            q.expected_chunk_ids
        );
    }
    println!("  — 들여다볼 물음 {bad}개");
}

/// 섞을 때 각 방법에서 몇 개를 가져올지, RRF 의 k 를 얼마로 할지.
///
/// **자세히 조정하지 않는다.** 몇 가지 값에서 크게 흔들리지 않는지만 본다.
/// 시험 문항에 맞춰 소수점을 맞추면 실제 자료에서 어떻게 될지 알 수 없다.
#[test]
fn 섞는_깊이와_k_가_많이_흔들리지_않는다() {
    let Ok(vs) = load_vectors() else {
        println!("\n[깊이·k] 벡터가 없어 재지 못했습니다.");
        return;
    };
    let corpus = load_corpus();
    let (conn, ids) = build(&corpus, Some(&vs));
    let qs = load_golden();

    println!("\n[가져올 후보 수(depth)]");
    for depth in [10, 20, 30, 50] {
        let mut s = Score::default();
        for q in &qs {
            let want = wanted(q, &ids);
            let qv = &vs.questions[&q.question_id];
            let r = run_hybrid(&conn, q, qv, vec![], depth);
            s.add(first_rank(&r.order, &want), r.ms);
        }
        println!("  {}", s.line(&format!("depth {depth}")));
    }

    println!("\n[RRF k]");
    for k in [10.0, 30.0, 60.0, 120.0] {
        let mut s = Score::default();
        for q in &qs {
            let want = wanted(q, &ids);
            let qv = &vs.questions[&q.question_id];
            let t = std::time::Instant::now();
            let kw = keyword_search(
                &conn,
                &Request { text: q.question.clone(), collection_ids: vec![], limit: hybrid::DEFAULT_DEPTH },
            )
            .unwrap();
            let (sem, _) = vector::nearest(&conn, qv, &[], hybrid::DEFAULT_DEPTH as usize).unwrap();
            let lists = vec![
                kw.hits.iter().map(|h| h.chunk_id).collect::<Vec<_>>(),
                sem.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            ];
            let fused = crate::domain::rrf::fuse(&lists, k, None);
            let order: Vec<i64> = fused.iter().take(TOP as usize).map(|(id, _)| *id).collect();
            s.add(first_rank(&order, &want), t.elapsed().as_secs_f64() * 1000.0);
        }
        println!("  {}", s.line(&format!("k {k:.0}")));
    }
}

/// 자료가 더 늘어도 쓸 만한가. 실제 청크를 여러 번 복사해 부풀린다.
#[test]
fn 자료가_늘어도_버틴다() {
    let vs = load_vectors().ok();
    let corpus = load_corpus();
    let (conn, _) = build(&corpus, vs.as_ref());

    let base: usize = corpus.documents.iter().map(|d| d.chunks.len()).sum();
    // 같은 청크를 문서만 바꿔 여러 벌 더 넣는다 (학교 한 곳이 몇 해치를 쌓은 꼴)
    const COPIES: i64 = 12;
    for copy in 1..=COPIES {
        let doc_id = 100 + copy;
        conn.execute(
            "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size,
                                  page_count, status, created_at)
             VALUES (?1, 1, ?2, 'x.pdf', ?2, 1, 1, 'ok', '2026-09-09')",
            rusqlite::params![doc_id, format!("불린 자료 {copy}")],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chunk(document_id, ord, heading_path, text, text_norm, kind,
                               page_start, page_end)
             SELECT ?1, ord, heading_path, text, text_norm, kind, page_start, page_end
               FROM chunk WHERE document_id <= 3",
            [doc_id],
        )
        .unwrap();
        if let Some(v) = &vs {
            // 벡터도 함께 불린다 — 뜻 검색이 훑을 거리를 실제와 비슷하게
            conn.execute(
                "INSERT INTO embedding(chunk_id, model, dim, vec)
                 SELECT c2.id, e.model, e.dim, e.vec
                   FROM chunk c1
                   JOIN embedding e ON e.chunk_id = c1.id
                   JOIN chunk c2 ON c2.document_id = ?1 AND c2.ord = c1.ord
                  WHERE c1.document_id <= 3",
                [doc_id],
            )
            .ok();
            let _ = v;
        }
    }

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunk", [], |r| r.get(0))
        .unwrap();
    let vecs: i64 = conn
        .query_row("SELECT COUNT(*) FROM embedding", [], |r| r.get(0))
        .unwrap();

    let questions = [
        "방과후학교 자유수강권 지원 대상은?",
        "경조사비는 1인당 얼마까지 집행할 수 있어?",
        "정보통신윤리교육 추진 일정은 어떻게 돼?",
    ];

    let t = std::time::Instant::now();
    for q in questions {
        keyword_search(
            &conn,
            &Request { text: q.into(), collection_ids: vec![], limit: 10 },
        )
        .unwrap();
    }
    let kw_ms = t.elapsed().as_secs_f64() * 1000.0 / questions.len() as f64;

    let sem_ms = match &vs {
        Some(v) => {
            let qv = v.questions.values().next().unwrap().clone();
            let t = std::time::Instant::now();
            for _ in 0..questions.len() {
                vector::nearest(&conn, &qv, &[], 10).unwrap();
            }
            Some(t.elapsed().as_secs_f64() * 1000.0 / questions.len() as f64)
        }
        None => None,
    };

    println!(
        "\n[자료를 불렸을 때]  청크 {base} → {total}개 · 벡터 {vecs}개\n  낱말 {kw_ms:.0}ms{}",
        sem_ms.map(|m| format!(" · 뜻 {m:.0}ms (전부 훑음)")).unwrap_or_default()
    );

    assert!(kw_ms < 1000.0, "낱말 검색이 {kw_ms:.0}ms 나 걸립니다");
    if let Some(m) = sem_ms {
        assert!(m < 2000.0, "뜻 검색이 {m:.0}ms 나 걸립니다 — 색인 구조를 생각할 때입니다");
    }
}
