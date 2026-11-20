// 찾은 앱 · 파일 · 폴더의 실제 아이콘 (탐색기 · 시작 메뉴에 보이는 그 그림) — 화면은 fileicon:// 로 받는다 (ui/common.js realIcon)
//   셸 IShellItemImageFactory 에 아이콘만(SIIGBF_ICONONLY) 64px 로 달라고 해 PNG 로 바꾼다 — 바로가기(.lnk)는 가리키는 프로그램의 아이콘
//   같은 확장자의 문서는 그림이 같으니 확장자로 한 번만 만들고, 프로그램 · 바로가기 · 폴더는 경로마다 (메모리에만, 많아지면 비운다)
//   Windows 가 아니거나 그림을 못 얻으면 404 — 화면은 원래 선 그림을 그대로 쓴다
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// 화면 칸 30px 의 2배 남짓 (고해상도 화면에서도 또렷하게)
const PX: i32 = 64;
const CACHE_MAX: usize = 600;

fn cache() -> &'static Mutex<HashMap<String, Option<Vec<u8>>>> {
    static C: OnceLock<Mutex<HashMap<String, Option<Vec<u8>>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 경로마다 그림이 다른 종류 — 나머지 문서는 확장자가 같으면 그림도 같다
const OWN_ICON: [&str; 9] = ["exe", "lnk", "ico", "url", "appref-ms", "msc", "cpl", "scr", "dll"];

fn key(path: &str) -> String {
    let p = std::path::Path::new(path);
    match p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()) {
        Some(ext) if !OWN_ICON.contains(&ext.as_str()) && !p.is_dir() => format!(".{ext}"),
        _ => path.to_ascii_lowercase(),
    }
}

/// PNG 바이트 (없으면 None)
pub fn png(path: &str) -> Option<Vec<u8>> {
    let k = key(path);
    if let Some(hit) = cache().lock().ok()?.get(&k) {
        return hit.clone();
    }
    let made = render(path);
    if let Ok(mut c) = cache().lock() {
        if c.len() >= CACHE_MAX {
            c.clear();
        }
        c.insert(k, made.clone());
    }
    made
}

/// fileicon://localhost/<encodeURIComponent(경로)> (Windows 웹뷰는 http://fileicon.localhost/…) 의 경로 부분을 되돌린다
pub fn path_of(uri_path: &str) -> String {
    let raw = uri_path.trim_start_matches('/').as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            if let Ok(b) = u8::from_str_radix(std::str::from_utf8(&raw[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(raw[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(windows)]
fn render(path: &str) -> Option<Vec<u8>> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS};
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY};
    unsafe {
        // 아이콘을 그리는 셸 확장은 STA 를 바란다 — 이 일만 하는 스레드에서 부른다 (main.rs fileicon 프로토콜)
        let inited = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let out = (|| {
            let item: IShellItemImageFactory = SHCreateItemFromParsingName(&HSTRING::from(path), None).ok()?;
            let hbm = item.GetImage(SIZE { cx: PX, cy: PX }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK).ok()?;
            let mut bm = BITMAP::default();
            GetObjectW(hbm.into(), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut _));
            let (w, h) = (bm.bmWidth, bm.bmHeight);
            let mut buf = vec![0u8; (w.max(0) * h.max(0) * 4) as usize];
            let mut bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() },
                ..Default::default()
            };
            let dc = CreateCompatibleDC(None);
            let lines = GetDIBits(dc, hbm, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
            let _ = DeleteDC(dc);
            let _ = DeleteObject(hbm.into());
            if lines <= 0 || w <= 0 || h <= 0 {
                return None;
            }
            encode(bgra_to_rgba(buf), w as u32, h as u32)
        })();
        if inited {
            CoUninitialize();
        }
        out
    }
}

#[cfg(not(windows))]
fn render(_: &str) -> Option<Vec<u8>> {
    None
}

/// 셸 그림(BGRA, 보통 알파를 곱해 둔 꼴) → PNG 용 RGBA. 알파가 하나도 없는 옛 아이콘은 불투명으로
#[cfg_attr(not(windows), allow(dead_code))]
fn bgra_to_rgba(mut buf: Vec<u8>) -> Vec<u8> {
    let no_alpha = buf.as_chunks::<4>().0.iter().all(|p| p[3] == 0);
    let premul = !no_alpha && buf.as_chunks::<4>().0.iter().all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]);
    for p in buf.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        if no_alpha {
            p[3] = 255;
        } else if premul && p[3] > 0 && p[3] < 255 {
            let a = p[3] as u32;
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    buf
}

#[cfg_attr(not(windows), allow(dead_code))]
fn encode(rgba: Vec<u8>, w: u32, h: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().ok()?.write_image_data(&rgba).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 경로_되돌리기() {
        assert_eq!(path_of("/C%3A%5CUsers%5C%EB%AC%B8%EC%84%9C.xlsx"), "C:\\Users\\문서.xlsx");
        assert_eq!(path_of("/plain"), "plain");
    }

    #[test]
    fn 같은_확장자는_한_그림() {
        assert_eq!(key("D:\\a\\견적.XLSX"), key("C:\\b\\정산.xlsx"));
        assert_ne!(key("C:\\a.exe"), key("C:\\b.exe"));
        assert_ne!(key("C:\\a.lnk"), key("C:\\b.lnk"));
    }

    #[test]
    fn 알파_곱한_색_되돌리기() {
        // 반투명 빨강(곱한 꼴 B0 G0 R64 A128) → RGBA 128,0,0,128 쯤
        let out = bgra_to_rgba(vec![0, 0, 64, 128]);
        assert_eq!(out, vec![128, 0, 0, 128]);
        // 알파가 없는 옛 아이콘은 불투명
        assert_eq!(bgra_to_rgba(vec![10, 20, 30, 0])[3], 255);
    }

    #[test]
    fn png_만들기() {
        let p = encode(vec![255; 4 * 4], 2, 2).unwrap();
        assert_eq!(&p[1..4], b"PNG");
    }
}
