// 찾은 파일 · 폴더를 탐색기 · 바탕화면 · 다른 앱으로 끌어 놓기 (메모킹 desktop-rs/app/src/memos.rs start_file_drag 와 같은 방법)
//   셸 끌어 놓기(SHDoDragDrop)를 창 스레드(메인)에서 — 탐색기처럼 그 파일 · 폴더의 아이콘(그림이면 미리보기)이 마우스를 따라온다.
//   놓거나 Esc 를 누를 때까지 그 안에서 메시지를 돈다.
//   복사만 허락한다 (DROPEFFECT_COPY): 같은 드라이브로 끌면 Windows 기본은 '이동' 이라 원본이 사라질 수 있었다
use std::path::PathBuf;
use tauri::AppHandle;

/// hwnd: 끌기를 시작한 창 (끄는 그림을 그 창에서 시작한다)
#[cfg(windows)]
pub fn start(app: &AppHandle, hwnd: isize, path: PathBuf) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{IDropSource, DROPEFFECT_COPY};
    use windows::Win32::UI::Shell::{BHID_DataObject, IShellItem, SHCreateItemFromParsingName, SHDoDragDrop};
    let _ = app.run_on_main_thread(move || unsafe {
        let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&windows::core::HSTRING::from(path.as_os_str()), None) else { return };
        let Ok(data) = item.BindToHandler::<_, IDataObject>(None, &BHID_DataObject) else { return };
        // 끄는 쪽(drop source)은 셸 기본 — 끄는 그림 · 커서를 셸이 맡는다
        let _ = SHDoDragDrop(Some(HWND(hwnd as _)), &data, None::<&IDropSource>, DROPEFFECT_COPY);
    });
}

#[cfg(not(windows))]
pub fn start(_: &AppHandle, _: isize, _: PathBuf) {}

/// 끌기를 시작하기 전 한 번 — tao 가 OLE 를 이미 초기화했으면 아무 일도 하지 않는다 (S_FALSE)
#[cfg(windows)]
pub fn init() {
    unsafe {
        let _ = windows::Win32::System::Ole::OleInitialize(None);
    }
}

#[cfg(not(windows))]
pub fn init() {}
