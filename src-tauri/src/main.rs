#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::{Manager, State};
use player::VideoPlayer;

#[cfg(windows)]
mod win32_child {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOZORDER, SW_SHOW,
        WS_CHILD, WS_CLIPSIBLINGS, WS_VISIBLE,
    };

    pub unsafe fn create_child_hwnd(parent: HWND, x: i32, y: i32, w: i32, h: i32) -> HWND {
        // "STATIC" is a standard built-in Win32 window class perfect for hosting custom renderers
        let class_name: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let child = CreateWindowExW(
            0,
            class_name.as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            x,
            y,
            w,
            h,
            parent,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        ShowWindow(child, SW_SHOW);
        child
    }

    pub unsafe fn resize_child_hwnd(child: HWND, x: i32, y: i32, w: i32, h: i32) {
        SetWindowPos(
            child,
            std::ptr::null_mut(),
            x,
            y,
            w,
            h,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

struct AppState {
    player: Mutex<Option<VideoPlayer>>,
    #[cfg(windows)]
    child_hwnd: Mutex<Option<isize>>,
}

#[derive(serde::Deserialize)]
struct HoleRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[tauri::command]
fn sync_video_rect(rect: HoleRect, state: State<AppState>) {
    #[cfg(windows)]
    {
        if let Some(hwnd) = *state.child_hwnd.lock().unwrap() {
            unsafe {
                win32_child::resize_child_hwnd(
                    hwnd as windows_sys::Win32::Foundation::HWND,
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                );
            }
        }
    }
}

#[tauri::command]
fn player_toggle_pause(state: State<AppState>) -> Result<bool, String> {
    if let Some(player) = state.player.lock().unwrap().as_ref() {
        player.toggle_pause().map_err(|e| e.to_string())
    } else {
        Err("Player not initialized".into())
    }
}

#[tauri::command]
fn player_seek(seconds: f64, state: State<AppState>) -> Result<(), String> {
    if let Some(player) = state.player.lock().unwrap().as_ref() {
        player.seek(seconds).map_err(|e| e.to_string())
    } else {
        Err("Player not initialized".into())
    }
}

fn main() {
    // Trim unused WebView2 browser subsystems to reduce idle memory footprint
    std::env::set_var(
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        "--disable-features=Translate,AutofillServerCommunication,OptimizationHints --disable-extensions --disable-component-update --disable-background-networking"
    );

    tauri::Builder::default()
        .manage(AppState {
            player: Mutex::new(None),
            #[cfg(windows)]
            child_hwnd: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            sync_video_rect,
            player_toggle_pause,
            player_seek
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            let _ = window.show();
            let _ = window.set_focus();

            #[cfg(windows)]
            {
                let raw_hwnd = window.hwnd().unwrap().0 as isize;
                unsafe {
                    // Create child window at (24, 70) sized 700x400 initially
                    let child = win32_child::create_child_hwnd(
                        raw_hwnd as windows_sys::Win32::Foundation::HWND,
                        24,
                        70,
                        700,
                        400,
                    );
                    let state = app.state::<AppState>();
                    *state.child_hwnd.lock().unwrap() = Some(child as isize);

                    // Initialize mpv on this child HWND
                    match VideoPlayer::new_with_hwnd(child as isize) {
                        Ok(player) => {
                            *state.player.lock().unwrap() = Some(player);
                            println!("[Litube Spike] libmpv successfully anchored to HWND: {:?}", child);
                        }
                        Err(e) => {
                            eprintln!("[Litube Spike] Error initializing libmpv on HWND: {e}");
                        }
                    }
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
