// 찾은 파일 · 폴더를 탐색기 · 바탕화면 · 다른 앱으로 끌어 놓기 (메모킹 desktop-rs/app/src/memos.rs start_file_drag 와 같은 방법)
//   OLE DoDragDrop 을 창 스레드(메인)에서 — 놓거나 Esc 를 누를 때까지 그 안에서 메시지를 돌린다.
//   복사만 허락한다 (DROPEFFECT_COPY): 같은 드라이브로 끌면 Windows 기본은 '이동' 이라 원본이 사라질 수 있었다
use std::path::PathBuf;
use tauri::AppHandle;

#[cfg(windows)]
pub fn start(app: &AppHandle, path: PathBuf) {
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{DoDragDrop, IDropSource, DROPEFFECT, DROPEFFECT_COPY};
    use windows::Win32::UI::Shell::{BHID_DataObject, IShellItem, SHCreateItemFromParsingName};
    let _ = app.run_on_main_thread(move || unsafe {
        let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&windows::core::HSTRING::from(path.as_os_str()), None) else { return };
        let Ok(data) = item.BindToHandler::<_, IDataObject>(None, &BHID_DataObject) else { return };
        let mut effect = DROPEFFECT::default();
        let _ = DoDragDrop(&data, &IDropSource::from(DropSource), DROPEFFECT_COPY, &mut effect);
    });
}

#[cfg(not(windows))]
pub fn start(_: &AppHandle, _: PathBuf) {}

/// 끌기를 시작하기 전 한 번 — tao 가 OLE 를 이미 초기화했으면 아무 일도 하지 않는다 (S_FALSE)
#[cfg(windows)]
pub fn init() {
    unsafe {
        let _ = windows::Win32::System::Ole::OleInitialize(None);
    }
}

#[cfg(not(windows))]
pub fn init() {}

#[cfg(windows)]
#[windows_core::implement(windows::Win32::System::Ole::IDropSource)]
struct DropSource;

#[cfg(windows)]
impl windows::Win32::System::Ole::IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(&self, escape: windows::core::BOOL, keys: windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS) -> windows::core::HRESULT {
        use windows::Win32::Foundation::{DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, S_OK};
        if escape.as_bool() {
            DRAGDROP_S_CANCEL
        } else if keys.0 & windows::Win32::System::SystemServices::MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _: windows::Win32::System::Ole::DROPEFFECT) -> windows::core::HRESULT {
        windows::Win32::Foundation::DRAGDROP_S_USEDEFAULTCURSORS
    }
}
