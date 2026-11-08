#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::{Manager, State};
use player::VideoPlayer;

#[cfg(windows)]
mod win32_child {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, SW_SHOW, WS_CHILD, WS_CLIPSIBLINGS, WS_VISIBLE, WS_EX_TRANSPARENT,
    };

    pub unsafe fn create_child_hwnd(parent: HWND, x: i32, y: i32, w: i32, h: i32) -> HWND {
        let class_name: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // WS_EX_TRANSPARENT ensures mouse and click events pass through to the WebView2 UI controls!
        let child = CreateWindowExW(
            WS_EX_TRANSPARENT as u32,
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

        // Put the child window at the BOTTOM of the sibling z-order so WebView2's controls always receive input
        SetWindowPos(
            child,
            1 as HWND, // HWND_BOTTOM
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );

        ShowWindow(child, SW_SHOW);
        child
    }

    pub unsafe fn resize_child_hwnd(child: HWND, x: i32, y: i32, w: i32, h: i32) {
        SetWindowPos(
            child,
            1 as HWND, // HWND_BOTTOM
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE,
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
                // Ensure the child window remains at the bottom of the Z-order so UI controls stay clickable
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

#[derive(serde::Serialize)]
struct PlaybackState {
    position: f64,
    duration: f64,
}

#[tauri::command]
fn player_get_state(state: State<AppState>) -> PlaybackState {
    if let Some(player) = state.player.lock().unwrap().as_ref() {
        PlaybackState {
            position: player.get_position(),
            duration: player.get_duration(),
        }
    } else {
        PlaybackState { position: 0.0, duration: 0.0 }
    }
}

#[tauri::command]
fn player_stop(state: State<AppState>) -> Result<(), String> {
    if let Some(player) = state.player.lock().unwrap().as_ref() {
        player.stop().map_err(|e| e.to_string())
    } else {
        Err("Player not initialized".into())
    }
}

#[tauri::command]
fn player_seek_to(seconds: f64, state: State<AppState>) -> Result<(), String> {
    if let Some(player) = state.player.lock().unwrap().as_ref() {
        player.seek_absolute(seconds).map_err(|e| e.to_string())
    } else {
        Err("Player not initialized".into())
    }
}

#[tauri::command]
fn player_set_speed(speed: f64, state: State<AppState>) -> Result<(), String> {
    if let Some(player) = state.player.lock().unwrap().as_ref() {
        player.set_speed(speed).map_err(|e| e.to_string())
    } else {
        Err("Player not initialized".into())
    }
}

#[tauri::command]
fn player_toggle_fullscreen(window: tauri::WebviewWindow) -> Result<bool, String> {
    let is_fullscreen = window.is_fullscreen().map_err(|e| e.to_string())?;
    window.set_fullscreen(!is_fullscreen).map_err(|e| e.to_string())?;
    Ok(!is_fullscreen)
}

#[tauri::command]
async fn play_youtube_id(video_id: String, resolution: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    let sidecar_path = if let Ok(exe_path) = std::env::current_exe() {
        let exe_dir = exe_path.parent().unwrap();
        let candidate = exe_dir.join("yt-dlp.exe");
        if candidate.exists() {
            candidate
        } else {
            let root_candidate = std::path::Path::new("vendor/yt-dlp.exe").to_path_buf();
            if root_candidate.exists() {
                root_candidate
            } else {
                std::path::PathBuf::from("yt-dlp.exe")
            }
        }
    } else {
        std::path::PathBuf::from("vendor/yt-dlp.exe")
    };

    let height = resolution.unwrap_or_else(|| "1080".into());
    let format_filter = format!("bestvideo[height<={height}]+bestaudio/best[height<={height}]/best");

    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let output = std::process::Command::new(&sidecar_path)
        .args([
            "--js-runtimes",
            "node",
            "-f",
            &format_filter,
            "-g",
            &url,
        ])
        .output()
        .map_err(|e| format!("Failed to execute extractor at {:?}: {e}", sidecar_path))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Extractor failed: {err}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines().filter(|l| l.starts_with("http"));
    let video_url = lines.next().ok_or("No video URL extracted")?;
    let audio_url = lines.next().unwrap_or(video_url);

    if let Some(player) = state.player.lock().unwrap().as_ref() {
        player.load_video_and_audio(video_url, audio_url).map_err(|e| e.to_string())?;
        Ok(())
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
            player_stop,
            player_seek,
            player_seek_to,
            player_get_state,
            player_set_speed,
            player_toggle_fullscreen,
            play_youtube_id
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
