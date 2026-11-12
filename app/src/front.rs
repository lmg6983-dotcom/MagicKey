// 창을 정말 앞으로 (키 입력을 받게) 올린다.
//   Windows 는 지금 앞에 있는 프로그램만 다른 창을 앞으로 올리게 둔다. 전역 단축키(RegisterHotKey)로 불리면 그 권한이 따라오지만,
//   키보드 훅(keyhook.rs)이 키를 삼키고 부르면 권한이 없다 — 그러면 창이 뜨기만 하고 포커스를 못 받아, 뜨자마자 숨거나 글이 쳐지지 않았다.
//   앞에 있는 창의 스레드에 입력을 잠깐 붙이면 그 창의 권한으로 올릴 수 있다 (실패하면 main.rs 가 tao 의 Alt 누르기로 한 번 더).
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::HWND,
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::WindowsAndMessaging::{BringWindowToTop, GetAncestor, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow, GA_ROOT},
};

/// 이 창이 지금 맨 앞(키 입력을 받는 창)인가
#[cfg(windows)]
pub fn is_front(hwnd: isize) -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        fg as isize == hwnd || (!fg.is_null() && GetAncestor(fg, GA_ROOT) as isize == hwnd) // 창 안의 창(웹뷰)이 앞이어도
    }
}

/// 창을 맨 앞으로 — 됐으면 true. 창을 만든 스레드(메인)에서 부른다
#[cfg(windows)]
pub fn bring(hwnd: isize) -> bool {
    if is_front(hwnd) {
        return true;
    }
    unsafe {
        let me = GetCurrentThreadId();
        let fg = GetForegroundWindow();
        let other = if fg.is_null() { 0 } else { GetWindowThreadProcessId(fg, std::ptr::null_mut()) };
        // 관리자 권한 창에는 붙지 못한다 (0 을 돌려준다) — 그때는 그냥 해 본다
        let attached = other != 0 && other != me && AttachThreadInput(me, other, 1) != 0;
        BringWindowToTop(hwnd as HWND);
        SetForegroundWindow(hwnd as HWND);
        if attached {
            AttachThreadInput(me, other, 0);
        }
    }
    is_front(hwnd)
}
