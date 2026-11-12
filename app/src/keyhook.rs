// 여는 단축키를 Windows 저수준 키보드 훅으로 직접 본다.
//   RegisterHotKey(전역 단축키)는 다른 프로그램의 키보드 훅 · 입력기(IME) 다음 차례라, Ctrl+Space 같은 키를 쓰는 기능이
//   Ctrl+Alt+Space 를 먼저 가로챌 수 있었다. 훅은 그보다 앞에서 보고, 우리 키면 삼켜서 다른 곳에 가지 않게 한다.
//   - 관리자 권한 창이 앞에 있으면 훅이 불리지 않는다 (Windows 규칙) — 그때는 main.rs 의 전역 단축키가 대신 연다
//   - 새 단축키를 고르는 동안(단축키 창이 앞에 있을 때만)은 모든 조합을 받아 창에 넘긴다 — Ctrl+Space · Win+… 도 고를 수 있게
//   - 뒤에 켜진 프로그램의 훅이 앞서지 않게, 그리고 Windows 가 느린 훅을 몰래 떼는 일에 대비해 30초마다 다시 건다
use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

/// 창에 넘기는 키 — 고르는 동안 누른 것 (조합키만 누르고 있으면 key 는 None)
#[derive(Serialize, Clone)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Pressed {
    /// 글쇠 모양으로 그릴 이름들: ["Ctrl", "Alt", "Space"]
    pub parts: Vec<String>,
    /// set_hotkey 에 넘길 글 ("control+alt+Space") — 조합키만이면 None
    pub key: Option<String>,
    /// 못 쓰는 까닭 · 겹치는 기능 (check)
    pub problem: Option<String>,
    pub warn: Option<String>,
}

/// 사람이 읽는 이름 (글쇠 하나씩): Ctrl · Alt · Shift · Win, 그다음 키
pub fn parts(mods: Modifiers, key: Option<Code>) -> Vec<String> {
    let mut out: Vec<String> = [(Modifiers::CONTROL, "Ctrl"), (Modifiers::ALT, "Alt"), (Modifiers::SHIFT, "Shift"), (Modifiers::SUPER, "Win")]
        .iter()
        .filter(|(m, _)| mods.contains(*m))
        .map(|(_, n)| n.to_string())
        .collect();
    if let Some(k) = key {
        let s = k.to_string();
        let name = match s.as_str() {
            "Backquote" => "`", "Minus" => "-", "Equal" => "=", "BracketLeft" => "[", "BracketRight" => "]", "Backslash" => "\\",
            "Semicolon" => ";", "Quote" => "'", "Comma" => ",", "Period" => ".", "Slash" => "/",
            "ArrowUp" => "↑", "ArrowDown" => "↓", "ArrowLeft" => "←", "ArrowRight" => "→", "Escape" => "Esc", "Backspace" => "Backspace",
            "PageUp" => "PgUp", "PageDown" => "PgDn", "Delete" => "Del", "Insert" => "Ins",
            _ => s.strip_prefix("Key").or_else(|| s.strip_prefix("Digit")).unwrap_or(&s),
        };
        out.push(name.to_string());
    }
    out
}

/// 이 키를 쓸 수 있나 — (못 쓰는 까닭, 쓰면 가려지는 기능 안내).
///   훅이 어디서든 먼저 받으니, 늘 쓰는 키를 고르면 그 키가 모든 프로그램에서 안 먹는다 — 그런 키는 막고, 겹치는 기능은 알린다
pub fn check(s: &Shortcut) -> (Option<String>, Option<String>) {
    let (m, k) = (s.mods, s.key);
    let ctrl = Modifiers::CONTROL;
    let letter_or_digit = k.to_string().starts_with("Key") || k.to_string().starts_with("Digit");
    if !m.intersects(ctrl | Modifiers::ALT | Modifiers::SUPER) {
        return (Some("Ctrl · Alt · Win 중 하나와 함께 눌러 주세요".into()), None);
    }
    let windows = [
        (Modifiers::ALT, Code::Tab), (Modifiers::ALT | Modifiers::SHIFT, Code::Tab), (Modifiers::ALT, Code::F4), (Modifiers::ALT, Code::Escape),
        (ctrl, Code::Escape), (ctrl | Modifiers::SHIFT, Code::Escape), (ctrl | Modifiers::ALT, Code::Delete), (Modifiers::SUPER, Code::KeyL),
    ];
    if windows.contains(&(m, k)) {
        return (Some("Windows 가 쓰는 키예요. 다른 조합을 눌러 주세요".into()), None);
    }
    if m == ctrl && matches!(k, Code::KeyA | Code::KeyC | Code::KeyV | Code::KeyX | Code::KeyZ | Code::KeyY | Code::KeyS) {
        return (Some("복사 · 붙여넣기처럼 늘 쓰는 키예요. 다른 조합을 눌러 주세요".into()), None);
    }
    let warn = match (m, k) {
        (_, Code::Space) if m == ctrl => "입력기 전환 · 자동 완성에 쓰는 키예요. 그 기능 대신 MagicKey 가 열려요",
        (_, Code::Space) if m == Modifiers::ALT => "창 메뉴(최소화 · 닫기)를 여는 키예요. 그 대신 MagicKey 가 열려요",
        (_, Code::Space) if m.contains(Modifiers::SUPER) => "입력 언어를 바꾸는 키예요. 그 대신 MagicKey 가 열려요",
        _ if m.contains(Modifiers::SUPER) => "Windows 기능 키와 겹칠 수 있어요",
        _ if (ctrl | Modifiers::SHIFT).contains(m) && letter_or_digit => "프로그램마다 쓰는 키예요. 어디서든 MagicKey 가 먼저 받아요",
        _ => return (None, None),
    };
    (None, Some(warn.into()))
}

/// 가상 키 번호 → 키 (Code 의 이름으로). 글자 · 숫자 · F 키 · 숫자판은 차례대로, 나머지는 표
pub fn vk_code(vk: u16) -> Option<Code> {
    let name = match vk {
        0x41..=0x5A => format!("Key{}", vk as u8 as char),
        0x30..=0x39 => format!("Digit{}", vk - 0x30),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x60..=0x69 => format!("Numpad{}", vk - 0x60),
        _ => match vk {
            0x20 => "Space", 0x0D => "Enter", 0x09 => "Tab", 0x08 => "Backspace", 0x1B => "Escape",
            0x21 => "PageUp", 0x22 => "PageDown", 0x23 => "End", 0x24 => "Home", 0x25 => "ArrowLeft", 0x26 => "ArrowUp", 0x27 => "ArrowRight", 0x28 => "ArrowDown",
            0x2C => "PrintScreen", 0x2D => "Insert", 0x2E => "Delete", 0x13 => "Pause", 0x91 => "ScrollLock",
            0x6A => "NumpadMultiply", 0x6B => "NumpadAdd", 0x6D => "NumpadSubtract", 0x6E => "NumpadDecimal", 0x6F => "NumpadDivide",
            0xBA => "Semicolon", 0xBB => "Equal", 0xBC => "Comma", 0xBD => "Minus", 0xBE => "Period", 0xBF => "Slash", 0xC0 => "Backquote",
            0xDB => "BracketLeft", 0xDC => "Backslash", 0xDD => "BracketRight", 0xDE => "Quote",
            _ => return None,
        }
        .to_string(),
    };
    name.parse().ok()
}

/// 키 → 가상 키 번호 (vk_code 를 거꾸로)
pub fn code_vk(code: Code) -> Option<u16> {
    (0..=0xFFu16).find(|&vk| vk_code(vk) == Some(code))
}

#[cfg_attr(not(windows), allow(dead_code))] // 훅은 Windows 에서만
struct State {
    /// 지금 여는 키 (조합키, 가상 키)
    key: Option<(Modifiers, u16)>,
    /// 새 키를 고르는 단축키 창 — 이 창이 앞에 있을 때만 키를 받는다
    recording: Option<isize>,
    /// 삼킨 키와 마지막으로 본 때(ms) — 뗄 때 · 꾹 눌러 반복될 때도 삼킨다 (떼는 것을 놓쳤으면 1초 뒤 잊는다)
    held: Option<(u16, u32)>,
    /// 고르는 동안 마지막으로 보낸 조합키 (같으면 다시 보내지 않는다)
    sent: Option<Modifiers>,
    /// 한국어 자판의 오른쪽 Alt(한/영) · 오른쪽 Ctrl(한자)을 누르고 있나 — 훅이 본 누름 · 뗌으로 (아래 win::korean)
    right: Modifiers,
}

static STATE: Mutex<State> = Mutex::new(State { key: None, recording: None, held: None, sent: None, right: Modifiers::empty() });
static APP: OnceLock<AppHandle> = OnceLock::new();

/// 여는 키를 바꾼다 (훅이 보는 키)
pub fn set_key(s: &Shortcut) {
    STATE.lock().unwrap().key = code_vk(s.key).map(|vk| (s.mods, vk));
}

/// 새 키 고르기 시작 · 끝 (hwnd: 단축키 창)
pub fn record(hwnd: Option<isize>) {
    let mut st = STATE.lock().unwrap();
    st.recording = hwnd;
    st.sent = None;
}

#[cfg(not(windows))]
pub fn start(app: AppHandle, s: &Shortcut) -> bool {
    let _ = APP.set(app);
    set_key(s);
    false
}

/// 훅을 거는 스레드를 띄운다 — 걸었으면 true (못 걸면 전역 단축키만으로)
#[cfg(windows)]
pub fn start(app: AppHandle, s: &Shortcut) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let _ = APP.set(app);
    set_key(s);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || unsafe {
        let hook = || SetWindowsHookExW(WH_KEYBOARD_LL, Some(win::proc), std::ptr::null_mut(), 0);
        let mut h = hook();
        let _ = tx.send(!h.is_null());
        if h.is_null() {
            return;
        }
        SetTimer(std::ptr::null_mut(), 0, 30_000, None);
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_TIMER {
                // 다시 걸어 맨 앞 차례로 (새로 건 훅이 먼저 불린다). 새것을 건 뒤에 옛것을 뗀다 — 그사이 빈틈이 없게
                let next = hook();
                if !next.is_null() {
                    UnhookWindowsHookEx(h);
                    h = next;
                }
            }
            DispatchMessageW(&msg);
        }
    });
    rx.recv().unwrap_or(false)
}

#[cfg(windows)]
mod win {
    use super::*;
    use tauri::Emitter;
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    // 한국어 자판은 오른쪽 Alt 가 한/영(VK_HANGUL), 오른쪽 Ctrl 이 한자(VK_HANJA) 키다 — 그대로 두면 오른쪽 Alt 로 누른
    //   Ctrl+Alt+Space 가 Windows 에는 Ctrl+Space 로 가서 그 키를 쓰는 기능이 열렸다. 여기서는 Alt · Ctrl 로 본다.
    //   따로 있는 한/영 · 한자 키는 떼는 신호가 없어(눌린 채로 남아) 빼고, 자리가 오른쪽 Alt · Ctrl(스캔 코드 + 확장 키)인 것만
    fn korean(vk: u16, scan: u32, extended: bool) -> Option<Modifiers> {
        match (vk, scan, extended) {
            (VK_HANGUL, 0x38, true) => Some(Modifiers::ALT),
            (VK_HANJA, 0x1D, true) => Some(Modifiers::CONTROL),
            _ => None,
        }
    }

    fn is_mod(vk: u16) -> bool {
        matches!(vk, VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_LWIN | VK_RWIN)
    }

    fn mod_of(vk: u16) -> Modifiers {
        match vk {
            VK_CONTROL | VK_LCONTROL | VK_RCONTROL => Modifiers::CONTROL,
            VK_MENU | VK_LMENU | VK_RMENU => Modifiers::ALT,
            VK_SHIFT | VK_LSHIFT | VK_RSHIFT => Modifiers::SHIFT,
            _ => Modifiers::SUPER,
        }
    }

    /// 지금 눌려 있는 조합키 (훅 안에서는 이번 키는 아직 반영 전) — right: 오른쪽 한/영 · 한자
    fn mods_now(right: Modifiers) -> Modifiers {
        let down = |vk: u16| unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000 != 0;
        let mut m = right;
        if down(VK_CONTROL) {
            m |= Modifiers::CONTROL;
        }
        if down(VK_MENU) {
            m |= Modifiers::ALT;
        }
        if down(VK_SHIFT) {
            m |= Modifiers::SHIFT;
        }
        if down(VK_LWIN) || down(VK_RWIN) {
            m |= Modifiers::SUPER;
        }
        m
    }

    /// Alt · Win 을 누른 채 키를 삼키면 떼는 순간 메뉴 막대 · 시작 메뉴가 열린다 — 아무 뜻 없는 키(0xE8)를 끼워 막는다
    fn mask(mods: Modifiers) {
        if !mods.intersects(Modifiers::ALT | Modifiers::SUPER) {
            return;
        }
        let key = |flags| INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0xE8, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } } };
        let inputs = [key(0), key(KEYEVENTF_KEYUP)];
        unsafe { SendInput(2, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    }

    fn emit(p: Pressed) {
        if let Some(app) = APP.get() {
            let _ = app.emit_to("hotkey", "hotkey:pressed", p);
        }
    }

    /// true 면 삼킨다 (다른 프로그램에 가지 않음)
    fn handle(msg: u32, vk: u16, scan: u32, extended: bool, at: u32) -> bool {
        let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let mut st = STATE.lock().unwrap();
        let recording = st.recording.is_some_and(|h| unsafe { GetForegroundWindow() } as isize == h);
        let korean = korean(vk, scan, extended);
        if let Some(m) = korean {
            st.right.set(m, down);
        }
        if korean.is_some() || is_mod(vk) {
            if recording {
                // 누르고 있는 조합키를 글쇠로 바로 보인다 (훅 안에서는 이번 키가 아직 반영 전이라 직접 더하고 뺀다)
                let this = korean.unwrap_or_else(|| mod_of(vk));
                let m = if down { mods_now(st.right) | this } else { mods_now(st.right) - this };
                if st.sent != Some(m) && st.held.is_none() {
                    st.sent = Some(m);
                    drop(st);
                    emit(Pressed { parts: parts(m, None), key: None, problem: None, warn: None });
                }
            }
            return false; // 조합키는 그대로 보낸다 (눌린 채로 남지 않게)
        }
        let held = st.held.is_some_and(|(k, t)| k == vk && at.wrapping_sub(t) < 1000);
        if !down {
            if held {
                st.held = None;
            }
            return held;
        }
        if held {
            st.held = Some((vk, at)); // 꾹 눌러 반복
            return true;
        }
        let mods = mods_now(st.right);
        if recording {
            st.held = Some((vk, at));
            st.sent = None;
            drop(st);
            mask(mods);
            let code = vk_code(vk);
            let shortcut = code.map(|c| Shortcut::new(Some(mods), c));
            let (problem, warn) = match &shortcut {
                Some(s) => check(s),
                None => (Some("이 키는 단축키로 쓸 수 없어요".into()), None),
            };
            let mut shown = parts(mods, code);
            if code.is_none() {
                shown.push(format!("키 {vk}"));
            }
            emit(Pressed { parts: shown, key: shortcut.map(|s| s.to_string()), problem, warn });
            return true;
        }
        if st.key == Some((mods, vk)) {
            st.held = Some((vk, at));
            drop(st);
            mask(mods);
            if let Some(app) = APP.get() {
                let a = app.clone();
                let _ = app.run_on_main_thread(move || crate::toggle(&a));
            }
            return true;
        }
        false
    }

    pub unsafe extern "system" fn proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let k = &*(l as *const KBDLLHOOKSTRUCT);
            // 보낸 키(SendInput — 우리 0xE8 · 창 띄울 때의 Alt 포함)는 보지 않는다
            if k.flags & LLKHF_INJECTED == 0 && handle(w as u32, k.vkCode as u16, k.scanCode, k.flags & LLKHF_EXTENDED != 0, k.time) {
                return 1;
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, w, l)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vk_round_trip() {
        for (vk, name) in [(0x20, "Space"), (0x4B, "KeyK"), (0x31, "Digit1"), (0x70, "F1"), (0x87, "F24"), (0xC0, "Backquote"), (0x60, "Numpad0")] {
            let c = vk_code(vk).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(c.to_string(), name);
            assert_eq!(code_vk(c), Some(vk));
        }
        assert_eq!(vk_code(0xE8), None);
    }

    #[test]
    fn names() {
        assert_eq!(parts(Modifiers::CONTROL | Modifiers::ALT, Some(Code::Space)), ["Ctrl", "Alt", "Space"]);
        assert_eq!(parts(Modifiers::SHIFT | Modifiers::SUPER, Some(Code::KeyK)), ["Shift", "Win", "K"]);
        assert_eq!(parts(Modifiers::CONTROL, Some(Code::Backquote)), ["Ctrl", "`"]);
        assert_eq!(parts(Modifiers::ALT, None), ["Alt"]);
        // 저장 글이 다시 읽힌다 (hotkey.txt)
        let s = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space);
        assert_eq!(s.to_string().parse::<Shortcut>().unwrap(), s);
    }

    #[test]
    fn rules() {
        let k = |m, c| check(&Shortcut::new(Some(m), c));
        assert_eq!(k(Modifiers::CONTROL | Modifiers::ALT, Code::Space), (None, None));
        assert!(k(Modifiers::SHIFT, Code::KeyK).0.is_some(), "조합키 없이");
        assert!(k(Modifiers::ALT, Code::Tab).0.is_some(), "Windows 키");
        assert!(k(Modifiers::CONTROL, Code::KeyC).0.is_some(), "복사");
        assert!(k(Modifiers::CONTROL, Code::Space).1.is_some(), "입력기 전환과 겹침");
        assert!(k(Modifiers::ALT, Code::Space).1.is_some());
        assert!(k(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyK).1.is_some());
        assert_eq!(k(Modifiers::CONTROL | Modifiers::SHIFT, Code::Space), (None, None));
    }
}
