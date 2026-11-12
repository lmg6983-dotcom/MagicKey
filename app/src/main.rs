// MagicKey — Ctrl+Alt+Space (바꿀 수 있음) 로 어디서든 여는 만능 입력창 (메모킹 메모 · 앱 · 파일 · 폴더 찾기, 계산 · 날짜 계산, 웹 검색, 웹 AI)
//   창은 늘 떠 있다가 보였다 숨었다 한다 (다른 곳을 누르면 숨음). 끝내기는 알림 영역 아이콘에서.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use magickey_core::index::{self, Entry, Found};
use magickey_core::memo::{self, Memo, MemoHit, Shelf};
use magickey_core::{calc, date, web, Answer};
use magicsystem_core::{client as opms_client, search as opms_search, settings as opms_settings};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::Wry;
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_updater::{Update, UpdaterExt};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_opener::OpenerExt;

mod keyhook;

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
        let memoking = p.config_dir().ok().map(|d| d.join("MemoKing")).filter(|d| d.is_dir());
        // 메모킹에게 MagicKey 가 여기 있다고 알린다 → 메모킹(0.77~)은 매직키 단축키를 넘긴다 (메모킹이 없으면 폴더를 만들지 않는다)
        if let (Some(dir), Ok(exe)) = (&memoking, std::env::current_exe()) {
            let note = serde_json::json!({ "exe": exe, "pid": std::process::id(), "version": env!("CARGO_PKG_VERSION") }); // pid: 켜져 있을 때만 넘기게
            let _ = std::fs::write(dir.join("magickey.json"), note.to_string());
        }
        let file = memoking.map(|d| d.join("settings.json"));
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

    /// 메모킹 설정 글 (없으면 빈 글)
    fn settings_json(&self) -> String {
        self.settings.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default()
    }

    /// 뒤에서 메모 폴더를 다시 훑는다 (바뀐 파일만 새로 읽음, 그동안은 지난 목록으로 찾는다)
    fn touch(self: &Arc<Self>) {
        let Some(root) = self.root.clone() else { return };
        let json = self.settings_json();
        self.can_make.store(self.exe.is_some() && memo::knows_magickey(&json), Ordering::SeqCst);
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

/// MagicSystem (따로 설치하는 OPMS 찾기 앱, 예전 이름 MagicSystem) — 켜질 때 설정 폴더의 magicsystem.json 에 실행 파일 위치를 적는다
struct MagicSystem {
    /// %APPDATA% (설정 폴더들이 있는 곳)
    config: Option<PathBuf>,
    /// 메모킹 설정 (MagicSystem 설정에 없는 서버 주소 · 번호 양식은 여기서)
    memoking: Option<PathBuf>,
}

impl MagicSystem {
    /// 설치된 MagicSystem: (설정 폴더, 실행 파일) — 새 이름이 먼저, 없으면 예전 MagicSystem
    ///   (설치 · 삭제가 MagicKey 가 켜져 있는 사이에도 바뀌어 매번 본다)
    fn found(&self) -> Option<(PathBuf, PathBuf)> {
        let config = self.config.as_ref()?;
        [(opms_settings::APP_DIR, "magicsystem.json"), (opms_settings::LEGACY_APP_DIR, "magicopms.json")].into_iter().find_map(|(dir, note)| {
            let dir = config.join(dir);
            let exe = PathBuf::from(opms_settings::read(&dir.join(note)).get("exe")?.as_str()?);
            exe.is_file().then_some((dir, exe))
        })
    }

    fn exe(&self) -> Option<PathBuf> {
        self.found().map(|(_, exe)| exe)
    }

    /// MagicSystem 이 쓰는 서버 주소 · 배운 번호 양식 (MagicSystem 설정 > 메모킹 설정 — 같은 코어로)
    fn settings(&self) -> opms_settings::Settings {
        let own = self.found().map(|(d, _)| opms_settings::read(&d.join("settings.json"))).unwrap_or_default();
        let mk = self.memoking.as_ref().map(|f| opms_settings::read(f)).unwrap_or_default();
        opms_settings::effective(&own, &mk)
    }
}

/// OPMS 저절로 찾기 — 사건번호를 온전히 썼거나 한 번 찾아 본 번호와 같은 꼴이면, 고르기 전에 사건을 보여 준다
///   (메모킹 매직키와 같게: 정확한 것부터 6건, 나머지는 "OPMS 다른 n건 더 보기"). 짧게 기다리고 다시 하지 않는다
#[derive(Serialize)]
struct OpmsCase {
    idx: String,
    ourref: String,
    yourref: String,
    n_app: String,
    applicant: String,
    title: String,
    accepted: String,
}

#[tauri::command]
async fn opms_auto(text: String, mo: State<'_, MagicSystem>) -> Result<serde_json::Value, String> {
    let s = mo.settings();
    let q: String = text.trim().chars().take(40).collect();
    if s.host.is_empty() || mo.exe().is_none() || !opms_search::auto_query(&q, &s.shapes) {
        return Ok(serde_json::json!({ "cases": [], "total": 0 }));
    }
    let found = tauri::async_runtime::spawn_blocking(move || opms_client::quick_search(&s.host, &q, 60, Duration::from_secs(5))).await.map_err(|e| e.to_string())?;
    let (rows, total) = found.map_err(|e| if e.offline { "사내망에 연결돼 있지 않아 OPMS 를 찾지 못했어요".to_string() } else { e.message })?;
    let g = |r: &magicsystem_core::Row, k: &str| magicsystem_core::get(r, k).to_string();
    let cases: Vec<OpmsCase> = rows
        .iter()
        .filter(|(_, score)| *score > 0)
        .take(6)
        .map(|(r, _)| OpmsCase {
            idx: g(r, "idx"),
            ourref: g(r, "ourref"),
            yourref: g(r, "yourref"),
            n_app: g(r, "n_app"),
            applicant: g(r, "applicant"),
            title: g(r, "title_kor"),
            accepted: magicsystem_core::resultset::clean_date(&g(r, "d_accept")),
        })
        .collect();
    Ok(serde_json::json!({ "cases": cases, "total": total }))
}

/// OPMS 에서 찾기 — MagicSystem 을 `--search=<글>` 로 (켜져 있으면 그 창에서 찾는다)
#[tauri::command]
fn open_opms(text: String, mo: State<MagicSystem>) -> Result<(), String> {
    let exe = mo.exe().ok_or("MagicSystem 이 설치돼 있지 않아요")?;
    let q: String = text.lines().next().unwrap_or("").trim().chars().take(60).collect();
    if opms_search::query_kind(&q).is_none() {
        return Err("OPMS 에서 찾을 글이 아니에요".into());
    }
    std::process::Command::new(exe).arg(format!("--search={q}")).spawn().map(|_| ()).map_err(|e| format!("MagicSystem 을 열지 못했어요: {e}"))
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
    /// 'OPMS 에서 찾기' 줄 (MagicSystem 이 있고, 사건번호 · 이름 · 초성 같은 글)
    opms: bool,
    /// 고르기 전에 저절로 찾을 글 (온전한 사건번호 · 찾아 본 번호와 같은 꼴, 서버 주소가 있음) → 창이 opms_auto 를 부른다
    #[serde(rename = "opmsAuto")]
    opms_auto: bool,
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
async fn query(text: String, mods: Vec<String>, idx: State<'_, Arc<Index>>, notes: State<'_, Arc<Memos>>, mo: State<'_, MagicSystem>) -> Result<QueryOut, String> {
    let on = |m: &str| mods.iter().any(|x| x == m);
    let text: String = text.chars().take(200).collect();
    let calc = on("calc").then(|| {
        date::date_calc(&text, chrono::Local::now().naive_local())
            .map(|answer| Calc { kind: "date", answer })
            .or_else(|| calc::calc(&text).map(|answer| Calc { kind: "calc", answer }))
    });
    let show_opms = on("opms") && opms_search::query_kind(&text).is_some() && mo.exe().is_some();
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
        opms: show_opms,
        opms_auto: show_opms && { let s = mo.settings(); !s.host.is_empty() && opms_search::auto_query(text.trim(), &s.shapes) },
        memo_more,
        found,
        indexed: entries.len(),
        building: idx.building.load(Ordering::SeqCst),
    })
}

/// 여는 단축키 — 지금 키, 기억할 파일, 키보드 훅을 걸었는가, 못 잡았을 때 창에 띄울 안내
struct Hotkey {
    now: Mutex<Shortcut>,
    file: PathBuf,
    hooked: bool,
    notice: Mutex<Option<String>>,
}

fn default_key() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space)
}

/// 사람이 읽는 이름: Ctrl+Alt+Space, Ctrl+Shift+K, Alt+1
fn key_label(s: &Shortcut) -> String {
    keyhook::parts(s.mods, Some(s.key)).join("+")
}

/// 창의 기능 캡슐과 같은 것 — 알림 영역 메뉴 '설정' 에서도 켜고 끈다 (id 는 magic.js MODULES 와 같게)
const MODULES: [(&str, &str); 8] = [("memo", "새 메모 (메모킹)"), ("notes", "메모 검색 (메모킹)"), ("opms", "OPMS 검색 (MagicSystem)"), ("apps", "앱 검색"), ("files", "파일 검색"), ("calc", "계산 · 날짜 계산"), ("web", "웹 검색"), ("ai", "웹 AI")];

/// 알림 영역 메뉴에서 글자 · 체크를 바꿔야 하는 줄
struct TrayMenu {
    open: MenuItem<Wry>,
    /// 업데이트 확인 · 설치 (글로 지금 상태를 보인다)
    update: MenuItem<Wry>,
    hotkey: MenuItem<Wry>,
    mods: Vec<CheckMenuItem<Wry>>,
}

// ---------- 자동 업데이트 ----------
//   GitHub Release 의 latest.json 을 본다 (서명이 맞는 설치 파일만). 켜고 1분 뒤 · 6시간마다 조용히 확인하고,
//   새 버전이 있으면 업데이트 창을 띄운다 (바뀐 점 → 받기 · 진행률 → 설치하고 다시 켜기 — 메모킹 업데이트 창과 같은 흐름).
//   알림 영역 메뉴 '업데이트 확인' 도 이 창을 연다

/// 업데이트 창이 그리는 지금 상태
#[derive(Clone, Serialize)]
struct UpdState {
    /// idle · checking · latest · available · downloading · error
    status: &'static str,
    current: &'static str,
    version: Option<String>,
    notes: Option<String>,
    percent: u8,
    message: Option<String>,
    /// 자동 업데이트 정보가 없는 릴리스 — 창이 '릴리스 페이지에서 받기' 를 보인다
    manual: bool,
}

impl Default for UpdState {
    fn default() -> Self {
        UpdState { status: "idle", current: env!("CARGO_PKG_VERSION"), version: None, notes: None, percent: 0, message: None, manual: false }
    }
}

#[derive(Default)]
struct Updates {
    /// 찾아 둔 새 버전 (받기를 누르면 설치)
    found: Mutex<Option<Update>>,
    state: Mutex<UpdState>,
    /// '나중에' 를 누른 버전 — 조용한 확인에서는 창을 다시 띄우지 않는다 (MagicKey 를 다시 켜면 잊음)
    later: Mutex<Option<String>>,
}

/// 상태를 바꾸고 업데이트 창 · 알림 영역 메뉴에 알린다
fn set_update(app: &AppHandle, f: impl FnOnce(&mut UpdState)) -> UpdState {
    let updates = app.state::<Updates>();
    let st = {
        let mut st = updates.state.lock().unwrap();
        f(&mut st);
        st.clone()
    };
    let text = match (st.status, &st.version) {
        ("available", Some(v)) => format!("✨ v{v} 으로 업데이트…"),
        ("downloading", Some(v)) => format!("v{v} 받는 중 {}%", st.percent),
        _ => format!("업데이트 확인… (v{})", st.current),
    };
    let _ = app.state::<TrayMenu>().update.set_text(text);
    let _ = app.emit_to("update", "update:state", &st);
    st
}

/// 새 버전 확인. 조용한 확인은 '확인 중' 을 보이지 않고, 확인하지 못해도 지난 상태를 그대로 둔다
async fn check_update(app: &AppHandle, by_hand: bool) -> UpdState {
    let before = app.state::<Updates>().state.lock().unwrap().clone();
    if before.status == "downloading" {
        return before;
    }
    if by_hand {
        set_update(app, |s| s.status = "checking");
    }
    let found = match app.updater() {
        Ok(u) => u.check().await,
        Err(e) => Err(e),
    };
    match found {
        Ok(Some(u)) => {
            let (version, notes) = (u.version.clone(), u.body.clone());
            *app.state::<Updates>().found.lock().unwrap() = Some(u);
            if let Some(tray) = app.tray_by_id("main") {
                let _ = tray.set_tooltip(Some(format!("MagicKey — 새 버전 v{version} 이 있어요")));
            }
            set_update(app, |s| *s = UpdState { status: "available", version: Some(version), notes, ..UpdState::default() })
        }
        Ok(None) => {
            *app.state::<Updates>().found.lock().unwrap() = None;
            set_update(app, |s| *s = UpdState { status: "latest", ..UpdState::default() })
        }
        Err(_) if !by_hand => set_update(app, |s| *s = before),
        // 릴리스에 latest.json 이 없다 (서명 키 없이 만든 릴리스) — 인터넷 탓이 아니니 손으로 받는 길을 보인다
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => set_update(app, |s| {
            *s = UpdState {
                status: "error",
                message: Some("이 버전은 자동 업데이트 정보가 없어요. 릴리스 페이지에서 새 설치 파일을 받아 실행하면 그대로 덮어 설치돼요".into()),
                manual: true,
                ..UpdState::default()
            };
        }),
        Err(e) => set_update(app, |s| {
            s.status = "error";
            s.manual = false;
            s.message = Some(format!("업데이트를 확인하지 못했어요. 인터넷 연결을 확인하고 다시 눌러 주세요 ({e})"));
        }),
    }
}

/// 업데이트 창 — 이미 있으면 앞으로
fn open_update(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("update") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(app, "update", tauri::WebviewUrl::App("update.html".into()))
        .title("MagicKey 업데이트")
        .inner_size(460.0, 172.0) // "확인 중" 화면 높이 — 뒤에 update.js fit 이 내용에 맞춘다
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .center()
        .build();
}

#[tauri::command]
fn update_state(u: State<Updates>) -> UpdState {
    u.state.lock().unwrap().clone()
}

#[tauri::command]
async fn update_check(app: AppHandle) -> UpdState {
    check_update(&app, true).await
}

/// 받아서 설치 — 설치 파일이 MagicKey 를 끄고 새 버전을 켠다 (진행률은 update:state 로)
#[tauri::command]
async fn update_install(app: AppHandle) -> Result<(), String> {
    let Some(u) = app.state::<Updates>().found.lock().unwrap().clone() else {
        return Err("받을 새 버전이 없어요. 다시 확인해 주세요".into());
    };
    set_update(&app, |s| {
        s.status = "downloading";
        s.percent = 0;
        s.message = None;
    });
    let (mut got, mut shown) = (0u64, 0u8);
    let done = u
        .download_and_install(
            |chunk, total| {
                got += chunk as u64;
                let p = total.filter(|t| *t > 0).map_or(0, |t| (got * 100 / t).min(100) as u8);
                if p != shown {
                    shown = p;
                    set_update(&app, |s| s.percent = p);
                }
            },
            || {},
        )
        .await;
    if let Err(e) = done {
        set_update(&app, |s| {
            s.status = "error";
            s.message = Some(format!("업데이트하지 못했어요 ({e}). 다시 눌러 주세요"));
        });
        return Err(e.to_string());
    }
    Ok(())
}

/// 릴리스 페이지 (자동 업데이트 정보가 없을 때 손으로 받기)
#[tauri::command]
fn update_page(app: AppHandle) -> Result<(), String> {
    app.opener().open_url("https://github.com/lmg6983-dotcom/MagicKey/releases/latest", None::<&str>).map_err(|e| e.to_string())
}

/// '나중에' — 이 버전은 조용한 확인에서 다시 묻지 않는다
#[tauri::command]
fn update_later(app: AppHandle) {
    let u = app.state::<Updates>();
    *u.later.lock().unwrap() = u.state.lock().unwrap().version.clone();
    if let Some(w) = app.get_webview_window("update") {
        let _ = w.close();
    }
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
    /// 메모킹 매직키에서 꺼 둔 기능 — 창에서 캡슐을 아직 만지지 않았으면 이대로
    #[serde(rename = "memokingOff")]
    memoking_off: Option<Vec<String>>,
    /// MagicSystem 이 있는가 (아니면 'OPMS' 캡슐을 숨긴다)
    opms: bool,
}

#[tauri::command]
fn init(hk: State<Hotkey>, notes: State<Arc<Memos>>, mo: State<MagicSystem>) -> Init {
    let hotkey = key_label(&hk.now.lock().unwrap());
    Init { web: web::WEB_ENGINES, ai: web::AI_SITES, notice: hk.notice.lock().unwrap().clone(), hotkey, memo: notes.root.is_some(), memoking_off: memo::magic_off(&notes.settings_json()), opms: mo.exe().is_some() }
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

/// 단축키 창 — 알림 영역 메뉴 '단축키 바꾸기' · 매직키 창의 안내에서 연다 (새 키는 키보드 훅으로 받는다: keyhook.rs)
fn open_hotkey(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("hotkey") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(app, "hotkey", tauri::WebviewUrl::App("hotkey.html".into()))
        .title("MagicKey 단축키")
        .inner_size(440.0, 360.0) // hotkey.js fit 이 내용에 맞춘다
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .center()
        .build();
}

#[tauri::command]
fn hotkey_open(app: AppHandle) {
    open_hotkey(&app);
}

#[derive(Serialize)]
struct KeyInfo {
    parts: Vec<String>,
    key: String,
    warn: Option<String>,
}

impl KeyInfo {
    fn of(s: Shortcut) -> Self {
        KeyInfo { parts: keyhook::parts(s.mods, Some(s.key)), key: s.to_string(), warn: keyhook::check(&s).1 }
    }
}

#[derive(Serialize)]
struct HotkeyInfo {
    now: KeyInfo,
    /// 추천 — 맨 앞이 기본값
    suggest: Vec<KeyInfo>,
}

#[tauri::command]
fn hotkey_state(hk: State<Hotkey>) -> HotkeyInfo {
    let (ca, cs) = (Modifiers::CONTROL | Modifiers::ALT, Modifiers::CONTROL | Modifiers::SHIFT);
    let suggest = [default_key(), Shortcut::new(Some(Modifiers::ALT), Code::Space), Shortcut::new(Some(cs), Code::Space), Shortcut::new(Some(ca), Code::KeyM)];
    HotkeyInfo { now: KeyInfo::of(*hk.now.lock().unwrap()), suggest: suggest.into_iter().map(KeyInfo::of).collect() }
}

/// 새 키 고르기 시작 · 끝 — 고르는 동안 누른 조합은 이 창에만 간다 (hotkey:pressed)
#[tauri::command]
fn hotkey_record(on: bool, window: tauri::WebviewWindow) {
    #[cfg(windows)]
    let hwnd = window.hwnd().ok().map(|h| h.0 as isize);
    #[cfg(not(windows))]
    let hwnd = Some(0).filter(|_| window.label() == "hotkey");
    keyhook::record(hwnd.filter(|_| on));
}

/// 단축키 바꾸기 (key = "control+shift+KeyK" 꼴)
#[tauri::command]
fn set_hotkey(app: AppHandle, key: String, hk: State<Hotkey>) -> Result<String, String> {
    let new: Shortcut = key.parse().map_err(|_| "이 키는 단축키로 쓸 수 없어요".to_string())?;
    if let (Some(why), _) = keyhook::check(&new) {
        return Err(why);
    }
    let mut now = hk.now.lock().unwrap();
    let gs = app.global_shortcut();
    if new.id() != now.id() {
        let _ = gs.unregister(*now); // 못 잡고 있던 키면 놓을 것도 없다
    }
    // 전역 단축키는 관리자 권한 창이 앞에 있을 때를 위한 것 — 다른 프로그램이 이미 쓰고 있어도 훅이 연다
    let registered = gs.is_registered(new) || gs.register(new).is_ok();
    keyhook::set_key(&new);
    *now = new;
    *hk.notice.lock().unwrap() = None;
    if let Some(dir) = hk.file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&hk.file, new.to_string()); // 기억 못 해도 지금은 된다
    show_key(&app, &new, hk.hooked || registered);
    Ok(key_label(&new))
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
    // 메모킹(Node)과 같은 임시 폴더: TEMP 먼저 (Rust 의 temp_dir 은 TMP 먼저라, 둘이 다른 PC 에서 메모킹이 파일을 거절한다)
    let tmp = std::env::var_os("TEMP").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let file = tmp.join(format!("magickey-{}-{nanos}.txt", std::process::id()));
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
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            let cfg = app.path().config_dir().ok();
            let mo = MagicSystem { memoking: cfg.as_ref().map(|d| d.join("MemoKing").join("settings.json")), config: cfg };
            let has_opms = mo.exe().is_some();
            app.manage(mo);

            let file = dir.join("hotkey.txt");
            // 내가 바꾼 키 > 메모킹 매직키에서 쓰던 키 > Ctrl+Alt+Space
            let memoking = app.state::<Arc<Memos>>().settings_json();
            let hotkey = std::fs::read_to_string(&file)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .or_else(|| memo::magic_key(&memoking)?.parse().ok())
                .filter(|s| keyhook::check(s).0.is_none())
                .unwrap_or_else(default_key);
            let hooked = keyhook::start(app.handle().clone(), &hotkey);
            app.manage(Hotkey { now: Mutex::new(hotkey), file, hooked, notice: Mutex::default() });

            // 알림 영역 ✦ — 왼쪽 클릭: 열기 · 오른쪽 클릭: 메뉴 (설정은 여기에만, 창은 깔끔하게)
            let open = MenuItem::with_id(app, "open", "열기", true, None::<&str>)?;
            let hotkey_item = MenuItem::with_id(app, "hotkey", "단축키 바꾸기…", true, None::<&str>)?;
            let mods = MODULES
                .iter()
                .filter(|(id, _)| match *id {
                    "memo" | "notes" => has_memo, // 메모킹이 없으면 메모 줄도 없다
                    "opms" => has_opms,
                    _ => true,
                })
                .map(|(id, name)| CheckMenuItem::with_id(app, format!("mod:{id}"), name, true, *id != "opms", None::<&str>))
                .collect::<tauri::Result<Vec<_>>>()?;
            let autostart = CheckMenuItem::with_id(app, "autostart", "Windows 시작할 때 켜기", true, app.autolaunch().is_enabled().unwrap_or(false), None::<&str>)?;
            let reindex = MenuItem::with_id(app, "reindex", "파일 목록 새로 만들기", true, None::<&str>)?;
            let update = MenuItem::with_id(app, "update", format!("업데이트 확인… (v{})", env!("CARGO_PKG_VERSION")), true, None::<&str>)?;
            let sep = || PredefinedMenuItem::separator(app);
            let feats = Submenu::with_items(app, "기능", true, &mods.iter().map(|m| m as &dyn tauri::menu::IsMenuItem<Wry>).collect::<Vec<_>>())?;
            let settings = Submenu::with_items(app, "설정", true, &[&hotkey_item, &feats, &sep()?, &autostart, &reindex, &sep()?, &update])?;
            let quit = MenuItem::with_id(app, "quit", "끝내기", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &sep()?, &settings, &sep()?, &quit])?;
            app.manage(TrayMenu { open, update, hotkey: hotkey_item, mods });
            app.manage(Updates::default());
            // 켜고 1분 뒤 · 그 뒤 6시간마다 조용히 확인 (로그인 직후 네트워크가 늦게 잡혀도 되게) — 새 버전이면 창을 띄운다 ('나중에' 한 버전은 빼고)
            let h = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(60));
                loop {
                    let st = tauri::async_runtime::block_on(check_update(&h, false));
                    if st.status == "available" && *h.state::<Updates>().later.lock().unwrap() != st.version {
                        open_update(&h);
                    }
                    std::thread::sleep(Duration::from_secs(6 * 3600));
                }
            });
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().ok_or("아이콘이 없어요")?)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, e| match e.id.as_ref() {
                    "open" => toggle(app),
                    "hotkey" => open_hotkey(app),
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
                    "update" => {
                        open_update(app);
                        // 찾아 둔 새 버전이 없으면 창을 연 김에 확인 (받는 중이면 그대로)
                        if app.state::<Updates>().found.lock().unwrap().is_none() {
                            let a = app.clone();
                            tauri::async_runtime::spawn(async move { check_update(&a, true).await });
                        }
                    }
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

            // 키보드 훅이 먼저 본다 (다른 프로그램 · 입력기가 가로채지 못하게). 전역 단축키는 관리자 권한 창이 앞에 있을 때를 위한 것 —
            //   둘 다 못 잡았을 때만 창을 띄워 알린다
            let registered = app.global_shortcut().register(hotkey).is_ok();
            show_key(app.handle(), &hotkey, hooked || registered);
            if !hooked && !registered {
                *app.state::<Hotkey>().notice.lock().unwrap() = Some(format!("{} 를 다른 프로그램이 쓰고 있어요. 여기를 눌러 다른 키로 바꾸세요", key_label(&hotkey)));
                toggle(app.handle());
            }
            Ok(())
        })
        .on_window_event(|w, e| {
            match (e, w.label()) {
                (WindowEvent::Focused(false), "main") => {
                    let _ = w.hide(); // 다른 곳을 누르면 숨는다 (업데이트 · 단축키 창은 그대로)
                }
                (WindowEvent::Focused(false) | WindowEvent::Destroyed, "hotkey") => keyhook::record(None), // 키 고르기는 이 창이 앞에 있을 때만
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![query, init, set_hotkey, hotkey_open, hotkey_state, hotkey_record, sync_modules, open_memo, new_memo, open_opms, opms_auto, act, copy, open_web, hide, update_state, update_check, update_install, update_later, update_page])
        .run(tauri::generate_context!())
        .expect("MagicKey 를 켜지 못했어요");
}
