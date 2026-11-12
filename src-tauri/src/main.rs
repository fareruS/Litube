#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use player::VideoPlayer;
use std::sync::{Arc, Mutex};
use store::{HistoryItem, Store};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, State};
use yt_model::{QualityPref, SearchQuery, VideoCard, VideoId};
use yt_source::{StreamSource, YtDlpSource};

#[cfg(windows)]
fn trim_process_working_set() {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
    unsafe {
        let process = GetCurrentProcess();
        SetProcessWorkingSetSize(process, usize::MAX, usize::MAX);
    }
}

#[cfg(not(windows))]
fn trim_process_working_set() {}

static IS_PIP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PREV_SIZE: Mutex<Option<(f64, f64)>> = Mutex::new(None);

#[cfg(windows)]
mod win32_child {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_SHOW,
        WS_CHILD, WS_CLIPSIBLINGS, WS_DISABLED, WS_VISIBLE,
    };

    pub unsafe fn create_child_hwnd(parent: HWND, x: i32, y: i32, w: i32, h: i32) -> HWND {
        let class_name: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // WS_DISABLED: disables input routing to this native child window so all mouse
        // clicks and keyboard events pass directly to WebView2 controls, while DirectX D3D11
        // hardware frames continue rendering unobstructed at full performance.
        let child = CreateWindowExW(
            0,
            class_name.as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_DISABLED,
            x,
            y,
            w,
            h,
            parent,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        // Bring child window to HWND_TOP so it is visible in front of WebView2's surface
        SetWindowPos(
            child,
            0 as HWND, // HWND_TOP
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );

        ShowWindow(child, SW_SHOW);
        child
    }

    pub unsafe fn show_child_hwnd(child: HWND, show: bool) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOW};
        ShowWindow(child, if show { SW_SHOW } else { SW_HIDE });
    }

    pub unsafe fn resize_child_hwnd(child: HWND, x: i32, y: i32, w: i32, h: i32) {
        SetWindowPos(
            child,
            0 as HWND, // HWND_TOP
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
    source: Arc<dyn StreamSource>,
    store: Arc<Store>,
    #[cfg(windows)]
    child_hwnd: Mutex<Option<isize>>,
}

#[tauri::command]
fn sync_video_rect(x: i32, y: i32, width: i32, height: i32, state: State<AppState>) {
    #[cfg(windows)]
    {
        if let Some(hwnd) = *state.child_hwnd.lock().unwrap() {
            unsafe {
                if width > 0 && height > 0 {
                    win32_child::show_child_hwnd(
                        hwnd as windows_sys::Win32::Foundation::HWND,
                        true,
                    );
                    win32_child::resize_child_hwnd(
                        hwnd as windows_sys::Win32::Foundation::HWND,
                        x,
                        y,
                        width,
                        height,
                    );
                } else {
                    win32_child::show_child_hwnd(
                        hwnd as windows_sys::Win32::Foundation::HWND,
                        false,
                    );
                }
            }
        }
    }
}

#[tauri::command]
fn player_set_visible(visible: bool, state: State<'_, AppState>) {
    #[cfg(windows)]
    {
        if let Some(hwnd) = *state.child_hwnd.lock().unwrap() {
            unsafe {
                win32_child::show_child_hwnd(hwnd as windows_sys::Win32::Foundation::HWND, visible);
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
    paused: bool,
    buffering: bool,
}

#[tauri::command]
fn player_get_state(state: State<AppState>) -> PlaybackState {
    if let Some(player) = state.player.lock().unwrap().as_ref() {
        PlaybackState {
            position: player.get_position(),
            duration: player.get_duration(),
            paused: player.is_paused(),
            buffering: player.is_buffering(),
        }
    } else {
        PlaybackState {
            position: 0.0,
            duration: 0.0,
            paused: false,
            buffering: false,
        }
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
fn player_toggle_audio_only(state: State<AppState>) -> Result<bool, String> {
    if let Some(player) = state.player.lock().unwrap().as_ref() {
        let current = player.is_audio_only();
        let next = !current;
        player.set_audio_only(next).map_err(|e| e.to_string())?;
        Ok(next)
    } else {
        Err("Player not initialized".into())
    }
}

#[tauri::command]
fn player_toggle_fullscreen(window: tauri::WebviewWindow) -> Result<bool, String> {
    let is_fullscreen = window.is_fullscreen().map_err(|e| e.to_string())?;
    window
        .set_fullscreen(!is_fullscreen)
        .map_err(|e| e.to_string())?;
    Ok(!is_fullscreen)
}

#[tauri::command]
fn player_set_volume(volume: f64, state: State<AppState>) -> Result<(), String> {
    if let Some(player) = state.player.lock().unwrap().as_ref() {
        player.set_volume(volume).map_err(|e| e.to_string())
    } else {
        Err("Player not initialized".into())
    }
}

#[tauri::command]
fn player_get_volume(state: State<AppState>) -> f64 {
    if let Some(player) = state.player.lock().unwrap().as_ref() {
        player.get_volume()
    } else {
        100.0
    }
}

#[tauri::command]
async fn search_youtube(
    query: String,
    page_token: Option<String>,
    state: State<'_, AppState>,
) -> Result<yt_model::Page<VideoCard>, String> {
    let q = SearchQuery {
        term: query,
        page_token,
    };
    state.source.search(&q).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn youtube_recommendations(
    page_token: Option<String>,
    state: State<'_, AppState>,
) -> Result<yt_model::Page<VideoCard>, String> {
    state
        .source
        .recommendations(page_token)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn play_youtube_id(
    video_id: String,
    resolution: Option<String>,
    start_time: Option<f64>,
    state: State<'_, AppState>,
) -> Result<Option<u32>, String> {
    let clean_id = video_id.trim();
    if clean_id.is_empty()
        || clean_id.len() > 64
        || !clean_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Invalid YouTube video ID format".into());
    }

    let max_height = match resolution.as_deref() {
        Some("auto") | None => 1080,
        Some(val) => val.parse::<u32>().unwrap_or(480),
    };

    let pref = QualityPref {
        max_height: Some(max_height),
        audio_only: false,
    };

    // If switching to a new video (not a live resolution switch), stop existing playback immediately
    if start_time.map_or(true, |t| t >= 0.0) {
        if let Some(player) = state.player.lock().unwrap().as_ref() {
            let _ = player.stop();
        }
    }

    let media = state
        .source
        .playable(&VideoId(clean_id.to_string()), pref)
        .await
        .map_err(|e| e.to_string())?;

    let video_height = media.video_height;
    let video_url = media.video_url.as_deref().unwrap_or(&media.audio_url);
    let audio_url = &media.audio_url;

    if let Some(player) = state.player.lock().unwrap().as_ref() {
        // A negative start_time (e.g. -1.0) indicates a live resolution switch.
        // Sample player.get_position() NOW at extraction completion, eliminating lag rewind.
        let resolved_start = match start_time {
            Some(t) if t < 0.0 => {
                let current = player.get_position();
                if current > 0.0 {
                    Some(current)
                } else {
                    None
                }
            }
            other => other,
        };
        player
            .load_video_and_audio(video_url, audio_url, resolved_start)
            .map_err(|e| e.to_string())?;
        Ok(video_height)
    } else {
        Err("Player not initialized".into())
    }
}

#[tauri::command]
fn store_record_history(
    video_id: String,
    title: String,
    channel_title: String,
    thumbnail_url: String,
    duration_seconds: Option<u64>,
    resume_pos: f64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .record_history(
            &video_id,
            &title,
            &channel_title,
            &thumbnail_url,
            duration_seconds,
            resume_pos,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn store_update_resume_pos(
    video_id: String,
    resume_pos: f64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .update_resume_position(&video_id, resume_pos)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn store_get_history(
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryItem>, String> {
    state
        .store
        .get_history(limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn store_set_setting(key: String, value: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .store
        .set_setting(&key, &value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn store_get_setting(key: String, state: State<'_, AppState>) -> Result<Option<String>, String> {
    state.store.get_setting(&key).map_err(|e| e.to_string())
}

#[tauri::command]
fn store_add_subscription(
    channel_id: String,
    title: String,
    avatar_url: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .add_subscription(&channel_id, &title, &avatar_url)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn store_remove_subscription(channel_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .store
        .remove_subscription(&channel_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn store_get_subscriptions(
    state: State<'_, AppState>,
) -> Result<Vec<store::SubscriptionItem>, String> {
    state.store.get_subscriptions().map_err(|e| e.to_string())
}

#[tauri::command]
fn store_is_subscribed(channel_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    state
        .store
        .is_subscribed(&channel_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn player_toggle_pip(window: tauri::WebviewWindow) -> Result<bool, String> {
    use std::sync::atomic::Ordering;
    let current = IS_PIP.load(Ordering::SeqCst);
    let next = !current;

    if next {
        if let Ok(true) = window.is_maximized() {
            let _ = window.unmaximize();
        }
        if let Ok(size) = window.inner_size() {
            *PREV_SIZE.lock().unwrap() = Some((size.width as f64, size.height as f64));
        }
        let _ = window.set_always_on_top(true);
        let _ = window.set_decorations(false); // Frameless: no window titlebar in PiP
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: 420.0,
            height: 236.0,
        }));
    } else {
        let _ = window.set_always_on_top(false);
        let _ = window.set_decorations(true); // Restore window decorations
        if let Some((w, h)) = *PREV_SIZE.lock().unwrap() {
            let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: w as u32,
                height: h as u32,
            }));
        } else {
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: 1000.0,
                height: 600.0,
            }));
        }
    }

    IS_PIP.store(next, Ordering::SeqCst);
    Ok(next)
}

fn get_cookies_path() -> std::path::PathBuf {
    std::env::var("APPDATA")
        .map(|appdata| {
            std::path::PathBuf::from(appdata)
                .join("Litube")
                .join("cookies.txt")
        })
        .unwrap_or_else(|_| std::path::PathBuf::from("cookies.txt"))
}

#[derive(serde::Serialize)]
struct AuthStatus {
    signed_in: bool,
    path: String,
}

#[tauri::command]
fn auth_get_status() -> AuthStatus {
    let path = get_cookies_path();
    let signed_in = path.exists()
        && std::fs::metadata(&path)
            .map(|m| m.len() > 10)
            .unwrap_or(false);
    AuthStatus {
        signed_in,
        path: path.to_string_lossy().into_owned(),
    }
}

#[tauri::command]
fn auth_import_cookies(content: String) -> Result<bool, String> {
    let clean = content.trim();
    if clean.is_empty() {
        return Err("Cookies content cannot be empty".into());
    }
    let path = get_cookies_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, clean).map_err(|e| format!("Failed to save cookies: {e}"))?;
    Ok(true)
}

struct CookieData {
    domain: String,
    path: String,
    secure: bool,
    name: String,
    value: String,
}

fn export_cookies_to_netscape(cookies: &[CookieData]) -> String {
    let mut out = String::from("# Netscape HTTP Cookie File\n# http://curl.haxx.se/rfc/cookie_spec.html\n# Generated by Litube\n\n");
    for c in cookies {
        let trimmed = c.domain.trim_start_matches('.');
        if !trimmed.ends_with("youtube.com") && !trimmed.ends_with("google.com") {
            continue;
        }
        let domain_str = if c.domain.starts_with('.') {
            c.domain.clone()
        } else {
            format!(".{}", c.domain)
        };
        let secure_str = if c.secure { "TRUE" } else { "FALSE" };
        out.push_str(&format!(
            "{}\tTRUE\t{}\t{}\t2147483647\t{}\t{}\n",
            domain_str, c.path, secure_str, c.name, c.value
        ));
    }
    out
}

async fn read_login_cookies(app: &tauri::AppHandle, label: &'static str) -> Vec<CookieData> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let app2 = app.clone();
    if app
        .run_on_main_thread(move || {
            let cookies = if let Some(wv) = app2.get_webview_window(label) {
                wv.cookies()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|c| CookieData {
                        domain: c.domain().unwrap_or("").to_string(),
                        path: c.path().unwrap_or("/").to_string(),
                        secure: c.secure().unwrap_or(false),
                        name: c.name().to_string(),
                        value: c.value().to_string(),
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let _ = tx.send(cookies);
        })
        .is_err()
    {
        return Vec::new();
    }
    rx.await.unwrap_or_default()
}

fn close_login_window(app: &tauri::AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = app2.get_webview_window("google_login") {
            let _ = w.destroy();
        }
    });
}

#[tauri::command]
fn auth_sign_out(app: tauri::AppHandle) -> Result<bool, String> {
    let path = get_cookies_path();
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    if let Some(wv) = app.get_webview_window("main") {
        if let Ok(cookies) = wv.cookies() {
            for c in cookies {
                let domain = c.domain().unwrap_or_default().trim_start_matches('.');
                if domain.ends_with("youtube.com") || domain.ends_with("google.com") {
                    let _ = wv.delete_cookie(c);
                }
            }
        }
    }
    use tauri::Emitter;
    let _ = app.emit("auth-changed", false);
    Ok(false)
}

#[tauri::command]
async fn auth_open_login_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("google_login") {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();

    let app_watcher = app.clone();
    tauri::async_runtime::spawn(async move {
        while rx.recv().await.is_some() {
            for _ in 0..8 {
                tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                let cookies = read_login_cookies(&app_watcher, "google_login").await;
                let is_logged_in = cookies.iter().any(|c| {
                    (c.domain.ends_with("youtube.com") || c.domain.ends_with("google.com"))
                        && (c.name == "SAPISID" || c.name == "SID" || c.name == "LOGIN_INFO")
                });
                if is_logged_in {
                    let netscape = export_cookies_to_netscape(&cookies);
                    let path = get_cookies_path();
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(_) = std::fs::write(&path, netscape) {
                        use tauri::Emitter;
                        let _ = app_watcher.emit("auth-changed", true);
                    }
                    close_login_window(&app_watcher);
                    return;
                }
            }
        }
    });

    let url = tauri::Url::parse("https://accounts.google.com/ServiceLogin?service=youtube&continue=https://www.youtube.com/")
        .map_err(|e| e.to_string())?;

    let builder =
        tauri::WebviewWindowBuilder::new(&app, "google_login", tauri::WebviewUrl::External(url))
            .title("Sign in to YouTube - Google Accounts")
            .inner_size(540.0, 720.0)
            .center()
            .on_page_load(move |_w, payload| {
                if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                    if let Some(host) = payload.url().host_str() {
                        if host == "www.youtube.com"
                            || host == "youtube.com"
                            || host == "m.youtube.com"
                        {
                            let _ = tx.send(());
                        }
                    }
                }
            });

    let win = builder.build().map_err(|e| e.to_string())?;
    let _ = win.show();
    Ok(())
}

fn main() {
    // Aggressive WebView2 memory optimization flags:
    // 1. Single renderer process limit (prevents multiple duplicate helper processes).
    // 2. V8 JS heap capped to 64 MB (our static single-page UI needs < 10 MB).
    // 3. Disables unused browser subsystems (Translate, Sync, Extensions, Speech, Background Networking).
    std::env::set_var(
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        "--renderer-process-limit=1 --js-flags=--max-old-space-size=64 --process-per-site --disable-features=Translate,AutofillServerCommunication,OptimizationHints,MediaRouter,DialMediaRouteProvider --disable-extensions --disable-component-update --disable-background-networking --disable-domain-reliability --disable-sync --disable-speech-api"
    );

    let source = Arc::new(YtDlpSource::auto_detect());

    let db_path = std::env::var("APPDATA")
        .map(|appdata| std::path::PathBuf::from(appdata).join("Litube"))
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let _ = std::fs::create_dir_all(&db_path);
    let db_file = db_path.join("litube.db");
    let store = Arc::new(Store::open(&db_file).unwrap_or_else(|e| {
        eprintln!(
            "[Litube] Failed to open SQLite DB at {:?}: {e}, falling back to memory",
            db_file
        );
        Store::open(":memory:").expect("in-memory db")
    }));

    tauri::Builder::default()
        .manage(AppState {
            player: Mutex::new(None),
            source,
            store,
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
            player_set_volume,
            player_get_volume,
            player_toggle_audio_only,
            player_toggle_fullscreen,
            play_youtube_id,
            search_youtube,
            youtube_recommendations,
            store_record_history,
            store_update_resume_pos,
            store_get_history,
            store_set_setting,
            store_get_setting,
            player_toggle_pip,
            player_set_visible,
            store_add_subscription,
            store_remove_subscription,
            store_get_subscriptions,
            store_is_subscribed,
            auth_get_status,
            auth_import_cookies,
            auth_sign_out,
            auth_open_login_window
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<AppState>();
                let close_to_tray = state
                    .store
                    .get_setting("close_to_tray")
                    .ok()
                    .flatten()
                    .map(|v| v == "true")
                    .unwrap_or(false);

                if close_to_tray {
                    #[cfg(windows)]
                    {
                        if let Some(hwnd) = *state.child_hwnd.lock().unwrap() {
                            unsafe {
                                win32_child::show_child_hwnd(
                                    hwnd as windows_sys::Win32::Foundation::HWND,
                                    false,
                                );
                            }
                        }
                    }
                    let _ = window.hide();
                    api.prevent_close();
                    trim_process_working_set();
                } else {
                    let guard = state.player.lock().unwrap();
                    if let Some(player) = guard.as_ref() {
                        let _ = player.stop();
                    }
                    app.exit(0);
                }
            }
        })
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            let _ = window.show();
            let _ = window.set_focus();

            // Set up native System Tray
            let show_i = MenuItem::with_id(app, "show", "Show Litube", true, None::<&str>)?;
            let play_i = MenuItem::with_id(app, "play_pause", "Play / Pause", true, None::<&str>)?;
            let mute_i = MenuItem::with_id(app, "mute", "Mute / Unmute", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit Litube", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &play_i, &mute_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "play_pause" => {
                        let state = app.state::<AppState>();
                        let guard = state.player.lock().unwrap();
                        if let Some(player) = guard.as_ref() {
                            let _ = player.toggle_pause();
                        }
                    }
                    "mute" => {
                        let state = app.state::<AppState>();
                        let guard = state.player.lock().unwrap();
                        if let Some(player) = guard.as_ref() {
                            let cur = player.get_volume();
                            let _ = player.set_volume(if cur > 0.0 { 0.0 } else { 100.0 });
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                                trim_process_working_set();
                            } else {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

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
                            println!(
                                "[Litube Spike] libmpv successfully anchored to HWND: {:?}",
                                child
                            );
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
