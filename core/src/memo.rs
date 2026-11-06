//! 메모킹 메모 — 메모 폴더의 .md 파일을 읽기만 해서 찾는다 (쓰기 · 열기는 메모킹이 한다).
//!   파일 모양: `---` 머리말(id · updated …) `---` + 마크다운 본문. 첫 줄이 제목.
//!   잠근 메모는 제목만 (암호문은 찾지 않음).

use crate::text::{choseong_index, is_choseong_query};
use fancy_regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;

/// 메모킹이 쓰는 폴더 (메모가 아님)
const APP_DIR: &str = "_memoking";
const ATTACH_DIR: &str = "attachments";
const MAX_BODY: usize = 200_000; // 이보다 긴 본문은 앞부분만 찾는다
pub const UNTITLED: &str = "제목 없음";
pub const LOCKED_TITLE: &str = "잠긴 메모";

#[derive(Debug, Clone)]
pub struct Memo {
    pub id: String,
    pub title: String,
    /// 찾기용 소문자 본문 (잠근 메모는 빈 글)
    body: String,
    pub locked: bool,
    pub updated: String,
    pub file: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoHit {
    pub id: String,
    pub title: String,
    /// 찾은 말 둘레의 본문 한 줄 (없으면 빈 글)
    pub snippet: String,
    pub locked: bool,
}

/// 메모킹 설정(settings.json)의 메모 폴더 — 비어 있으면 None (그때 메모킹은 문서\메모킹 을 쓴다)
pub fn vault_from_settings(json: &str) -> Option<PathBuf> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let p = v.get("vaultPath")?.as_str()?.trim();
    (!p.is_empty()).then(|| PathBuf::from(p))
}

static FRONT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\u{FEFF}?---\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n|$)").unwrap());
static LOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<!--\s*memoking-lock\s+v\d+\s").unwrap());
static BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s{0,3}#{1,6}\s+|^\s*>\s?|^\s*(?:[-*+]|\d+[.)])(?:\s+|$)(?:\\?\[[ xX]\](?:\s+|$))?").unwrap());
static INLINE: LazyLock<[(Regex, &'static str); 6]> = LazyLock::new(|| {
    [
        (Regex::new(r"!\[[^\]]*\]\([^)]*\)").unwrap(), " "),
        (Regex::new(r"\[([^\]]*)\]\([^)]*\)").unwrap(), "$1"),
        (Regex::new(r"<(https?://[^>]+)>").unwrap(), "$1"),
        (Regex::new(r"<[^>]*>").unwrap(), ""),
        (Regex::new(r"(\*\*|__|~~|`)(.+?)\1").unwrap(), "$2"),
        (Regex::new(r"\\(.)").unwrap(), "$1"),
    ]
});

/// 머리말의 `키: 값` 하나 (따옴표는 뗀다)
fn front_value<'a>(front: &'a str, key: &str) -> Option<&'a str> {
    front.lines().find_map(|l| {
        let v = l.strip_prefix(key)?.strip_prefix(':')?.trim();
        let v = v.trim_matches(|c| c == '"' || c == '\'');
        (!v.is_empty()).then_some(v)
    })
}

/// 본문 → 읽는 글자 줄들 (코드 울타리 · 구분선 · 한 줄 주석은 빼고, 마크다운 기호는 걷어낸다)
fn plain_lines(body: &str) -> impl Iterator<Item = String> + '_ {
    let mut fence = false;
    body.lines().filter_map(move |raw| {
        let t = raw.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            return None;
        }
        if fence {
            return (!t.is_empty()).then(|| t.to_string());
        }
        if (t.starts_with("<!--") && t.ends_with("-->")) || (t.len() >= 3 && t.chars().all(|c| c == '-' || c == '*' || c == '_' || c == ' ')) {
            return None;
        }
        let mut s = BLOCK.replace(raw, "").into_owned();
        for (re, to) in INLINE.iter() {
            s = re.replace_all(&s, *to).into_owned();
        }
        let s = s.trim().to_string();
        (!s.is_empty()).then_some(s)
    })
}

/// 메모 파일 하나 → 메모 (머리말에 id 가 없으면 None — 메모킹이 아직 정리하지 않은 파일)
pub fn parse(text: &str, file: PathBuf) -> Option<Memo> {
    let caps = FRONT.captures(text).ok()??;
    let front = caps.get(1)?.as_str();
    let id = front_value(front, "id")?;
    if !id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') || id.len() > 80 {
        return None;
    }
    let body = &text[caps.get(0)?.end()..];
    let locked = LOCK.is_match(body).unwrap_or(false);
    let mut lines = plain_lines(body);
    let first = lines.next();
    let title = match first {
        Some(t) if !(locked && t.starts_with('🔒')) => t.chars().take(80).collect(),
        _ if locked => LOCKED_TITLE.into(),
        _ => UNTITLED.into(),
    };
    let body = if locked {
        String::new()
    } else {
        let mut b: String = lines.collect::<Vec<_>>().join("\n").to_lowercase();
        if b.len() > MAX_BODY {
            let cut = (0..=MAX_BODY).rev().find(|&i| b.is_char_boundary(i)).unwrap_or(0);
            b.truncate(cut);
        }
        b
    };
    Some(Memo { id: id.into(), title, body, locked, updated: front_value(front, "updated").unwrap_or("").into(), file })
}

/// 메모 폴더의 .md 파일들 (메모킹처럼: 숨김 · 임시 파일, 맨 위 _memoking · attachments 폴더는 뺀다)
pub fn list_files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, top: bool, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.starts_with("~$") || (top && (name == APP_DIR || name == ATTACH_DIR)) {
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                walk(&e.path(), false, out);
            } else if ft.is_file() && name.to_lowercase().ends_with(".md") {
                out.push(e.path());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, true, &mut out);
    out
}

/// 읽어 둔 메모 — 다시 훑을 때 바뀐 파일만 새로 읽는다 (메모 폴더가 Google Drive 에 있어도 가볍게)
#[derive(Default)]
pub struct Shelf {
    seen: HashMap<PathBuf, (SystemTime, Option<Memo>)>,
}

impl Shelf {
    /// 폴더를 다시 훑어 지금 메모들을 돌려준다
    pub fn refresh(&mut self, root: &Path) -> Vec<Memo> {
        let mut next = HashMap::new();
        for f in list_files(root) {
            let Ok(mtime) = std::fs::metadata(&f).and_then(|m| m.modified()) else { continue };
            let memo = match self.seen.remove(&f) {
                Some((t, m)) if t == mtime => m,
                _ => std::fs::read_to_string(&f).ok().and_then(|s| parse(&s, f.clone())),
            };
            next.insert(f, (mtime, memo));
        }
        self.seen = next;
        let mut out: Vec<Memo> = self.seen.values().filter_map(|(_, m)| m.clone()).collect();
        out.sort_by(|a, b| b.updated.cmp(&a.updated)); // 최근에 고친 것부터
        out
    }
}

/// 찾은 말 둘레 한 줄 (앞뒤 몇 글자)
fn snippet(body: &str, at: usize) -> String {
    let start = body[..at].rfind('\n').map_or(0, |i| i + 1);
    let end = body[at..].find('\n').map_or(body.len(), |i| at + i);
    let line = &body[start..end];
    let pos = body[start..at].chars().count();
    let skip = pos.saturating_sub(20);
    let s: String = line.chars().skip(skip).take(90).collect();
    if skip > 0 { format!("…{s}") } else { s }
}

/// 제목 · 본문에서 찾기 — 낱말마다 제목에 있으면 크게, 본문에만 있으면 작게. 같으면 최근 것이 앞
pub fn search(memos: &[Memo], query: &str, limit: usize) -> (Vec<MemoHit>, usize) {
    let q = query.trim().to_lowercase();
    if q.is_empty() || limit == 0 {
        return (Vec::new(), 0);
    }
    let terms: Vec<&str> = q.split_whitespace().collect();
    let mut hits: Vec<(f32, usize, &Memo, Option<usize>)> = Vec::new();
    'next: for (i, m) in memos.iter().enumerate() {
        let title = m.title.to_lowercase();
        let mut score = 0.0;
        let mut first_body = None;
        for t in &terms {
            score += if let Some(at) = title.find(t) {
                if at == 0 { 4.0 } else { 3.0 }
            } else if is_choseong_query(t) && choseong_index(&m.title, t).is_some() {
                2.5
            } else if let Some(at) = m.body.find(t) {
                first_body.get_or_insert(at);
                1.0
            } else {
                continue 'next;
            };
        }
        hits.push((score, i, m, first_body));
    }
    let total = hits.len();
    hits.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let out: Vec<MemoHit> = hits
        .into_iter()
        .take(limit)
        .map(|(_, _, m, at)| MemoHit { id: m.id.clone(), title: m.title.clone(), snippet: at.map(|a| snippet(&m.body, a)).unwrap_or_default(), locked: m.locked })
        .collect();
    let more = total - out.len();
    (out, more)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memo(id: &str, updated: &str, body: &str) -> Memo {
        parse(&format!("---\nid: {id}\nupdated: '{updated}'\ncolor: yellow\n---\n{body}"), PathBuf::from(format!("{id}.md"))).unwrap()
    }

    #[test]
    fn parse_title_and_body() {
        let m = memo("a1", "2026-11-01T09:00:00+09:00", "# **회의** 메모\n\n- [ ] 견적서 [보내기](https://x.y)\n```\ncode\n```\n");
        assert_eq!((m.id.as_str(), m.title.as_str(), m.locked), ("a1", "회의 메모", false));
        assert_eq!(m.body, "견적서 보내기\ncode");
        assert_eq!(m.updated, "2026-11-01T09:00:00+09:00");
        assert!(parse("제목만 있는 파일", PathBuf::new()).is_none(), "머리말 · id 없으면 뺀다");
        assert!(parse("---\nid: ../x\n---\n본문", PathBuf::new()).is_none(), "이상한 id 는 뺀다");
        assert_eq!(memo("b", "", "\n\n").title, UNTITLED);
    }

    #[test]
    fn locked_memo_shows_title_only() {
        let body = "# 통장 비번\n🔒 잠긴 메모예요. 메모킹에서 비밀번호를 넣으면 열려요.\n<!-- memoking-lock v1 n=1 s=AA== i=BB== c=CC== -->\n";
        let m = memo("l", "", body);
        assert_eq!((m.title.as_str(), m.locked, m.body.as_str()), ("통장 비번", true, ""));
        let bare = memo("l2", "", "🔒 잠긴 메모예요. 메모킹에서 비밀번호를 넣으면 열려요.\n<!-- memoking-lock v1 n=1 s=A i=B c=C -->");
        assert_eq!(bare.title, LOCKED_TITLE);
    }

    #[test]
    fn search_title_first_then_recent() {
        let mut v = vec![
            memo("old", "2026-01-01", "장보기\n견적서 출력"),
            memo("new", "2026-11-01", "오늘 할 일\n견적서 보내기"),
            memo("t", "2025-01-01", "견적서 양식"),
        ];
        v.sort_by(|a, b| b.updated.cmp(&a.updated));
        let (hits, more) = search(&v, "견적서", 2);
        assert_eq!(hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), ["t", "new"], "제목에 든 것 > 최근에 고친 것");
        assert_eq!((hits[1].snippet.as_str(), more), ("견적서 보내기", 1));
        assert_eq!(search(&v, "ㅈㅂㄱ", 5).0[0].id, "old", "제목 초성");
        assert!(search(&v, "견적서 없는말", 5).0.is_empty(), "낱말이 모두 있어야");
    }

    #[test]
    fn settings_and_shelf() {
        assert_eq!(vault_from_settings(r#"{"vaultPath":"D:\\메모"}"#), Some(PathBuf::from("D:\\메모")));
        assert_eq!(vault_from_settings(r#"{"vaultPath":""}"#), None);
        assert_eq!(vault_from_settings("깨진 글"), None);

        let dir = std::env::temp_dir().join(format!("magickey-memo-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("_memoking/trash")).unwrap();
        std::fs::create_dir_all(dir.join("일")).unwrap();
        std::fs::write(dir.join("일/a.md"), "---\nid: a\n---\n첫 메모").unwrap();
        std::fs::write(dir.join("_memoking/trash/x.md"), "---\nid: x\n---\n지운 메모").unwrap();
        std::fs::write(dir.join("~$tmp.md"), "---\nid: t\n---\n임시").unwrap();
        let mut shelf = Shelf::default();
        let got = shelf.refresh(&dir);
        assert_eq!(got.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(), ["첫 메모"]);
        std::fs::remove_file(dir.join("일/a.md")).unwrap();
        assert!(shelf.refresh(&dir).is_empty(), "지운 파일은 빠진다");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
