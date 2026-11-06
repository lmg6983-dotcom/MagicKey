//! OPMS 에 물을 글인가 — 그렇다면 "OPMS 에서 찾기" 줄로 MagicOPMS 를 부른다
//!   MagicOPMS core/src/search.rs 의 query_kind 와 같은 규칙 (바꾸면 둘 다): 번호(일부도) · 이름 · 초성
//!   문장 · 긴 글 · 한 글자 · 여러 줄은 묻지 않는다

use crate::text::is_choseong_query;
use fancy_regex::Regex;
use std::sync::LazyLock;

const MAX_WORDS: usize = 3;
const MAX_NAME: usize = 20;

static NAME_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[가-힣A-Za-z().&·-]+$").unwrap());
static HANGUL2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[가-힣]{2,}").unwrap());
static LATIN3: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z]{3,}").unwrap());

fn len(s: &str) -> usize {
    s.chars().count()
}

/// 사건번호 · 출원인 · 발명자 · 초성처럼 OPMS 에 물을 만한 글인가
pub fn looks_like_opms_query(text: &str) -> bool {
    let s = text.trim();
    if s.is_empty() || s.contains('\n') || len(s) > 40 {
        return false;
    }
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.iter().all(|w| is_choseong_query(w)) && len(&s.replace(char::is_whitespace, "")) >= 2 {
        return true; // 초성
    }
    if s.chars().any(|c| c.is_ascii_digit()) {
        return s.chars().filter(char::is_ascii_alphanumeric).count() >= 3; // 번호 (일부도)
    }
    if words.len() > MAX_WORDS || len(s) > MAX_NAME || words.iter().any(|w| len(w) < 2) {
        return false;
    }
    let is = |re: &Regex, t: &str| re.is_match(t).unwrap_or(false);
    words.iter().all(|w| is(&NAME_WORD, w)) && (is(&HANGUL2, s) || is(&LATIN3, s)) // 이름
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_rule_as_magicopms() {
        for q in ["FPD/2026-0271/E", "0271", "1020260012345", "2026-0012", "P-1234", "10-2026", "홍길동", "홍길", "메모킹 홍길동", "Samsung", "FPD", "(주)메모킹", "ㅎㄱㄷ", "ㅎㄱ ㄷ"] {
            assert!(looks_like_opms_query(q), "{q}");
        }
        for q in ["", "홍", "ㅎ", "12", "ab", "내일 오후 3시까지 보고서 정리해서 보내기", "첫 줄\n홍길동", "이것 저것 그것 요것", "회의 준비?"] {
            assert!(!looks_like_opms_query(q), "{q}");
        }
    }
}
