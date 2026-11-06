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
    /// 본문 (읽는 글자만, 잠근 메모는 빈 글) · 찾기용 소문자 제목 · 본문 (글자 수가 같아 찾은 자리를 그대로 옮길 수 있다)
    body: String,
    title_lower: String,
    lower: String,
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

/// 메모킹 매직키 단축키(settings.json 의 shortcuts.magic, Electron 표기 "Control+Alt+Space") → MagicKey 표기 ("ctrl+alt+Space")
///   MagicKey 에서 단축키를 아직 바꾸지 않았으면 이걸 쓴다 (메모킹에서 쓰던 키 그대로).
///   메모킹이 MagicKey 에게 키를 넘기는 버전일 때만 — 옛 메모킹은 그 키를 계속 잡고 있어 MagicKey 가 못 잡는다
pub fn magic_key(json: &str) -> Option<String> {
    if !knows_magickey(json) {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let acc = v.get("shortcuts")?.get("magic")?.as_str()?;
    let parts: Vec<String> = acc
        .split('+')
        .map(|t| match t.trim().to_lowercase().as_str() {
            "control" | "ctrl" | "commandorcontrol" | "cmdorctrl" => "ctrl".into(),
            "alt" | "option" => "alt".into(),
            "shift" => "shift".into(),
            "super" | "meta" | "command" | "cmd" => "super".into(),
            _ => t.trim().to_string(),
        })
        .collect();
    (parts.len() >= 2 && parts.iter().all(|p| !p.is_empty())).then(|| parts.join("+"))
}

/// 메모킹 매직키에서 꺼 둔 기능 (설정에서 끈 것 · 창에서 잠깐 끈 것) — MagicKey 에서 캡슐을 아직 만지지 않았으면 이대로
///   메모킹의 'ai'(매직 에이전트 + 웹 AI 단추)를 끄면 MagicKey 의 웹 AI 도 끈다. 매직키를 통째로 껐으면 따르지 않는다
pub fn magic_off(json: &str) -> Option<Vec<String>> {
    if !knows_magickey(json) {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let magic = v.get("magic")?;
    if magic.get("enabled").and_then(|e| e.as_bool()) == Some(false) {
        return None;
    }
    let off = |key: &str, id: &str, want: bool| magic.get(key).and_then(|m| m.get(id)).and_then(|b| b.as_bool()) == Some(want);
    Some(["memo", "notes", "apps", "files", "calc", "web", "ai"].iter().filter(|id| off("modules", id, false) || off("paused", id, true)).map(|id| id.to_string()).collect())
}

/// 메모킹이 MagicKey 와 함께 쓰기를 아는 첫 버전 — `--new-memo-file=` · `--remind` 를 알아듣고, MagicKey 가 있으면 매직키 단축키를 넘긴다
pub const MAGICKEY_SINCE: (u32, u32, u32) = (0, 77, 0);

/// 메모킹이 마지막으로 켜진 버전(settings.json 의 seenVersion)이 MagicKey 와 함께 쓰기를 아는가
pub fn knows_magickey(json: &str) -> bool {
    let v: Option<serde_json::Value> = serde_json::from_str(json).ok();
    let ver = v.as_ref().and_then(|v| v.get("seenVersion")?.as_str().map(str::to_string)).unwrap_or_default();
    let mut n = ver.trim_start_matches('v').split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    (n.next().unwrap_or(0), n.next().unwrap_or(0), n.next().unwrap_or(0)) >= MAGICKEY_SINCE
}

static FRONT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\u{FEFF}?---\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n|$)").unwrap());
// 잠근 메모 본문 전체의 모양 (메모킹 memo-lock.js LOCK_RE 와 같게): (제목 줄) (안내 한 줄) 암호문 주석 — 다른 글이 섞이면 보통 메모
static LOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^\u{FEFF}?\s*(?:#[^\n]*\n\s*)?(?:🔒 잠긴 메모예요\. 메모킹에서 비밀번호를 넣으면 열려요\.\s*)?",
        r"<!--\s*memoking-lock\s+v\d+\s+n=\d+\s+s=[A-Za-z0-9+/=]+\s+i=[A-Za-z0-9+/=]+\s+c=[A-Za-z0-9+/=]+\s*-->\s*$"
    ))
    .unwrap()
});

/// 글자마다 소문자로 — 글자 수를 그대로 둔다 (소문자가 두 글자가 되는 드문 글자는 그대로)
fn lower(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        })
        .collect()
}
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

// 날짜 · 시각처럼 보이는 말 — 정확히 읽는 일은 메모킹(findDates)이 한다. 여기서는 "알림 맞춰 저장" 줄을 보여 줄지만
//   shortcut: 메모킹이 못 읽는 말에도 줄이 뜰 수 있다 (그때 메모킹은 알림 없이 저장), 틀리게 뜨는 일이 잦으면 메모킹에 묻는 길로
static DATE_HINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)오늘|내일|모레|글피|이번\s*주|다음\s*주|다음\s*달|[월화수목금토일]요일|\d{1,2}\s*월\s*\d{1,2}\s*일|\d{1,2}\s*일\s*(?:에|까지)",
        r"|\d{1,2}/\d{1,2}|\d{1,2}:\d{2}|(?:\d{1,2}|열한|열두|한|두|세|네|다섯|여섯|일곱|여덟|아홉|열)\s*시(?![간작])",
        r"|오전|오후|저녁|새벽|이틀|사흘|나흘|\d+\s*(?:일|주|달|개월)\s*(?:뒤|후)|매일|매주|매달|매년",
        r"|\btomorrow\b|\btonight\b|\bnext\s+(?:week|month|mon|tue|wed|thu|fri|sat|sun)|\d{1,2}\s*(?:am|pm)\b"
    ))
    .unwrap()
});

/// 첫 줄에 날짜 · 시각 같은 말이 있는가 ("내일 3시 치과" → 알림 맞춰 저장 줄)
pub fn mentions_date(text: &str) -> bool {
    DATE_HINT.is_match(text.lines().next().unwrap_or("")).unwrap_or(false)
}

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
    let title: String = match first {
        Some(t) if !(locked && t.starts_with('🔒')) => t.chars().take(80).collect(),
        _ if locked => LOCKED_TITLE.into(),
        _ => UNTITLED.into(),
    };
    let body = if locked {
        String::new()
    } else {
        let mut b: String = lines.collect::<Vec<_>>().join("\n");
        if b.len() > MAX_BODY {
            let cut = (0..=MAX_BODY).rev().find(|&i| b.is_char_boundary(i)).unwrap_or(0);
            b.truncate(cut);
        }
        b
    };
    let (title_lower, lower) = (lower(&title), lower(&body));
    Some(Memo { id: id.into(), title, body, title_lower, lower, locked, updated: front_value(front, "updated").unwrap_or("").into(), file })
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

/// 찾은 말 둘레 한 줄 (앞뒤 몇 글자) — 소문자 본문에서 찾은 자리 → 원래 본문의 같은 글자 자리
fn snippet_at(m: &Memo, at: usize) -> String {
    let nth = m.lower[..at].chars().count();
    snippet(&m.body, m.body.char_indices().nth(nth).map_or(m.body.len(), |(b, _)| b))
}

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
    let q = lower(query.trim());
    if q.is_empty() || limit == 0 {
        return (Vec::new(), 0);
    }
    let terms: Vec<&str> = q.split_whitespace().collect();
    let mut hits: Vec<(f32, usize, &Memo, Option<usize>)> = Vec::new();
    'next: for (i, m) in memos.iter().enumerate() {
        let title = &m.title_lower;
        let mut score = 0.0;
        let mut first_body = None;
        for t in &terms {
            score += if let Some(at) = title.find(t) {
                if at == 0 { 4.0 } else { 3.0 }
            } else if is_choseong_query(t) && choseong_index(&m.title, t).is_some() {
                2.5
            } else if let Some(at) = m.lower.find(t) {
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
        .map(|(_, _, m, at)| MemoHit { id: m.id.clone(), title: m.title.clone(), snippet: at.map(|a| snippet_at(m, a)).unwrap_or_default(), locked: m.locked })
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
        let n = memo("e", "", "회의\nKT 담당자 John 에게 전화");
        assert_eq!(search(&[n], "john", 1).0[0].snippet, "KT 담당자 John 에게 전화", "찾기는 소문자로, 보여 주기는 원래 글로");
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
        let pasted = memo("p", "", "# 암호 예시\n<!-- memoking-lock v1 n=1 s=A i=B c=C -->\n이 줄은 보통 글");
        assert!(!pasted.locked, "암호문 주석을 붙여 넣은 보통 메모는 잠근 메모가 아니다 (메모킹과 같게)");
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
    fn follow_memoking_magic_settings() {
        let v = r#""seenVersion":"0.77.0""#;
        assert_eq!(magic_key(&format!(r#"{{{v},"shortcuts":{{"magic":"Control+Shift+Space"}}}}"#)).as_deref(), Some("ctrl+shift+Space"));
        assert_eq!(magic_key(&format!(r#"{{{v},"shortcuts":{{"magic":"CommandOrControl+Alt+`"}}}}"#)).as_deref(), Some("ctrl+alt+`"));
        assert_eq!(magic_key(&format!(r#"{{{v},"shortcuts":{{}}}}"#)), None);
        assert_eq!(magic_key(&format!(r#"{{{v},"shortcuts":{{"magic":"K"}}}}"#)), None, "조합키 없는 키는 쓰지 않는다");
        assert_eq!(magic_key(r#"{"seenVersion":"0.76.1","shortcuts":{"magic":"Control+Shift+Space"}}"#), None, "옛 메모킹은 그 키를 계속 잡고 있다");
        let s = format!(r#"{{{v},"magic":{{"enabled":true,"modules":{{"memo":true,"files":false,"opms":false}},"paused":{{"web":true,"calc":false}}}}}}"#);
        assert_eq!(magic_off(&s), Some(vec!["files".to_string(), "web".to_string()]), "MagicKey 에 없는 기능(opms)은 빼고");
        assert_eq!(magic_off(&format!(r#"{{{v},"magic":{{"enabled":false,"modules":{{"files":false}}}}}}"#)), None, "매직키를 통째로 껐으면 따르지 않는다");
        assert_eq!(magic_off("{}"), None);
    }

    #[test]
    fn date_words() {
        for t in ["내일 오후 3시 치과", "다음 주 화요일 회의", "11월 7일 보고", "15:30 전화", "세 시 반에 보자", "3일 뒤 마감", "meet tomorrow"] {
            assert!(mentions_date(t), "{t}");
        }
        for t in ["장보기 목록", "세 시간 걸림", "3시간 회의 정리", "시작 버튼", "그냥 메모\n내일 3시"] {
            assert!(!mentions_date(t), "{t}");
        }
    }

    #[test]
    fn settings_and_shelf() {
        assert_eq!(vault_from_settings(r#"{"vaultPath":"D:\\메모"}"#), Some(PathBuf::from("D:\\메모")));
        assert_eq!(vault_from_settings(r#"{"vaultPath":""}"#), None);
        assert_eq!(vault_from_settings("깨진 글"), None);
        assert!(knows_magickey(r#"{"seenVersion":"0.77.0"}"#) && knows_magickey(r#"{"seenVersion":"1.0.0"}"#));
        assert!(!knows_magickey(r#"{"seenVersion":"0.76.1"}"#) && !knows_magickey("{}"), "옛 메모킹은 인자를 몰라 새 메모 줄을 숨긴다");

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
