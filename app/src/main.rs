// MagicKey — Ctrl+Alt+Space 로 어디서든 여는 만능 입력창 (앱 · 파일 · 폴더 찾기, 계산 · 날짜 계산, 웹 검색, 웹 AI)
//   창은 늘 떠 있다가 보였다 숨었다 한다 (다른 곳을 누르면 숨음). 끝내기는 알림 영역 아이콘에서.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use magickey_core::index::{self, Entry, Found};
use magickey_core::{calc, date, web, Answer};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime};
use tauri::menu::{Menu, MenuItem};
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
    #[serde(flatten)]
    found: Found,
    indexed: usize,
    building: bool,
}

/// 칠 때마다: 계산 · 날짜 계산, 사이트 주소, 묻는 글인가, 앱 · 폴더 · 파일 (mods = 켜 둔 기능)
///   async: 메인 스레드 밖에서 돈다 (이름 수십만 개를 훑는 동안 창이 굳지 않게)
#[tauri::command]
async fn query(text: String, mods: Vec<String>, idx: State<'_, Arc<Index>>) -> Result<QueryOut, String> {
    let on = |m: &str| mods.iter().any(|x| x == m);
    let text: String = text.chars().take(200).collect();
    let calc = on("calc").then(|| {
        date::date_calc(&text, chrono::Local::now().naive_local())
            .map(|answer| Calc { kind: "date", answer })
            .or_else(|| calc::calc(&text).map(|answer| Calc { kind: "calc", answer }))
    });
    let entries = idx.entries.read().unwrap().clone();
    let limits = (if on("apps") { 4 } else { 0 }, if on("files") { 3 } else { 0 }, if on("files") { 6 } else { 0 });
    let found = index::search(&entries, &text, limits);
    // 더하기만 한다 (답이 순서가 바뀌어 와도 창에 보이는 줄은 늘 열 수 있게) — 창을 띄울 때 비운다
    idx.shown.lock().unwrap().extend(found.apps.iter().chain(&found.folders).chain(&found.files).map(|h| h.path.clone()));
    Ok(QueryOut {
        calc: calc.flatten(),
        site: on("web").then(|| web::site_url(&text)).flatten(),
        question: web::looks_like_question(&text),
        found,
        indexed: entries.len(),
        building: idx.building.load(Ordering::SeqCst),
    })
}

/// 단축키를 못 잡았을 때 창에 띄울 안내
#[derive(Default)]
struct Notice(Mutex<Option<String>>);

#[derive(Serialize)]
struct Init {
    web: &'static [web::Site],
    ai: &'static [web::Site],
    notice: Option<String>,
}

#[tauri::command]
fn init(notice: State<Notice>) -> Init {
    Init { web: web::WEB_ENGINES, ai: web::AI_SITES, notice: notice.0.lock().unwrap().clone() }
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
}

fn main() {
    let hotkey = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space);
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| toggle(app))) // 두 번 켜면 이미 켜진 창을 띄운다
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, e| {
                    if e.state() == ShortcutState::Pressed {
                        toggle(app);
                    }
                })
                .build(),
        )
        .setup(move |app| {
            let cache = app.path().app_local_data_dir()?.join("index.tsv");
            app.manage(Index::new(cache));
            app.manage(Notice::default());

            let open = MenuItem::with_id(app, "open", "열기 (Ctrl+Alt+Space)", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "끝내기", true, None::<&str>)?;
            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().ok_or("아이콘이 없어요")?)
                .tooltip("MagicKey — Ctrl+Alt+Space")
                .menu(&Menu::with_items(app, &[&open, &quit])?)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => toggle(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        toggle(tray.app_handle());
                    }
                })
                .build(app)?;

            // 메모킹 매직키처럼 다른 프로그램이 이미 쓰고 있으면: 창을 한 번 띄워 알리고, 알림 영역 아이콘으로 연다
            if app.global_shortcut().register(hotkey).is_err() {
                let msg = "Ctrl+Alt+Space 를 다른 프로그램(메모킹 등)이 쓰고 있어요. 그 프로그램의 단축키를 끄고 MagicKey 를 다시 켜거나, 알림 영역의 ✦ 를 눌러 여세요";
                *app.state::<Notice>().0.lock().unwrap() = Some(msg.into());
                let _ = tray.set_tooltip(Some("MagicKey — 단축키를 못 잡았어요. 여기를 눌러 열기"));
                toggle(app.handle());
            }
            Ok(())
        })
        .on_window_event(|w, e| {
            if let WindowEvent::Focused(false) = e {
                let _ = w.hide(); // 다른 곳을 누르면 숨는다
            }
        })
        .invoke_handler(tauri::generate_handler![query, init, act, copy, open_web, hide])
        .run(tauri::generate_context!())
        .expect("MagicKey 를 켜지 못했어요");
}
