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

/// 색인 한 줄 — 이름은 경로 끝을 가리키기만 하고, 소문자는 이름과 다를 때만 따로 들고 있다 (40만 개라 메모리를 아낀다)
#[derive(Debug, Clone)]
pub struct Entry {
    path: Box<str>,
    name_at: u32,
    lower: Option<Box<str>>,
    pub is_dir: bool,
    pub app: bool,
    pub depth: u16,
}

impl Entry {
    // shortcut: 경로는 String 으로 (Windows 의 짝 없는 UTF-16 이름은 깨져 열리지 않음 — 그런 이름은 거의 없다)
    pub fn new(path: PathBuf, is_dir: bool, app: bool, depth: u16) -> Self {
        let path = path.to_string_lossy().into_owned();
        let name_len = Path::new(&path).file_name().map_or(0, |n| n.len());
        let name_at = (path.len() - name_len) as u32;
        let lower = path[name_at as usize..].to_lowercase();
        let lower = (lower != path[name_at as usize..]).then(|| lower.into_boxed_str());
        Entry { path: path.into_boxed_str(), name_at, lower, is_dir, app, depth }
    }
    pub fn name(&self) -> &str {
        &self.path[self.name_at as usize..]
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    fn lower(&self) -> &str {
        self.lower.as_deref().unwrap_or(self.name())
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
        writeln!(w, "{}{}{}\t{}", u8::from(e.is_dir), u8::from(e.app), e.depth, e.path)?;
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
    let n = e.name();
    match e.app.then(|| [".lnk", ".url"].into_iter().find(|x| e.lower().ends_with(x))).flatten() {
        Some(ext) => n[..n.len() - ext.len()].to_string(),
        None => n.to_string(),
    }
}

/// 글자들이 순서대로 들어 있나 (fzf 처럼 — 철자가 조금 빠져도)
fn subsequence(name: &str, term: &str) -> bool {
    let mut it = name.chars();
    term.chars().all(|c| it.any(|n| n == c))
}

/// 점수가 높은 n 개만 차례대로 — 맞은 것 전부를 줄 세우지 않는다 (짧은 글은 수십만 개가 맞는다)
fn top(mut v: Vec<(f32, &Entry)>, n: usize) -> Vec<Hit> {
    let by_score = |a: &(f32, &Entry), b: &(f32, &Entry)| b.0.total_cmp(&a.0);
    let keep = (n * 2).min(v.len()); // 같은 경로가 두 번 들어 있어도 n 개를 채우게 넉넉히
    if keep > 0 && keep < v.len() {
        v.select_nth_unstable_by(keep - 1, by_score);
        v.truncate(keep);
    }
    v.sort_by(by_score);
    let mut seen = HashSet::new();
    v.into_iter()
        .filter(|(_, e)| seen.insert(e.path().to_lowercase()))
        .take(n)
        .map(|(_, e)| Hit { name: shown_name(e), path: e.path().to_string(), is_dir: e.is_dir, app: e.app })
        .collect()
}

/// 이름으로 찾기 — 앱 · 폴더 · 파일을 따로 몇 개씩 (프로그램이나 폴더가 파일들에 묻히지 않게)
///   점수: 낱말이 통째로 든 이름 > 글자만 순서대로 든 이름, 이름이 찾는 말로 시작 · 프로그램 · 얕은 곳 · 짧은 이름이 앞
pub fn search(entries: &[Entry], query: &str, limits: (usize, usize, usize)) -> Found {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Found::default();
    }
    let terms: Vec<(String, Option<String>, bool)> = q.split_whitespace().map(|t| (t.to_string(), compact_term(t), is_choseong_query(t))).collect();
    let mut kinds: [Vec<(f32, &Entry)>; 3] = Default::default(); // 앱 · 폴더 · 파일
    let mut buf = String::new(); // 기호를 뺀 이름 — 이름마다 새로 만들지 않고 한 칸을 다시 쓴다
    'next: for e in entries {
        let kind = if e.app { 0 } else if e.is_dir { 1 } else { 2 };
        if [limits.0, limits.1, limits.2][kind] == 0 {
            continue; // 꺼 둔 종류는 세지도 않는다
        }
        let lower = e.lower();
        let mut score = 0.0;
        for (t, compact, cho) in &terms {
            score += if let Some(at) = lower.find(t.as_str()) {
                if at == 0 { 3.0 } else { 2.0 }
            } else if compact.as_ref().is_some_and(|c| {
                if !subsequence(lower, c) {
                    return false; // 글자가 순서대로라도 없으면 기호를 빼도 없다 (빠른 거르기)
                }
                buf.clear();
                buf.extend(lower.chars().filter(|ch| ch.is_alphanumeric()));
                buf.contains(c.as_str())
            }) {
                2.0
            } else if let Some(at) = cho.then(|| choseong_index(e.name(), t)).flatten() {
                if at == 0 { 2.5 } else { 1.5 }
            } else if subsequence(lower, t) {
                0.5
            } else {
                continue 'next;
            };
        }
        let bonus = if e.app { 1.5 } else { 0.0 } + if lower == q { 2.0 } else { 0.0 } - (e.depth as f32 * 0.08).min(1.0) - (lower.chars().count() as f32 * 0.01).min(0.5);
        kinds[kind].push((score + bonus, e));
    }
    let total: usize = kinds.iter().map(Vec::len).sum();
    let [apps, folders, files] = kinds;
    let out = Found { apps: top(apps, limits.0), folders: top(folders, limits.1), files: top(files, limits.2), more: 0 };
    Found { more: total - out.apps.len() - out.folders.len() - out.files.len(), ..out }
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
    fn best_first_among_many() {
        let mut v: Vec<Entry> = (0..5000).map(|i| e(&format!("/d/deep/deeper/x{i}_report.txt"), false, false, 5)).collect();
        v.push(e("/me/report.txt", false, false, 0));
        v.push(e("/me/Report", true, false, 0));
        let f = search(&v, "report", (0, 3, 6));
        assert_eq!(f.files[0].name, "report.txt", "이름이 찾는 말로 시작하는 얕은 것이 맨 앞");
        assert_eq!(f.folders[0].name, "Report", "대문자 이름도 찾고 이름은 그대로");
        assert_eq!((f.files.len(), f.more), (6, 5001 - 6));
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
        let names: Vec<_> = entries.iter().map(Entry::name).collect();
        assert_eq!(names, ["보고서", "2026", "회의록.md"], "얕은 곳부터, 뺄 것은 빼고");
        let file = dir.join("index.tsv");
        save(&entries, &file).unwrap();
        let back = load(&file).unwrap();
        assert_eq!(back.len(), 3);
        assert!(back[0].is_dir && back[2].depth == 2);
        fs::remove_dir_all(dir).unwrap();
    }
}
