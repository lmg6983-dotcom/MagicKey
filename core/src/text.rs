//! 이름 맞추기: 초성 검색 (ㄱㅈㅅ → 견적서) · 기호 뺀 비교 (P2026-1170-KR01 ↔ P20261170KR01)

const CHO: [char; 19] = ['ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ'];

/// 자음만 쓴 낱말인가 (ㄱ-ㅎ)
pub fn is_choseong_query(term: &str) -> bool {
    !term.is_empty() && term.chars().all(|c| ('ㄱ'..='ㅎ').contains(&c))
}

/// 글자 하나의 초성 (한글이 아니면 그 글자 소문자)
fn cho_of(c: char) -> char {
    match c as u32 {
        0xAC00..=0xD7A3 => CHO[((c as u32 - 0xAC00) / 588) as usize],
        _ => c.to_ascii_lowercase(),
    }
}

/// 초성 검색어가 이름에 들어 있으면 띄어쓰기를 뺀 글자 번호, 없으면 None
pub fn choseong_index(text: &str, term: &str) -> Option<usize> {
    let t: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).map(cho_of).collect();
    let q: Vec<char> = term.chars().collect();
    if q.is_empty() {
        return Some(0);
    }
    t.windows(q.len()).position(|w| w == q.as_slice())
}

/// 기호(띄어쓰기 포함)를 뺀 소문자 글
pub fn compact(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// 기호를 빼고 비교할 낱말이면 그 글: 숫자가 있고 뺀 뒤에도 4글자 이상 (짧은 말은 엉뚱한 곳에 걸리기 쉬워)
pub fn compact_term(term: &str) -> Option<String> {
    let c = compact(term);
    (c.chars().any(|ch| ch.is_ascii_digit()) && c.chars().count() >= 4).then_some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choseong() {
        assert!(is_choseong_query("ㄱㅈㅅ"));
        assert!(!is_choseong_query("견적"));
        assert_eq!(choseong_index("견적서.pdf", "ㄱㅈㅅ"), Some(0));
        assert_eq!(choseong_index("2026 회의 록", "ㅎㅇㄹ"), Some(4));
        assert_eq!(choseong_index("견적서", "ㅎㅇ"), None);
    }

    #[test]
    fn compact_terms() {
        assert_eq!(compact_term("P2026-1170-KR01").as_deref(), Some("p20261170kr01"));
        assert_eq!(compact_term("a-1"), None);
        assert_eq!(compact_term("보고서"), None);
    }
}
