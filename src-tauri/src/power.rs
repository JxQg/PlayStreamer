#[cfg(windows)]
pub fn watch(engine: &std::sync::Arc<playstreamer_core::Engine>) {
    use std::{sync::OnceLock, sync::Weak};
    use windows_sys::Win32::{
        Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*,
    };
    static ENGINE: OnceLock<Weak<playstreamer_core::Engine>> = OnceLock::new();
    let _ = ENGINE.set(std::sync::Arc::downgrade(engine));
    unsafe extern "system" fn procedure(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_POWERBROADCAST {
            if let Some(engine) = ENGINE.get().and_then(Weak::upgrade) {
                let result = match wparam {
                    4 if engine.snapshot().dns_running => {
                        Some(tauri::async_runtime::block_on(engine.suspend()))
                    }
                    7 | 18 => Some(tauri::async_runtime::block_on(engine.resume())),
                    _ => None,
                };
                if let Some(Err(error)) = result {
                    eprintln!("电源状态处理失败：{error}");
                }
            }
            return 1;
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
    std::thread::spawn(|| unsafe {
        let name: Vec<u16> = "PlayStreamerPowerEvents"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let module = GetModuleHandleW(std::ptr::null());
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: module,
            lpszClassName: name.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&class) == 0 {
            return;
        }
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            module,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

#[cfg(not(windows))]
pub fn watch(_: &std::sync::Arc<playstreamer_core::Engine>) {}
