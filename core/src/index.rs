//! 앱 · 파일 · 폴더 이름 색인 — 따로 설치할 것 없이 이름 목록을 직접 만들어 두고(이 PC 안에서만, 내용은 읽지 않음) 칠 때마다 찾는다.
//!   찾는 곳: 시작 메뉴 프로그램(바로 가기) → 사용자 폴더 → 다른 드라이브. 시스템 · 프로그램 폴더, 숨김(. $ ~), node_modules 등은 뺀다.
//!   얕은 곳부터(넓이 우선) 훑고, 목록은 파일 하나(cache)에 저장해 다음에 켤 때 바로 쓴다.

use crate::text::{choseong_index, compact_term, is_choseong_query};
use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

pub const MAX_ENTRIES: usize = 400_000;

const SKIP: &[&str] = &[
    "node_modules", "__pycache__", "appdata", "application data", "local settings", "temp", "tmp", "cache", "caches",
    "windows", "program files", "program files (x86)", "programdata", "users", "recovery", "perflogs", "msocache",
    "system volume information", "config.msi", "onedrivetemp", "site-packages", "venv", ".venv",
];

/// 이 이름의 폴더는 들어가지 않는다 (숨김 . $ ~ 로 시작하는 것도)
pub fn skip_dir(name: &str) -> bool {
    name.starts_with(['.', '$', '~']) || SKIP.contains(&name.to_lowercase().as_str())
}

fn skip_file(name: &str) -> bool {
    let l = name.to_lowercase();
    name.starts_with(['.', '~']) || l == "desktop.ini" || l == "thumbs.db" || l.starts_with("ntuser.")
}

pub struct Root {
    pub dir: PathBuf,
    /// 프로그램 바로 가기 폴더 (시작 메뉴)
    pub app: bool,
}

/// 찾을 곳 (앞의 것부터 — 목록이 차면 뒤의 것은 덜 들어간다)
pub fn default_roots() -> Vec<Root> {
    let mut roots = Vec::new();
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        for base in [env("APPDATA"), env("ProgramData")].into_iter().flatten() {
            roots.push(Root { dir: base.join(r"Microsoft\Windows\Start Menu\Programs"), app: true });
        }
    }
    if let Some(home) = env(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        roots.push(Root { dir: home, app: false });
    }
    if cfg!(windows) {
        for letter in 'C'..='Z' {
            roots.push(Root { dir: PathBuf::from(format!("{letter}:\\")), app: false });
        }
    }
    roots.into_iter().filter(|r| r.dir.is_dir()).collect()
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    lower: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub app: bool,
    pub depth: u16,
}

impl Entry {
    pub fn new(path: PathBuf, is_dir: bool, app: bool, depth: u16) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Entry { lower: name.to_lowercase(), name, path, is_dir, app, depth }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub name: String,
    pub path: String,
    #[serde(rename = "isDir")]
    pub is_dir: bool,
    pub app: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct Found {
    pub apps: Vec<Hit>,
    pub folders: Vec<Hit>,
    pub files: Vec<Hit>,
    /// 보여 주지 못한 나머지 개수
    pub more: usize,
}

/// 넓이 우선으로 (얕은 곳부터) 훑는다 — 링크는 따라가지 않는다
pub fn crawl(roots: &[Root], max: usize) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        let mut queue = VecDeque::from([(root.dir.clone(), 0u16)]);
        while let Some((dir, depth)) = queue.pop_front() {
            if entries.len() >= max {
                return entries;
            }
            if !seen.insert(dir.to_string_lossy().to_lowercase()) {
                continue;
            }
            let Ok(list) = fs::read_dir(&dir) else { continue }; // 권한 없음 등
            for ent in list.flatten() {
                if entries.len() >= max {
                    break;
                }
                let Ok(ft) = ent.file_type() else { continue };
                if ft.is_symlink() || !(ft.is_dir() || ft.is_file()) {
                    continue;
                }
                let name = ent.file_name().to_string_lossy().into_owned();
                if (ft.is_dir() && skip_dir(&name)) || (ft.is_file() && skip_file(&name)) {
                    continue;
                }
                let path = ent.path();
                if ft.is_dir() {
                    queue.push_back((path.clone(), depth + 1));
                }
                entries.push(Entry::new(path, ft.is_dir(), root.app && ft.is_file(), depth));
            }
        }
    }
    entries
}

/// 저장: 한 줄에 "d|a|depth\t경로" (d = 폴더, a = 프로그램)
pub fn save(entries: &[Entry], file: &Path) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("tmp");
    let mut w = BufWriter::new(fs::File::create(&tmp)?);
    for e in entries {
        writeln!(w, "{}{}{}\t{}", u8::from(e.is_dir), u8::from(e.app), e.depth, e.path.display())?;
    }
    w.into_inner()?.sync_all()?;
    fs::rename(tmp, file) // 다 쓴 뒤 바꿔치기 — 쓰다 꺼져도 지난 목록이 남게
}

pub fn load(file: &Path) -> Option<Vec<Entry>> {
    let r = BufReader::new(fs::File::open(file).ok()?);
    let mut out = Vec::new();
    for line in r.lines() {
        let line = line.ok()?;
        let (flags, path) = line.split_once('\t')?;
        let (d, a, depth) = (flags.get(0..1)?, flags.get(1..2)?, flags.get(2..)?.parse().ok()?);
        out.push(Entry::new(PathBuf::from(path), d == "1", a == "1", depth));
    }
    Some(out)
}

/// 앱(.lnk · .url)은 확장자 없이 보인다
fn shown_name(e: &Entry) -> String {
    if e.app {
        for ext in [".lnk", ".url"] {
            if e.lower.ends_with(ext) {
                return e.name[..e.name.len() - ext.len()].to_string();
            }
        }
    }
    e.name.clone()
}

/// 글자들이 순서대로 들어 있나 (fzf 처럼 — 철자가 조금 빠져도)
fn subsequence(name: &str, term: &str) -> bool {
    let mut it = name.chars();
    term.chars().all(|c| it.any(|n| n == c))
}

/// 이름으로 찾기 — 앱 · 폴더 · 파일을 따로 몇 개씩 (프로그램이나 폴더가 파일들에 묻히지 않게)
///   점수: 낱말이 통째로 든 이름 > 글자만 순서대로 든 이름, 이름이 찾는 말로 시작 · 프로그램 · 얕은 곳 · 짧은 이름이 앞
pub fn search(entries: &[Entry], query: &str, limits: (usize, usize, usize)) -> Found {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Found::default();
    }
    let terms: Vec<(String, Option<String>, bool)> = q.split_whitespace().map(|t| (t.to_string(), compact_term(t), is_choseong_query(t))).collect();
    let mut scored: Vec<(f32, &Entry)> = Vec::new();
    let (mut apps, mut folders, mut files) = (0, 0, 0);
    'next: for e in entries {
        let max = if e.app { limits.0 } else if e.is_dir { limits.1 } else { limits.2 };
        if max == 0 {
            continue; // 꺼 둔 종류는 세지도 않는다
        }
        let mut score = 0.0;
        for (t, compact, cho) in &terms {
            score += if let Some(at) = e.lower.find(t.as_str()) {
                if at == 0 { 3.0 } else { 2.0 }
            } else if compact.as_ref().is_some_and(|c| crate::text::compact(&e.lower).contains(c.as_str())) {
                2.0
            } else if let Some(at) = cho.then(|| choseong_index(&e.name, t)).flatten() {
                if at == 0 { 2.5 } else { 1.5 }
            } else if subsequence(&e.lower, t) {
                0.5
            } else {
                continue 'next;
            };
        }
        if e.app {
            apps += 1;
        } else if e.is_dir {
            folders += 1;
        } else {
            files += 1;
        }
        let bonus = if e.app { 1.5 } else { 0.0 } + if e.lower == q { 2.0 } else { 0.0 } - (e.depth as f32 * 0.08).min(1.0) - (e.lower.chars().count() as f32 * 0.01).min(0.5);
        scored.push((score + bonus, e));
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out = Found::default();
    let mut seen = HashSet::new();
    for (_, e) in scored {
        if !seen.insert(e.path.to_string_lossy().to_lowercase()) {
            continue;
        }
        let (list, max) = if e.app { (&mut out.apps, limits.0) } else if e.is_dir { (&mut out.folders, limits.1) } else { (&mut out.files, limits.2) };
        if list.len() < max {
            list.push(Hit { name: shown_name(e), path: e.path.to_string_lossy().into_owned(), is_dir: e.is_dir, app: e.app });
        }
    }
    out.more = apps + folders + files - out.apps.len() - out.folders.len() - out.files.len();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(p: &str, is_dir: bool, app: bool, depth: u16) -> Entry {
        Entry::new(PathBuf::from(p), is_dir, app, depth)
    }

    fn sample() -> Vec<Entry> {
        vec![
            e("/me/Programs/카카오톡.lnk", false, true, 1),
            e("/me/Documents/카카오페이 증권 영수증.pdf", false, false, 2),
            e("/me/Documents/카카오 관련 폴더", true, false, 2),
            e("/pd/Programs/Chrome.lnk", false, true, 1),
            e("/me/Documents/견적서_2026.xlsx", false, false, 2),
            e("/me/Documents/P2026-1170-KR01", true, false, 3),
        ]
    }

    #[test]
    fn split_by_kind() {
        let f = search(&sample(), "카카오", (2, 2, 2));
        assert_eq!(f.apps.len(), 1);
        assert_eq!(f.apps[0].name, "카카오톡", ".lnk 는 떼고");
        assert_eq!(f.folders[0].name, "카카오 관련 폴더");
        assert_eq!(f.files[0].name, "카카오페이 증권 영수증.pdf");
        assert_eq!(f.more, 0);
    }

    #[test]
    fn choseong_compact_and_fuzzy() {
        let s = sample();
        assert_eq!(search(&s, "ㄱㅈㅅ", (3, 3, 6)).files[0].name, "견적서_2026.xlsx");
        assert_eq!(search(&s, "P20261170KR01", (3, 3, 6)).folders[0].name, "P2026-1170-KR01");
        assert_eq!(search(&s, "chrm", (3, 3, 6)).apps[0].name, "Chrome");
        assert!(search(&s, "없는이름", (3, 3, 6)).apps.is_empty());
        let only_apps = search(&s, "카카오", (3, 0, 0));
        assert_eq!((only_apps.apps.len(), only_apps.files.len(), only_apps.more), (1, 0, 0), "꺼 둔 종류는 나머지 개수에도 없음");
    }

    #[test]
    fn crawl_save_load() {
        let dir = std::env::temp_dir().join(format!("magickey-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("보고서/2026")).unwrap();
        fs::create_dir_all(dir.join("node_modules/x")).unwrap();
        fs::write(dir.join("보고서/2026/회의록.md"), "").unwrap();
        fs::write(dir.join("desktop.ini"), "").unwrap();
        fs::write(dir.join(".bashrc"), "").unwrap();
        let entries = crawl(&[Root { dir: dir.clone(), app: false }], 100);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["보고서", "2026", "회의록.md"], "얕은 곳부터, 뺄 것은 빼고");
        let file = dir.join("index.tsv");
        save(&entries, &file).unwrap();
        let back = load(&file).unwrap();
        assert_eq!(back.len(), 3);
        assert!(back[0].is_dir && back[2].depth == 2);
        fs::remove_dir_all(dir).unwrap();
    }
}
