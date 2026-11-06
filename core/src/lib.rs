//! 매직키 코어 — 창(UI) 없이 테스트하는 로직.
//!   calc: 계산식 · date: 날짜 계산 · index: 앱 · 파일 · 폴더 이름 색인 · memo: 메모킹 메모 찾기 · web: 웹 검색 · AI 사이트 · text: 초성 검색

pub mod calc;
pub mod date;
pub mod index;
pub mod memo;
pub mod text;
pub mod web;

use serde::Serialize;

/// 계산기 줄 하나: value = 크게 보일 답, plain = 복사할 답, detail = 아래 작은 설명
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Answer {
    pub value: String,
    pub plain: String,
    pub detail: String,
}
