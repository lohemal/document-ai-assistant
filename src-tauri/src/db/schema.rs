//! 자료구조. 전진 전용(forward-only)이다.
//!
//! 새 버전을 낼 때는 아래 `MIGRATIONS` 에 항목을 **덧붙이기만** 한다.
//! 이미 나간 버전의 SQL 은 고치지 않는다 — 남의 PC 에는 이미 그 모양으로
//! 만들어져 있기 때문이다.

/// (버전, 이 버전으로 올리는 SQL)
pub const MIGRATIONS: &[(i64, &str)] = &[(1, V1), (2, V2)];

const V1: &str = r#"
-- 자료집 -----------------------------------------------------------------
CREATE TABLE collection (
  id           INTEGER PRIMARY KEY,
  name         TEXT NOT NULL UNIQUE,
  -- 임베딩을 아직 한 번도 만들지 않았으면 NULL 이다.
  -- AI 모델 없이도 자료집을 쓸 수 있어야 하므로 NULL 을 허용한다.
  embed_model  TEXT,
  embed_dim    INTEGER,
  created_at   TEXT NOT NULL
);

-- 문서 -------------------------------------------------------------------
CREATE TABLE document (
  id            INTEGER PRIMARY KEY,
  collection_id INTEGER NOT NULL REFERENCES collection(id) ON DELETE CASCADE,
  title         TEXT NOT NULL,
  filename      TEXT NOT NULL,
  sha256        TEXT NOT NULL,
  byte_size     INTEGER NOT NULL,
  page_count    INTEGER NOT NULL DEFAULT 0,
  source_path   TEXT,
  issuer        TEXT,          -- 교육청 / 학교 / 기타 : 근거 충돌 판단 힌트
  issued_at     TEXT,          -- 발행일
  apply_year    INTEGER,       -- 적용연도
  -- ok | scanned | extract_failed | indexing
  status        TEXT NOT NULL DEFAULT 'indexing',
  -- none | partial | done : 임베딩이 어디까지 되었는가
  embed_state   TEXT NOT NULL DEFAULT 'none',
  revision_of   INTEGER REFERENCES document(id) ON DELETE SET NULL,
  superseded_by INTEGER REFERENCES document(id) ON DELETE SET NULL,
  created_at    TEXT NOT NULL
);
CREATE INDEX idx_document_collection ON document(collection_id);
CREATE INDEX idx_document_sha ON document(sha256);

-- 쪽 ---------------------------------------------------------------------
CREATE TABLE page (
  document_id INTEGER NOT NULL REFERENCES document(id) ON DELETE CASCADE,
  page        INTEGER NOT NULL,   -- 1부터. 물리 쪽
  label       TEXT,               -- PDF 안에 적힌 쪽 이름 (있으면)
  text        TEXT NOT NULL,
  is_scanned  INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (document_id, page)
);

-- 청크 -------------------------------------------------------------------
CREATE TABLE chunk (
  id           INTEGER PRIMARY KEY,
  document_id  INTEGER NOT NULL REFERENCES document(id) ON DELETE CASCADE,
  ord          INTEGER NOT NULL,   -- 문서 안 순서. 이웃 확장에 쓴다
  heading_path TEXT,               -- "Ⅲ. 프로그램 운영 > 3. 강사 채용"
  text         TEXT NOT NULL,      -- 화면에 근거로 보여 줄 원문
  text_norm    TEXT NOT NULL,      -- 검색 색인용 정규화본
  kind         TEXT NOT NULL DEFAULT 'text',   -- text | table
  page_start   INTEGER NOT NULL,
  page_end     INTEGER NOT NULL
);
CREATE INDEX idx_chunk_doc ON chunk(document_id, ord);

-- 청크가 쪽을 걸칠 때, 쪽마다의 정확한 문자 구간.
-- 근거를 원본 PDF 위에 형광펜으로 칠할 때 이 값을 쓴다.
CREATE TABLE chunk_span (
  chunk_id   INTEGER NOT NULL REFERENCES chunk(id) ON DELETE CASCADE,
  page       INTEGER NOT NULL,
  char_start INTEGER NOT NULL,     -- 그 쪽 page.text 안의 오프셋
  char_end   INTEGER NOT NULL
);
CREATE INDEX idx_chunk_span_chunk ON chunk_span(chunk_id);

-- 임베딩 -----------------------------------------------------------------
CREATE TABLE embedding (
  chunk_id INTEGER PRIMARY KEY REFERENCES chunk(id) ON DELETE CASCADE,
  model    TEXT NOT NULL,
  dim      INTEGER NOT NULL,
  vec      BLOB NOT NULL           -- f32 리틀엔디언
);

-- 낱말 검색. trigram 을 쓰는 이유는 한국어 조사 때문이다 —
-- unicode61 로는 "이용권을" 과 "이용권" 이 서로 다른 낱말이 된다.
CREATE VIRTUAL TABLE chunk_fts USING fts5(
  text_norm,
  content='chunk',
  content_rowid='id',
  tokenize='trigram'
);

-- external content 방식이므로 색인을 손으로 맞춰 준다
CREATE TRIGGER chunk_fts_ai AFTER INSERT ON chunk BEGIN
  INSERT INTO chunk_fts(rowid, text_norm) VALUES (new.id, new.text_norm);
END;
CREATE TRIGGER chunk_fts_ad AFTER DELETE ON chunk BEGIN
  INSERT INTO chunk_fts(chunk_fts, rowid, text_norm) VALUES ('delete', old.id, old.text_norm);
END;
CREATE TRIGGER chunk_fts_au AFTER UPDATE ON chunk BEGIN
  INSERT INTO chunk_fts(chunk_fts, rowid, text_norm) VALUES ('delete', old.id, old.text_norm);
  INSERT INTO chunk_fts(rowid, text_norm) VALUES (new.id, new.text_norm);
END;

-- 작업 기록 ---------------------------------------------------------------
CREATE TABLE job (
  id               INTEGER PRIMARY KEY,
  kind             TEXT NOT NULL,   -- search|interpret|draft|compare|summarize
  question         TEXT NOT NULL,
  collections_json TEXT NOT NULL,   -- 여러 자료집 선택을 미리 담아 둔다
  llm_model        TEXT,            -- 낱말로만 찾았으면 NULL
  embed_model      TEXT,            -- 임베딩 없이 찾았으면 NULL
  answer_json      TEXT NOT NULL,   -- 답변 + 검증 결과
  pinned           INTEGER NOT NULL DEFAULT 0,
  created_at       TEXT NOT NULL
);
CREATE INDEX idx_job_created ON job(created_at DESC);
CREATE INDEX idx_job_pinned ON job(pinned, created_at DESC);

-- 당시 근거를 통째로 복사해 둔다.
-- 문서를 지우거나 다시 색인해도 과거 기록이 온전해야 하기 때문이다.
CREATE TABLE job_evidence (
  job_id       INTEGER NOT NULL REFERENCES job(id) ON DELETE CASCADE,
  ord          INTEGER NOT NULL,    -- 답변 안의 [n] 번호와 같다
  document_id  INTEGER REFERENCES document(id) ON DELETE SET NULL,
  doc_title    TEXT NOT NULL,       -- 당시 이름
  doc_sha256   TEXT NOT NULL,       -- 당시 파일. 지금 파일과 다르면 알려 준다
  page         INTEGER NOT NULL,
  heading_path TEXT,
  quoted_text  TEXT NOT NULL,       -- 당시 원문 그대로
  chunk_id     INTEGER              -- 참고용. 사라질 수 있다
);
CREATE INDEX idx_job_evidence_job ON job_evidence(job_id, ord);

-- "이어서 질문하기" 자리. v0.1 은 쓰지 않지만 구조를 미리 둔다.
CREATE TABLE job_turn (
  job_id  INTEGER NOT NULL REFERENCES job(id) ON DELETE CASCADE,
  ord     INTEGER NOT NULL,
  role    TEXT NOT NULL,            -- user | assistant
  content TEXT NOT NULL
);

CREATE TABLE setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

INSERT INTO setting(key, value) VALUES
  ('retention_days', '30');
"#;

/// P2 에서 더한 것 — 문자 위치 지도.
///
/// 쪽마다 `[문자시작, 문자끝, 항목번호]` 를 담는다. 이게 나중에 "근거가 이 쪽
/// 이 자리에 있다"는 형광펜의 근거가 된다. 자세한 내용은 설계안 2-1.
///
/// 항목 번호는 pdf.js 가 준 차례이므로, 어느 판으로 뽑았는지 함께 적어 둔다.
const V2: &str = r#"
ALTER TABLE page ADD COLUMN item_map TEXT NOT NULL DEFAULT '[]';
ALTER TABLE document ADD COLUMN extractor TEXT;
"#;
