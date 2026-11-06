// MagicKey — Ctrl+Alt+Space (바꿀 수 있음) 로 어디서든 여는 만능 입력창 (메모킹 메모 · 앱 · 파일 · 폴더 찾기, 계산 · 날짜 계산, 웹 검색, 웹 AI)
//   창은 늘 떠 있다가 보였다 숨었다 한다 (다른 곳을 누르면 숨음). 끝내기는 알림 영역 아이콘에서.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use magickey_core::index::{self, Entry, Found};
use magickey_core::memo::{self, Memo, MemoHit, Shelf};
use magickey_core::{calc, date, web, Answer};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::Wry;
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_opener::OpenerExt;

const STALE: Duration = Duration::from_secs(10 * 60); // 매직키를 열 때 이보다 오래된 목록이면 뒤에서 새로 만든다

struct Index {
    entries: RwLock<Arc<Vec<Entry>>>,
    built: Mutex<Option<SystemTime>>,
    building: AtomicBool,
    cache: PathBuf,
    /// 방금 보여 준 경로 — 창이 보낸 경로 중 이것만 열고 복사한다 (아무 경로나 열지 않게)
    shown: Mutex<HashSet<String>>,
}

impl Index {
    fn new(cache: PathBuf) -> Arc<Self> {
        let built = std::fs::metadata(&cache).and_then(|m| m.modified()).ok();
        let me = Arc::new(Index {
            entries: RwLock::default(),
            built: Mutex::new(built),
            building: AtomicBool::new(false),
            cache,
            shown: Mutex::default(),
        });
        let bg = me.clone();
        std::thread::spawn(move || {
            if let Some(list) = index::load(&bg.cache) {
                *bg.entries.write().unwrap() = Arc::new(list);
            }
            bg.touch();
        });
        me
    }

    /// 목록이 없거나 오래됐으면 뒤에서 새로 만든다 (만드는 동안에도 지난 목록으로 찾는다)
    fn touch(self: &Arc<Self>) {
        let fresh = self.built.lock().unwrap().is_some_and(|t| t.elapsed().unwrap_or(STALE) < STALE);
        if (fresh && !self.entries.read().unwrap().is_empty()) || self.building.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            let list = index::crawl(&index::default_roots(), index::MAX_ENTRIES);
            let _ = index::save(&list, &me.cache); // 저장 못 해도 지금은 쓸 수 있다
            *me.entries.write().unwrap() = Arc::new(list);
            *me.built.lock().unwrap() = Some(SystemTime::now());
            me.building.store(false, Ordering::SeqCst);
        });
    }
}

/// 메모킹 메모 — 메모킹이 이 PC 에 있을 때만 (메모 폴더는 읽기만, 열기는 메모킹에게)
struct Memos {
    root: Option<PathBuf>,
    settings: Option<PathBuf>,
    /// 메모킹이 새 메모 만들기(`--new-memo-file=`)를 아는가 — 메모킹이 업데이트될 수 있어 열 때마다 다시 본다
    can_make: AtomicBool,
    /// 메모킹 실행 파일 (없으면 메모 파일을 기본 앱으로 연다)
    exe: Option<PathBuf>,
    shelf: Mutex<Shelf>,
    list: RwLock<Arc<Vec<Memo>>>,
    busy: AtomicBool,
    /// 방금 보여 준 메모 id — 이것만 연다
    shown: Mutex<HashSet<String>>,
}

impl Memos {
    /// 메모킹 설정(%APPDATA%\MemoKing\settings.json)의 메모 폴더, 없으면 메모킹 기본값(문서\메모킹)
    fn find(app: &AppHandle) -> Arc<Self> {
        let p = app.path();
        let file = p.config_dir().ok().map(|d| d.join("MemoKing").join("settings.json"));
        let settings = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok());
        let root = settings
            .as_deref()
            .and_then(memo::vault_from_settings)
            .or_else(|| p.document_dir().ok().map(|d| d.join("메모킹")))
            .filter(|r| r.is_dir());
        let exe = p.local_data_dir().ok().map(|d| d.join("Programs").join("memoking").join("MemoKing.exe")).filter(|e| e.is_file());
        let me = Arc::new(Memos { root, settings: file, can_make: AtomicBool::new(false), exe, shelf: Mutex::default(), list: RwLock::default(), busy: AtomicBool::new(false), shown: Mutex::default() });
        me.touch();
        me
    }

    /// 뒤에서 메모 폴더를 다시 훑는다 (바뀐 파일만 새로 읽음, 그동안은 지난 목록으로 찾는다)
    fn touch(self: &Arc<Self>) {
        let Some(root) = self.root.clone() else { return };
        let json = self.settings.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
        self.can_make.store(self.exe.is_some() && memo::can_make_memo(&json), Ordering::SeqCst);
        if self.busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            let list = me.shelf.lock().unwrap().refresh(&root);
            *me.list.write().unwrap() = Arc::new(list);
            me.busy.store(false, Ordering::SeqCst);
        });
    }
}

#[derive(Serialize)]
struct Calc {
    kind: &'static str,
    #[serde(flatten)]
    answer: Answer,
}

#[derive(Serialize)]
struct QueryOut {
    calc: Option<Calc>,
    site: Option<String>,
    question: bool,
    memos: Vec<MemoHit>,
    /// '새 메모' 줄을 보여 줄까 (메모킹이 알아듣고, 기능을 켜 둠)
    #[serde(rename = "newMemo")]
    new_memo: bool,
    /// '알림 맞춰 저장' 줄도 (첫 줄에 날짜 같은 말, 날짜 계산 글이 아님)
    remind: bool,
    #[serde(rename = "memoMore")]
    memo_more: usize,
    #[serde(flatten)]
    found: Found,
    indexed: usize,
    building: bool,
}

/// 칠 때마다: 계산 · 날짜 계산, 사이트 주소, 묻는 글인가, 앱 · 폴더 · 파일 (mods = 켜 둔 기능)
///   async: 메인 스레드 밖에서 돈다 (이름 수십만 개를 훑는 동안 창이 굳지 않게)
#[tauri::command]
async fn query(text: String, mods: Vec<String>, idx: State<'_, Arc<Index>>, notes: State<'_, Arc<Memos>>) -> Result<QueryOut, String> {
    let on = |m: &str| mods.iter().any(|x| x == m);
    let text: String = text.chars().take(200).collect();
    let calc = on("calc").then(|| {
        date::date_calc(&text, chrono::Local::now().naive_local())
            .map(|answer| Calc { kind: "date", answer })
            .or_else(|| calc::calc(&text).map(|answer| Calc { kind: "calc", answer }))
    });
    let date_calc = calc.as_ref().is_some_and(|c| c.as_ref().is_some_and(|c| c.kind == "date"));
    let new_memo = on("memo") && notes.can_make.load(Ordering::SeqCst);
    let (memos, memo_more) = memo::search(&notes.list.read().unwrap().clone(), &text, if on("notes") { 4 } else { 0 });
    notes.shown.lock().unwrap().extend(memos.iter().map(|m| m.id.clone()));
    let entries = idx.entries.read().unwrap().clone();
    let limits = (if on("apps") { 4 } else { 0 }, if on("files") { 3 } else { 0 }, if on("files") { 6 } else { 0 });
    let found = index::search(&entries, &text, limits);
    // 더하기만 한다 (답이 순서가 바뀌어 와도 창에 보이는 줄은 늘 열 수 있게) — 창을 띄울 때 비운다
    idx.shown.lock().unwrap().extend(found.apps.iter().chain(&found.folders).chain(&found.files).map(|h| h.path.clone()));
    Ok(QueryOut {
        calc: calc.flatten(),
        site: on("web").then(|| web::site_url(&text)).flatten(),
        question: web::looks_like_question(&text),
        memos,
        new_memo,
        remind: new_memo && !date_calc && memo::mentions_date(&text),
        memo_more,
        found,
        indexed: entries.len(),
        building: idx.building.load(Ordering::SeqCst),
    })
}

/// 여는 단축키 — 지금 키, 기억할 파일, 못 잡았을 때 창에 띄울 안내
struct Hotkey {
    now: Mutex<Shortcut>,
    file: PathBuf,
    notice: Mutex<Option<String>>,
}

/// 사람이 읽는 이름: Ctrl+Alt+Space, Ctrl+Shift+K, Alt+1
fn key_label(s: &Shortcut) -> String {
    let mut out = String::new();
    for (m, name) in [(Modifiers::CONTROL, "Ctrl+"), (Modifiers::ALT, "Alt+"), (Modifiers::SHIFT, "Shift+"), (Modifiers::SUPER, "Win+")] {
        if s.mods.contains(m) {
            out.push_str(name);
        }
    }
    let key = s.key.to_string();
    out + key.strip_prefix("Key").or_else(|| key.strip_prefix("Digit")).unwrap_or(&key)
}

/// 창의 기능 캡슐과 같은 것 — 알림 영역 메뉴 '설정' 에서도 켜고 끈다 (id 는 magic.js MODULES 와 같게)
const MODULES: [(&str, &str); 7] = [("memo", "새 메모 (메모킹)"), ("notes", "메모 검색 (메모킹)"), ("apps", "앱 검색"), ("files", "파일 검색"), ("calc", "계산 · 날짜 계산"), ("web", "웹 검색"), ("ai", "웹 AI")];

/// 알림 영역 메뉴에서 글자 · 체크를 바꿔야 하는 줄
struct TrayMenu {
    open: MenuItem<Wry>,
    hotkey: MenuItem<Wry>,
    mods: Vec<CheckMenuItem<Wry>>,
}

/// 알림 영역 아이콘 · 메뉴에 지금 단축키를 쓴다 (못 잡았으면 그렇다고)
fn show_key(app: &AppHandle, s: &Shortcut, ok: bool) {
    let label = key_label(s);
    let menu = app.state::<TrayMenu>();
    let _ = menu.open.set_text(format!("열기\t{label}"));
    let _ = menu.hotkey.set_text(if ok { format!("단축키 바꾸기…\t{label}") } else { format!("단축키 바꾸기…\t{label} (못 잡음)") });
    if let Some(tray) = app.tray_by_id("main") {
        let tip = if ok { format!("MagicKey — {label}") } else { "MagicKey — 단축키를 못 잡았어요. 여기를 눌러 열기".into() };
        let _ = tray.set_tooltip(Some(tip));
    }
}

#[derive(Serialize)]
struct Init {
    web: &'static [web::Site],
    ai: &'static [web::Site],
    notice: Option<String>,
    hotkey: String,
    /// 메모킹 메모를 찾을 수 있는가 (아니면 '메모' 캡슐을 숨긴다)
    memo: bool,
}

#[tauri::command]
fn init(hk: State<Hotkey>, notes: State<Arc<Memos>>) -> Init {
    let hotkey = key_label(&hk.now.lock().unwrap());
    Init { web: web::WEB_ENGINES, ai: web::AI_SITES, notice: hk.notice.lock().unwrap().clone(), hotkey, memo: notes.root.is_some() }
}

/// 메모 열기 — 메모킹에게 `--open-memo=<id>` 로 (꺼져 있으면 켜면서 연다). 메모킹 실행 파일을 못 찾으면 .md 파일을 기본 앱으로
#[tauri::command]
fn open_memo(app: AppHandle, id: String, notes: State<Arc<Memos>>) -> Result<(), String> {
    if !notes.shown.lock().unwrap().contains(&id) {
        return Err("찾은 메모가 아니에요".into());
    }
    match &notes.exe {
        Some(exe) => std::process::Command::new(exe).arg(format!("--open-memo={id}")).spawn().map(|_| ()).map_err(|e| format!("메모킹을 열지 못했어요: {e}")),
        None => {
            let file = notes.list.read().unwrap().iter().find(|m| m.id == id).map(|m| m.file.clone()).ok_or("메모를 찾지 못했어요")?;
            app.opener().open_path(file.to_string_lossy(), None::<&str>).map_err(|e| format!("열지 못했어요: {e}"))
        }
    }
}

/// 단축키 바꾸기 (key = "ctrl+shift+KeyK" 꼴) — 새 키를 잡은 뒤에야 옛 키를 놓는다 (못 잡으면 그대로)
#[tauri::command]
fn set_hotkey(app: AppHandle, key: String, hk: State<Hotkey>) -> Result<String, String> {
    let new: Shortcut = key.parse().map_err(|_| "이 키는 단축키로 쓸 수 없어요".to_string())?;
    if !new.mods.intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER) {
        return Err("Ctrl · Alt · Win 중 하나와 함께 눌러 주세요".into());
    }
    let label = key_label(&new);
    let mut now = hk.now.lock().unwrap();
    let gs = app.global_shortcut();
    if new.id() != now.id() || !gs.is_registered(*now) {
        gs.register(new).map_err(|_| format!("{label} 도 다른 프로그램이 쓰고 있어요. 다른 조합을 눌러 보세요"))?;
        if new.id() != now.id() {
            let _ = gs.unregister(*now); // 못 잡고 있던 키면 놓을 것도 없다
        }
    }
    *now = new;
    *hk.notice.lock().unwrap() = None;
    if let Some(dir) = hk.file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&hk.file, new.to_string()); // 기억 못 해도 지금은 된다
    show_key(&app, &new, true);
    Ok(label)
}

/// 창에서 캡슐을 눌렀을 때 — 알림 영역 메뉴의 체크도 맞춘다
#[tauri::command]
fn sync_modules(off: Vec<String>, menu: State<TrayMenu>) {
    for item in &menu.mods {
        let id = item.id().as_ref().trim_start_matches("mod:").to_string();
        let _ = item.set_checked(!off.contains(&id));
    }
}

/// 새 메모 (remind: 첫 줄 날짜로 알림도) — 글을 임시 폴더의 magickey-*.txt 로 쓰고 메모킹에게 `--new-memo-file=` 로 넘긴다 (메모 파일은 메모킹만 쓴다)
#[tauri::command]
fn new_memo(text: String, open: bool, remind: bool, notes: State<Arc<Memos>>) -> Result<String, String> {
    let (Some(exe), true) = (&notes.exe, notes.can_make.load(Ordering::SeqCst)) else {
        return Err("메모킹을 업데이트하면 새 메모를 만들 수 있어요".into());
    };
    let text = text.trim();
    if text.is_empty() || text.len() > 1024 * 1024 {
        return Err("메모로 만들 글이 없어요".into());
    }
    let nanos = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let file = std::env::temp_dir().join(format!("magickey-{}-{nanos}.txt", std::process::id()));
    std::fs::write(&file, text).map_err(|e| format!("글을 넘기지 못했어요: {e}"))?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg(format!("--new-memo-file={}", file.display()));
    if open {
        cmd.arg("--open-new");
    }
    if remind {
        cmd.arg("--remind"); // 메모킹이 첫 줄의 날짜를 읽어 알림을 맞춘다
    }
    cmd.spawn().map_err(|e| {
        let _ = std::fs::remove_file(&file);
        format!("메모킹을 열지 못했어요: {e}")
    })?;
    Ok(if open { String::new() } else { "메모킹에 새 메모를 만들었어요".into() })
}

/// 찾은 줄의 할 일: open (열기 · 실행) · reveal (폴더 열기) · copyPath (경로 복사)
#[tauri::command]
fn act(app: AppHandle, action: String, path: String, idx: State<Arc<Index>>) -> Result<String, String> {
    if !idx.shown.lock().unwrap().contains(&path) {
        return Err("찾은 결과가 아니에요".into());
    }
    let err = |e: &dyn std::fmt::Display| format!("열지 못했어요: {e}");
    match action.as_str() {
        "open" => app.opener().open_path(&path, None::<&str>).map(|_| String::new()).map_err(|e| err(&e)),
        "reveal" => app.opener().reveal_item_in_dir(&path).map(|_| String::new()).map_err(|e| err(&e)),
        "copyPath" => app.clipboard().write_text(path).map(|_| "경로를 복사했어요".into()).map_err(|e| err(&e)),
        _ => Err("모르는 할 일이에요".into()),
    }
}

#[tauri::command]
fn copy(app: AppHandle, text: String) -> Result<String, String> {
    app.clipboard().write_text(text).map(|_| "복사했어요".into()).map_err(|e| e.to_string())
}

/// 웹 검색 · AI 에게 묻기 · 사이트 열기 — 주소는 여기서 만든다 (창이 보낸 주소를 그대로 열지 않게)
#[tauri::command]
fn open_web(app: AppHandle, kind: String, site: String, text: String) -> Result<String, String> {
    let (url, paste, name) = match kind.as_str() {
        "site" => (web::site_url(&text).ok_or("사이트 주소가 아니에요")?, false, ""),
        "web" | "ai" => {
            let list = if kind == "web" { web::WEB_ENGINES } else { web::AI_SITES };
            let l = web::site_link(list, &site, &text).ok_or("모르는 사이트예요")?;
            (l.url, l.paste, l.name)
        }
        _ => return Err("모르는 할 일이에요".into()),
    };
    // 주소로 질문을 넘길 수 없는 사이트(Gemini)는 질문을 복사해 두고 연다
    if paste {
        app.clipboard().write_text(text.trim()).map_err(|e| e.to_string())?;
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())?;
    Ok(if paste { format!("{name}를 열었어요. 질문을 복사해 두었으니 Ctrl+V 로 붙여 넣어요") } else { String::new() })
}

#[tauri::command]
fn hide(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

/// 단축키: 숨어 있으면 띄우고, 떠 있으면 숨긴다
fn toggle(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    let _ = w.show();
    let _ = w.set_focus();
    let _ = w.emit("magic:show", ());
    let idx = app.state::<Arc<Index>>();
    idx.shown.lock().unwrap().clear();
    idx.touch();
    let notes = app.state::<Arc<Memos>>();
    notes.shown.lock().unwrap().clear();
    notes.touch();
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| toggle(app))) // 두 번 켜면 이미 켜진 창을 띄운다
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, e| {
                    if e.state() == ShortcutState::Pressed {
                        toggle(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_local_data_dir()?;
            app.manage(Index::new(dir.join("index.tsv")));
            let notes = Memos::find(app.handle());
            let has_memo = notes.root.is_some();
            app.manage(notes);
            let file = dir.join("hotkey.txt");
            let hotkey = std::fs::read_to_string(&file)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space));
            app.manage(Hotkey { now: Mutex::new(hotkey), file, notice: Mutex::default() });

            // 알림 영역 ✦ — 왼쪽 클릭: 열기 · 오른쪽 클릭: 메뉴 (설정은 여기에만, 창은 깔끔하게)
            let open = MenuItem::with_id(app, "open", "열기", true, None::<&str>)?;
            let hotkey_item = MenuItem::with_id(app, "hotkey", "단축키 바꾸기…", true, None::<&str>)?;
            let mods = MODULES
                .iter()
                .filter(|(id, _)| has_memo || !matches!(*id, "memo" | "notes")) // 메모킹이 없으면 메모 줄도 없다
                .map(|(id, name)| CheckMenuItem::with_id(app, format!("mod:{id}"), name, true, true, None::<&str>))
                .collect::<tauri::Result<Vec<_>>>()?;
            let autostart = CheckMenuItem::with_id(app, "autostart", "Windows 시작할 때 켜기", true, app.autolaunch().is_enabled().unwrap_or(false), None::<&str>)?;
            let reindex = MenuItem::with_id(app, "reindex", "파일 목록 새로 만들기", true, None::<&str>)?;
            let sep = || PredefinedMenuItem::separator(app);
            let feats = Submenu::with_items(app, "기능", true, &mods.iter().map(|m| m as &dyn tauri::menu::IsMenuItem<Wry>).collect::<Vec<_>>())?;
            let settings = Submenu::with_items(app, "설정", true, &[&hotkey_item, &feats, &sep()?, &autostart, &reindex])?;
            let quit = MenuItem::with_id(app, "quit", "끝내기", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &sep()?, &settings, &sep()?, &quit])?;
            app.manage(TrayMenu { open, hotkey: hotkey_item, mods });
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().ok_or("아이콘이 없어요")?)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, e| match e.id.as_ref() {
                    "open" => toggle(app),
                    "hotkey" => {
                        toggle(app);
                        let _ = app.emit("magic:hotkey", ());
                    }
                    "autostart" => {
                        let al = app.autolaunch();
                        let on = autostart.is_checked().unwrap_or(false); // 누르면 체크가 먼저 바뀌어 온다
                        if (if on { al.enable() } else { al.disable() }).is_err() {
                            let _ = autostart.set_checked(al.is_enabled().unwrap_or(false));
                        }
                    }
                    "reindex" => {
                        let idx = app.state::<Arc<Index>>();
                        *idx.built.lock().unwrap() = None;
                        idx.touch();
                    }
                    "quit" => app.exit(0),
                    id => {
                        if let Some(m) = id.strip_prefix("mod:") {
                            let on = app.state::<TrayMenu>().mods.iter().find(|i| i.id().as_ref() == id).and_then(|i| i.is_checked().ok());
                            let _ = app.emit("magic:module", (m, on.unwrap_or(true)));
                        }
                    }
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        toggle(tray.app_handle());
                    }
                })
                .build(app)?;

            // 메모킹 매직키처럼 다른 프로그램이 이미 쓰고 있으면: 창을 한 번 띄워 알리고, 거기서 바로 다른 키로 바꾸게 한다
            let ok = app.global_shortcut().register(hotkey).is_ok();
            show_key(app.handle(), &hotkey, ok);
            if !ok {
                let msg = format!(
                    "{} 를 다른 프로그램이 쓰고 있어요 (메모킹은 창을 닫아도 알림 영역에 남아 이 키를 잡고 있어요). 여기를 눌러 다른 키로 바꾸세요",
                    key_label(&hotkey)
                );
                *app.state::<Hotkey>().notice.lock().unwrap() = Some(msg);
                toggle(app.handle());
            }
            Ok(())
        })
        .on_window_event(|w, e| {
            if let WindowEvent::Focused(false) = e {
                let _ = w.hide(); // 다른 곳을 누르면 숨는다
            }
        })
        .invoke_handler(tauri::generate_handler![query, init, set_hotkey, sync_modules, open_memo, new_memo, act, copy, open_web, hide])
        .run(tauri::generate_context!())
        .expect("MagicKey 를 켜지 못했어요");
}
