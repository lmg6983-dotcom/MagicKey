//! 웹 검색 · AI 에게 묻기 · 사이트 열기 — 치는 동안에는 아무것도 보내지 않고, 고른 줄을 실행할 때만 주소를 만든다

use fancy_regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Site {
    pub id: &'static str,
    pub name: &'static str,
    #[serde(skip)]
    url: &'static str, // {q} 자리에 검색어
    /// 주소로 질문을 넘길 수 없는 사이트 — 질문을 복사해 두고 연다
    pub paste: bool,
}

const fn site(id: &'static str, name: &'static str, url: &'static str) -> Site {
    Site { id, name, url, paste: false }
}

/// 첫 번째가 Enter
pub const WEB_ENGINES: &[Site] = &[
    site("naver", "네이버", "https://search.naver.com/search.naver?query={q}"),
    site("google", "Google", "https://www.google.com/search?q={q}"),
    site("youtube", "YouTube", "https://www.youtube.com/results?search_query={q}"),
    site("bing", "Bing", "https://www.bing.com/search?q={q}"),
    site("duckduckgo", "DuckDuckGo", "https://duckduckgo.com/?q={q}"),
];

pub const AI_SITES: &[Site] = &[
    site("chatgpt", "ChatGPT", "https://chatgpt.com/?q={q}"),
    site("claude", "Claude", "https://claude.ai/new?q={q}"),
    Site { id: "gemini", name: "Gemini", url: "https://gemini.google.com/app", paste: true },
    site("perplexity", "Perplexity", "https://www.perplexity.ai/search?q={q}"),
];

/// JS encodeURIComponent 와 같게
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub struct Link {
    pub url: String,
    pub paste: bool,
    pub name: &'static str,
}

/// 이 사이트에서 이 글을 찾는 주소 — 모르는 사이트 · 빈 글이면 None
pub fn site_link(list: &[Site], id: &str, query: &str) -> Option<Link> {
    let site = list.iter().find(|s| s.id == id)?;
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    Some(Link { url: site.url.replace("{q}", &encode(q)), paste: site.paste, name: site.name })
}

static TLD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\.(com|net|org|kr|io|ai|dev|app|me|so|co|gg|tv|info|biz|xyz|page|site|link|jp|us|uk|edu|gov|ly|to|fm|im|in|cn|de|fr|news|blog|shop|store|cloud|tech|wiki)(/|:|\?|#|$)").unwrap()
});
static HTTP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^https?://[^/\s]+\.[^/\s]+").unwrap());
static HOST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\w-]+(\.[\w-]+)+(:\d+)?([/?#]\S*)?$").unwrap());

/// 사이트 주소처럼 보이면 열 주소 (example.com, www.naver.com/abc, https://…) — 파일 이름(report.pdf)은 아님
pub fn site_url(text: &str) -> Option<String> {
    let s = text.trim();
    if s.is_empty() || s.chars().any(char::is_whitespace) || s.len() > 2000 {
        return None;
    }
    if HTTP.is_match(s).unwrap_or(false) {
        return Some(s.to_string());
    }
    let host = s.strip_prefix("www.").or_else(|| s.strip_prefix("WWW.")).unwrap_or(s);
    (HOST.is_match(host).unwrap_or(false) && TLD.is_match(s).unwrap_or(false)).then(|| format!("https://{s}"))
}

static ASK_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(뭐|왜|어떻게|어떤|언제|어디|누가|누구|무엇|몇|얼마|how|what|why|when|where|who|which|can|should|is|are|do|does)(\s|$)").unwrap()
});
static ASK_END: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(해\s?줘|알려\s?줘|해\s?주세요|알려\s?주세요|뭐야|뭐지|뭔가요|인가요|까|까요|나요|는지|어때|explain|summarize|translate)$").unwrap()
});

/// 묻는 글인가 (AI 에게 묻기 줄을 위로 올릴지) — "?" · 묻는 말로 시작 · 부탁하는 끝말
pub fn looks_like_question(text: &str) -> bool {
    let s = text.trim();
    if s.chars().count() < 2 || s.contains('\n') {
        return false;
    }
    s.ends_with('?') || s.ends_with('？') || ASK_START.is_match(s).unwrap_or(false) || ASK_END.is_match(s).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links() {
        assert_eq!(
            site_link(WEB_ENGINES, "naver", " 날씨 서울 ").unwrap().url,
            "https://search.naver.com/search.naver?query=%EB%82%A0%EC%94%A8%20%EC%84%9C%EC%9A%B8"
        );
        assert_eq!(site_link(AI_SITES, "chatgpt", "a&b").unwrap().url, "https://chatgpt.com/?q=a%26b");
        assert_eq!(site_link(AI_SITES, "claude", "안녕").unwrap().url, "https://claude.ai/new?q=%EC%95%88%EB%85%95");
        assert!(site_link(AI_SITES, "gemini", "안녕").unwrap().paste);
        assert!(site_link(WEB_ENGINES, "nope", "x").is_none());
        assert!(site_link(WEB_ENGINES, "google", "   ").is_none());
    }

    #[test]
    fn sites() {
        assert_eq!(site_url("naver.com").as_deref(), Some("https://naver.com"));
        assert_eq!(site_url("www.youtube.com/watch?v=1").as_deref(), Some("https://www.youtube.com/watch?v=1"));
        assert_eq!(site_url("https://example.org/a b"), None);
        assert_eq!(site_url("http://localhost.test/x").as_deref(), Some("http://localhost.test/x"));
        for t in ["report.pdf", "memo.md", "1.5", "naver com", "회의.com 메모"] {
            assert_eq!(site_url(t), None, "{t}");
        }
    }

    #[test]
    fn questions() {
        for q in ["환율 뭐야", "이 메일 정리해줘", "회의록 요약해 주세요", "파이썬 리스트 정렬은?", "왜 하늘은 파란가", "how to center a div", "내일 비 올까", "이거 맞는지"] {
            assert!(looks_like_question(q), "{q}");
        }
        for q in ["내일 3시 회의", "견적서", "1,200 × 3", "naver.com", "장보기: 우유, 계란", "", "첫 줄\n둘째 줄?"] {
            assert!(!looks_like_question(q), "{q}");
        }
    }
}
