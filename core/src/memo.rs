//! 메모킹 메모 — 메모 폴더의 .md 파일을 읽기만 해서 찾는다 (쓰기 · 열기는 메모킹이 한다).
//!   파일 모양: `---` 머리말(id · updated …) `---` + 마크다운 본문. 첫 줄이 제목.
//!   잠근 메모는 제목만 (암호문은 찾지 않음).

use crate::text::{choseong_index, compact_term, is_choseong_query};
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
    /// 본문에는 안 보이지만 찾을 글 (링크 주소 · 메일 주소, 줄마다 하나, 소문자)
    hidden: String,
    /// 기호 · 띄어쓰기를 뺀 소문자 본문 (010-1234-5678 ↔ 01012345678)
    compact: String,
    pub locked: bool,
    pub updated: String,
    pub file: PathBuf,
}

impl Memo {
    /// 본문 (읽는 글자만, 잠근 메모는 빈 글) — "내용 복사"
    pub fn body(&self) -> &str {
        &self.body
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoHit {
    pub id: String,
    pub title: String,
    /// 찾은 말 둘레의 본문 한 줄 (없으면 빈 글)
    pub snippet: String,
    pub locked: bool,
    /// 마지막으로 고친 때 (머리말 updated, 모두 찾기 창의 '고친 때')
    pub updated: String,
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

/// 메모킹 버전이 MagicKey 와 함께 쓰기를 아는가 — appVersion(0.77.1~, 켤 때마다 적음)과 seenVersion(목록 창을 열 때) 중 큰 것
///   (seenVersion 만 보면 알림 영역에서 조용히 켜진 메모킹을 옛 버전으로 잘못 본다)
pub fn knows_magickey(json: &str) -> bool {
    memoking_version(json) >= MAGICKEY_SINCE
}

/// 메모킹이 `--new-schedule-file=` (일정 탭 빠른 추가와 같은 일정 만들기)를 알아듣는 첫 버전
pub const SCHEDULE_SINCE: (u32, u32, u32) = (1, 4, 0);

/// 메모킹이 매직키의 "일정 추가" 를 알아듣는가 (옛 메모킹은 "알림 맞춰 저장" — 메모 + 알림 — 으로 대신)
pub fn knows_schedule(json: &str) -> bool {
    memoking_version(json) >= SCHEDULE_SINCE
}

/// 설정에 적힌 메모킹 버전 — appVersion(0.77.1~, 켤 때마다 적음)과 seenVersion(목록 창을 열 때) 중 큰 것
fn memoking_version(json: &str) -> (u32, u32, u32) {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let ver = |key: &str| {
        let s = v.get(key).and_then(|x| x.as_str()).unwrap_or("");
        let mut n = s.trim_start_matches('v').split('.').map(|p| p.parse::<u32>().unwrap_or(0));
        (n.next().unwrap_or(0), n.next().unwrap_or(0), n.next().unwrap_or(0))
    };
    ver("appVersion").max(ver("seenVersion"))
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

/// 찾기용 글자 — 글자 수를 그대로 둔다 (찾은 자리를 원래 글에 그대로 옮기게):
///   소문자로 (두 글자가 되는 드문 글자는 그대로) · 전각 영숫자 · 기호는 반각으로 (ｈｏｎｇ＠… → hong@…)
fn lower(s: &str) -> String {
    s.chars()
        .map(|c| {
            let c = match c {
                '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
                _ => c,
            };
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        })
        .collect()
}

/// 보이지 않는 글자는 빼고 (메일 · 웹에서 붙여 넣은 글에 숨어 있어 이메일 · 번호가 안 맞던 것), 특수 띄어쓰기는 보통 띄어쓰기로
fn clean(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}' | '\u{00AD}'))
        .map(|c| if matches!(c, '\u{00A0}' | '\u{2007}' | '\u{202F}' | '\u{3000}') { ' ' } else { c })
        .collect()
}

/// 기호 · 띄어쓰기를 뺀 글 (이미 소문자인 글에서)
fn compact_of(lower: &str) -> String {
    lower.chars().filter(|c| c.is_alphanumeric()).collect()
}

static BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s{0,3}#{1,6}\s+|^\s*>\s?|^\s*(?:[-*+]|\d+[.)])(?:\s+|$)(?:\\?\[[ xX]\](?:\s+|$))?").unwrap());
// 표 구분 줄 |---|:--:|
static TABLE_SEP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\|?\s*:?-+:?\s*(?:\|\s*:?-+:?\s*)*\|?$").unwrap());
// ![대체 글](주소) · [글](주소 "제목") — 주소는 <…> 로 감쌀 수도 있다
static IMAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"!\[([^\]]*)\]\(\s*<?([^)\s>]*)>?(?:\s+["'][^"']*["'])?\s*\)"#).unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\[([^\]]*)\]\(\s*<?([^)\s>]*)>?(?:\s+["'][^"']*["'])?\s*\)"#).unwrap());
// 자동 링크 <https://…> · <hong@example.com> · <mailto:…> — 글자를 남긴다 (예전엔 HTML 태그로 보고 지워 이메일로 못 찾았다)
static AUTOLINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<(?:mailto:)?((?:https?|ftp)://[^<>\s]+|[^<>\s@]+@[^<>\s]+)>").unwrap());
// HTML 태그 (흔히 쓰는 이름만) · 주석만 지운다 — <회의록> · <Enter> · a < b > c 같은 글은 그대로
static TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)<!--.*?-->|</?(?:a|abbr|b|big|blockquote|br|center|code|col|colgroup|dd|del|details|div|dl|dt|em|font|h[1-6]|hr|i|img|ins|kbd|li|mark|ol|p|pre|q|s|small|span|strike|strong|sub|summary|sup|table|tbody|td|tfoot|th|thead|tr|tt|u|ul)",
        r"(?:\s[^<>]*)?/?>"
    ))
    .unwrap()
});
static EMPHASIS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\*\*|__|~~|`)(.+?)\1").unwrap());
static ESCAPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\([!-/:-@\[-`{-~])").unwrap());
static ENTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&(nbsp|amp|lt|gt|quot|apos|#39);").unwrap());

/// 링크 주소 → 찾을 글 (mailto: 는 떼고, 링크 글에 이미 있으면 뺀다)
fn keep_url(url: &str, text: &str, hidden: &mut Vec<String>) {
    let url = url.strip_prefix("mailto:").unwrap_or(url);
    let u = lower(url);
    if !u.is_empty() && !u.starts_with('#') && !lower(text).contains(&u) && !hidden.contains(&u) {
        hidden.push(u);
    }
}

/// 한 줄의 마크다운 · HTML 기호를 걷어낸다 (링크 주소는 hidden 으로)
fn inline(s: &str, hidden: &mut Vec<String>) -> String {
    let s = IMAGE.replace_all(s, |c: &fancy_regex::Captures| {
        keep_url(&c[2], &c[1], hidden);
        format!(" {} ", &c[1])
    });
    let s = LINK.replace_all(&s, |c: &fancy_regex::Captures| {
        keep_url(&c[2], &c[1], hidden);
        c[1].to_string()
    });
    let s = AUTOLINK.replace_all(&s, "$1");
    let s = TAG.replace_all(&s, "");
    let s = EMPHASIS.replace_all(&s, "$2");
    let s = ESCAPE.replace_all(&s, "$1");
    ENTITY
        .replace_all(&s, |c: &fancy_regex::Captures| {
            match &c[1] {
                "nbsp" => " ",
                "amp" => "&",
                "lt" => "<",
                "gt" => ">",
                "quot" => "\"",
                _ => "'",
            }
            .to_string()
        })
        .split(' ')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ") // 걷어낸 자리에 남은 겹 띄어쓰기는 하나로
}

/// 머리말의 `키: 값` 하나 (따옴표는 뗀다)
fn front_value<'a>(front: &'a str, key: &str) -> Option<&'a str> {
    front.lines().find_map(|l| {
        let v = l.strip_prefix(key)?.strip_prefix(':')?.trim();
        let v = v.trim_matches(|c| c == '"' || c == '\'');
        (!v.is_empty()).then_some(v)
    })
}

/// 본문 → 읽는 글자 줄들 (코드 울타리 · 구분선 · 한 줄 주석은 빼고, 마크다운 기호는 걷어낸다) · 링크 주소들
fn plain(body: &str) -> (Vec<String>, Vec<String>) {
    let mut fence = false;
    let (mut lines, mut hidden) = (Vec::new(), Vec::new());
    for raw in body.lines() {
        let raw = clean(raw);
        let t = raw.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            if !t.is_empty() {
                lines.push(t.to_string());
            }
            continue;
        }
        if (t.starts_with("<!--") && t.ends_with("-->")) || (t.len() >= 3 && t.chars().all(|c| c == '-' || c == '*' || c == '_' || c == ' ')) || (t.contains('|') && TABLE_SEP.is_match(t).unwrap_or(false)) {
            continue;
        }
        let mut s = BLOCK.replace(&raw, "").into_owned();
        // 표 한 줄: | 이름 | 메일 | → 이름 · 메일
        if s.trim_start().starts_with('|') {
            let cells: Vec<&str> = s.trim().trim_matches('|').split('|').map(str::trim).filter(|c| !c.is_empty()).collect();
            s = cells.join(" · ");
        }
        let s = inline(&s, &mut hidden);
        let s = s.trim();
        if !s.is_empty() {
            lines.push(s.to_string());
        }
    }
    (lines, hidden)
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
    let (lines, hidden) = plain(body);
    let mut lines = lines.into_iter();
    let title: String = match lines.next() {
        Some(t) if !(locked && t.starts_with('🔒')) => t.chars().take(80).collect(),
        _ if locked => LOCKED_TITLE.into(),
        _ => UNTITLED.into(),
    };
    let (body, hidden) = if locked {
        (String::new(), String::new())
    } else {
        let mut b: String = lines.collect::<Vec<_>>().join("\n");
        if b.len() > MAX_BODY {
            let cut = (0..=MAX_BODY).rev().find(|&i| b.is_char_boundary(i)).unwrap_or(0);
            b.truncate(cut);
        }
        (b, hidden.join("\n"))
    };
    let (title_lower, lower) = (lower(&title), lower(&body));
    let compact = compact_of(&lower);
    Some(Memo { id: id.into(), title, body, title_lower, lower, hidden, compact, locked, updated: front_value(front, "updated").unwrap_or("").into(), file })
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

/// 본문 어디서 찾았나 (보여 줄 한 줄을 고르려고)
#[derive(Clone, Copy)]
enum At {
    /// 소문자 본문의 자리
    Body(usize),
    /// 기호를 뺀 본문의 자리
    Compact(usize),
    /// 링크 주소의 자리
    Hidden(usize),
}

/// 찾은 말 둘레 한 줄 (앞뒤 몇 글자) — 소문자 본문에서 찾은 자리 → 원래 본문의 같은 글자 자리
fn snippet_at(m: &Memo, at: At) -> String {
    let nth = match at {
        At::Body(at) => m.lower[..at].chars().count(),
        // 기호를 뺀 글의 n 번째 글자 → 본문에서 n 번째 영숫자
        At::Compact(at) => {
            let n = m.compact[..at].chars().count();
            m.lower.chars().enumerate().filter(|(_, c)| c.is_alphanumeric()).nth(n).map_or(0, |(i, _)| i)
        }
        At::Hidden(at) => return snippet(&m.hidden, at),
    };
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

/// 낱말 하나가 메모에 있나 → (점수, 본문에서 찾은 자리)
///   제목 (맨 앞 4 · 가운데 3) > 제목 초성 2.5 > 제목 기호 빼고 2 > 본문 1 > 본문 기호 빼고 0.9 > 링크 주소 0.8
fn match_term(m: &Memo, t: &str) -> Option<(f32, Option<At>)> {
    if let Some(at) = m.title_lower.find(t) {
        return Some((if at == 0 { 4.0 } else { 3.0 }, None));
    }
    if is_choseong_query(t) && choseong_index(&m.title, t).is_some() {
        return Some((2.5, None));
    }
    let c = compact_term(t);
    if c.as_ref().is_some_and(|c| compact_of(&m.title_lower).contains(c.as_str())) {
        return Some((2.0, None));
    }
    if let Some(at) = m.lower.find(t) {
        return Some((1.0, Some(At::Body(at))));
    }
    if let Some(at) = c.and_then(|c| m.compact.find(&c)) {
        return Some((0.9, Some(At::Compact(at))));
    }
    m.hidden.find(t).map(|at| (0.8, Some(At::Hidden(at))))
}

/// 붙여 넣은 낱말에 딸려 온 껍데기 (<…> · "…" · (…) · 끝의 쉼표 · mailto:) — 그대로 못 찾으면 떼고 다시 찾는다
fn unwrap_term(t: &str) -> &str {
    let t = t.trim_matches(|c: char| "<>()[]{}\"'“”‘’`,;:".contains(c)).trim_end_matches(['.', '!', '?']);
    t.strip_prefix("mailto:").unwrap_or(t)
}

/// 제목 · 본문에서 찾기 — 낱말마다 제목에 있으면 크게, 본문에만 있으면 작게. 같으면 최근 것이 앞
pub fn search(memos: &[Memo], query: &str, limit: usize) -> (Vec<MemoHit>, usize) {
    let q = lower(clean(query).trim());
    if q.is_empty() || limit == 0 {
        return (Vec::new(), 0);
    }
    let terms: Vec<(&str, &str)> = q.split_whitespace().map(|t| (t, unwrap_term(t))).filter(|(_, u)| !u.is_empty()).collect();
    if terms.is_empty() {
        return (Vec::new(), 0);
    }
    let mut hits: Vec<(f32, usize, &Memo, Option<At>)> = Vec::new();
    'next: for (i, m) in memos.iter().enumerate() {
        let mut score = 0.0;
        let mut first = None;
        for &(t, u) in &terms {
            let Some((s, at)) = match_term(m, t).or_else(|| (u != t).then(|| match_term(m, u)).flatten()) else { continue 'next };
            score += s;
            if first.is_none() {
                first = at;
            }
        }
        hits.push((score, i, m, first));
    }
    let total = hits.len();
    hits.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let out: Vec<MemoHit> = hits
        .into_iter()
        .take(limit)
        .map(|(_, _, m, at)| MemoHit { id: m.id.clone(), title: m.title.clone(), snippet: at.map(|a| snippet_at(m, a)).unwrap_or_default(), locked: m.locked, updated: m.updated.clone() })
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
    fn email_autolink_is_searchable() {
        let m = memo("z", "", "[회의 공지]\n1팀 : <kim@example.com>\n2팀 : <mailto:hong@example.com> · [메일](mailto:x@y.kr)\n<b>굵게</b>");
        assert_eq!(m.body, "1팀 : kim@example.com\n2팀 : hong@example.com · 메일\n굵게");
        for q in ["ong@example.com", "hon", "example", "kim@example.com"] {
            assert_eq!(search(std::slice::from_ref(&m), q, 5).0.len(), 1, "{q}");
        }
    }

    #[test]
    fn similar_hidden_text_cases() {
        let body = concat!(
            "연락처\n",
            "<회의록> 은 그대로 · a < b > c · Ctrl+<Enter>\n",
            "| 이름 | 메일 |\n|---|:--:|\n| 홍길동 | ho\u{200B}ng@example.com |\n",
            "전화 010-1234-5678 · ｈｏｎｇ２＠ｅｘａｍｐｌｅ．ｃｏｍ\n",
            "[줌 안내](https://zoom.us/j/9876) ![로고](https://x.kr/logo.png)\n",
            "AT&amp;T \\*별\\* <span style=\"x\">글</span><!-- 숨김 -->\n",
        );
        let m = memo("s", "", body);
        assert_eq!(
            m.body,
            "<회의록> 은 그대로 · a < b > c · Ctrl+<Enter>\n이름 · 메일\n홍길동 · hong@example.com\n전화 010-1234-5678 · ｈｏｎｇ２＠ｅｘａｍｐｌｅ．ｃｏｍ\n줌 안내 로고\nAT&T *별* 글"
        );
        let find = |q: &str| search(std::slice::from_ref(&m), q, 5).0.into_iter().next().map(|h| h.snippet);
        // 숨은 글자 · 전각 · 표 · 꺾쇠 글
        assert_eq!(find("ong@example").as_deref(), Some("홍길동 · hong@example.com"));
        assert!(find("hong2@example.com").is_some());
        assert!(find("<회의록>").is_some());
        // 기호를 뺀 번호 ↔ 기호 있는 번호
        assert_eq!(find("01012345678").as_deref(), Some("전화 010-1234-5678 · ｈｏｎｇ２＠ｅｘａｍｐｌｅ．ｃｏｍ"));
        assert!(find("010 1234 5678").is_some());
        // 링크 주소 · 이미지 대체 글 · HTML 글자
        assert_eq!(find("zoom.us/j/9876").as_deref(), Some("https://zoom.us/j/9876"));
        assert!(find("로고").is_some() && find("at&t").is_some() && find("*별*").is_some());
        // 붙여 넣은 껍데기: <…> · mailto: · 끝의 쉼표
        for q in ["<hong@example.com>", "mailto:hong@example.com", "hong@example.com,", "(hong@example.com)"] {
            assert!(find(q).is_some(), "{q}");
        }
        assert!(find("없는말").is_none() && find("<>").is_none());
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
    fn settings_and_shelf() {
        assert_eq!(vault_from_settings(r#"{"vaultPath":"D:\\메모"}"#), Some(PathBuf::from("D:\\메모")));
        assert_eq!(vault_from_settings(r#"{"vaultPath":""}"#), None);
        assert_eq!(vault_from_settings("깨진 글"), None);
        assert!(knows_magickey(r#"{"seenVersion":"0.77.0"}"#) && knows_magickey(r#"{"seenVersion":"1.0.0"}"#));
        assert!(!knows_magickey(r#"{"seenVersion":"0.76.1"}"#) && !knows_magickey("{}"), "옛 메모킹은 인자를 몰라 새 메모 줄을 숨긴다");
        assert!(knows_magickey(r#"{"seenVersion":"0.76.1","appVersion":"0.77.1"}"#), "목록 창을 열지 않아 seenVersion 이 옛것이어도");
        assert!(knows_schedule(r#"{"appVersion":"1.4.0"}"#) && knows_schedule(r#"{"appVersion":"1.10.0"}"#));
        assert!(!knows_schedule(r#"{"appVersion":"1.3.0"}"#) && !knows_schedule("{}"), "옛 메모킹은 일정 글을 몰라 알림 맞춰 저장으로 대신한다");

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
