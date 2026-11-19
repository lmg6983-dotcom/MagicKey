//! WebView2 가 메모리를 덜 쓰게 — 메모킹(desktop-rs)과 같은 방법
//!   - 안 쓰는 크롬 기능을 끈다 (모든 창이 같은 인자로)
//!   - 숨은 창은 메모리 단계를 낮추고, 한동안 숨어 있으면 그리기도 멈춘다. 다시 띄우면 되돌린다

use std::time::Duration;
use tauri::{AppHandle, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};

/// wry 의 기본 인자(미니 메뉴 · 스마트스크린 끄기)에 안 쓰는 크롬 기능 끄기를 더한다 — 인자를 주면 기본 인자를 대신하므로 그대로 옮겨 적는다.
///   다음 페이지용으로 미리 띄워 두는 빈 렌더러 · 번역 · 최적화 힌트 · 미디어 캐스트 · 백그라운드 네트워크 · 구성 요소 업데이트.
///   한 앱의 창은 모두 같은 인자여야 한다 (다르면 WebView2 가 다음 창을 만들지 못한다) — 창은 꼭 window() 로 만든다
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,SpareRendererForSitePerProcess,Translate,OptimizationHints,MediaRouter --disable-background-networking --disable-component-update";

/// 이만큼 숨어 있으면 그리기도 멈춘다 (자주 여닫는 동안은 바로 뜨게 둔다)
const DEEP_HIDE: Duration = Duration::from_secs(60);

pub fn window<'a>(app: &'a AppHandle, label: &str, page: &str) -> WebviewWindowBuilder<'a, Wry, AppHandle> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into())).additional_browser_args(BROWSER_ARGS)
}

/// tauri.conf.json 의 창 (create: false 로 둔 것) 을 같은 인자로
pub fn from_config(app: &AppHandle, label: &str) -> tauri::Result<WebviewWindow> {
    let cfg = app.config().app.windows.iter().find(|w| w.label == label).cloned().ok_or(tauri::Error::WindowNotFound)?;
    WebviewWindowBuilder::from_config(app, &cfg)?.additional_browser_args(BROWSER_ARGS).build()
}

/// 숨겼다: 메모리를 낮추고, 한동안 그대로면 그리기도 멈춘다
pub fn hidden(w: &WebviewWindow) {
    memory_low(w, true);
    let w = w.clone();
    std::thread::spawn(move || {
        std::thread::sleep(DEEP_HIDE);
        set_visible(&w, false);
    });
}

/// 띄우기 직전: 그리기를 다시 켜고 메모리를 보통으로
pub fn showing(w: &WebviewWindow) {
    set_visible(w, true);
    memory_low(w, false);
}

/// WebView2 에 메모리를 덜 쓰라고(low) · 보통으로 — 캐시 · 풀어 둔 그림을 놓는다. 화면 · 상태는 그대로
#[cfg(windows)]
pub fn memory_low(w: &WebviewWindow, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL};
    use windows_core::Interface;
    let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
    let _ = w.with_webview(move |wv| unsafe {
        // 오래된 WebView2 런타임(_19 없음)이면 그대로
        if let Ok(core) = wv.controller().CoreWebView2().and_then(|c| c.cast::<ICoreWebView2_19>()) {
            let _ = core.SetMemoryUsageTargetLevel(level);
        }
    });
}
#[cfg(not(windows))]
pub fn memory_low(_w: &WebviewWindow, _low: bool) {}

/// WebView2 에 이 창이 보이는지 알린다 — Tauri 는 창을 숨겨도 웹뷰에는 알리지 않아, 숨은 창도 그리기 자원(화면 조각 · GPU 메모리)을 쥐고 있었다
#[cfg(windows)]
fn set_visible(w: &WebviewWindow, on: bool) {
    let win = w.clone();
    let _ = w.with_webview(move |wv| unsafe {
        // 끄기는 UI 스레드에서 그 순간에도 숨어 있을 때만 (그사이 다시 떴으면 빈 화면이 되지 않게)
        if on || !win.is_visible().unwrap_or(true) || win.is_minimized().unwrap_or(false) {
            let _ = wv.controller().SetIsVisible(on);
        }
    });
}
#[cfg(not(windows))]
fn set_visible(_w: &WebviewWindow, _on: bool) {}
