mod config;
mod double_copy;
mod log;
mod platform;
mod popup;
mod translate_api;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_autostart::{ManagerExt as AutostartManagerExt, MacosLauncher};
use tauri_plugin_clipboard_manager::ClipboardExt;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_api_key() -> Option<String> {
    config::load_api_key()
}

#[tauri::command]
fn save_api_key(key: String) -> Result<(), String> {
    config::save_api_key(&key)
}

#[tauri::command]
fn get_autostart_enabled(app: tauri::AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let result = if enabled { autolaunch.enable() } else { autolaunch.disable() };
    result.map_err(|e| e.to_string())
}

fn open_settings_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        // macOS 는 Accessory 정책이라 앱이 숨어 있어, 창을 띄우기 전에 앱부터 보이게 해야 한다.
        // AppHandle::show 는 macOS 전용 API 라 다른 타깃에서는 컴파일되지 않는다.
        #[cfg(target_os = "macos")]
        let _ = app.show();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 번역 요청마다 하나씩 늘어난다. 요청이 끝났을 때 이 값이 자기 번호와
/// 다르면(= 그 사이 새 요청이 또 들어왔으면) 결과를 버린다 — 요청을 무시하지 않고
/// 전부 처리하되, 화면에는 항상 가장 최근 것만 반영하기 위해서다.
static REQUEST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 복사 두 번/트레이 메뉴 공통 진입점. 지금 클립보드에 있는 텍스트를 번역해 팝업에 띄운다.
fn translate_clipboard(app: &tauri::AppHandle) {
    let my_id = REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let is_latest = move || REQUEST_ID.load(std::sync::atomic::Ordering::SeqCst) == my_id;

    popup::capture_anchor(app);
    popup::emit_loading(app);

    let app = app.clone();
    std::thread::spawn(move || {
        let text = match app.clipboard().read_text() {
            Ok(text) if !text.trim().is_empty() => text,
            Ok(_) => {
                if is_latest() {
                    popup::emit_error(&app, "복사한 텍스트가 없습니다");
                }
                return;
            }
            Err(err) => {
                if is_latest() {
                    popup::emit_error(&app, format!("클립보드 읽기 실패: {err}"));
                }
                return;
            }
        };

        let Some(api_key) = config::load_api_key() else {
            if is_latest() {
                popup::emit_error(&app, "DeepL API 키가 설정되지 않았습니다");
            }
            return;
        };

        let result = translate_api::translate(&text, &api_key);
        if !is_latest() {
            return;
        }
        match result {
            Ok(result) => popup::emit_translation(
                &app,
                popup::TranslationPayload {
                    direction: format!("{} → {}", result.source_lang, result.target_lang),
                    count: text.chars().count(),
                    text: result.text,
                },
            ),
            Err(err) => popup::emit_error(&app, format!("번역 실패: {err}")),
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .manage(popup::PopupState::default())
        .on_window_event(|window, event| {
            if window.label() == "settings" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            get_api_key,
            save_api_key,
            get_autostart_enabled,
            set_autostart_enabled,
            popup::popup_content_ready,
            popup::popup_save_position,
            popup::popup_hide,
            popup::popup_resize
        ])
        .setup(|app| {
            log::init();

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            double_copy::spawn_watcher(move || {
                if platform::clipboard_is_sensitive() {
                    log::write("복사 두 번 감지: 민감 표시된 내용이라 번역하지 않음");
                    return;
                }
                // 직접 누른 단축키가 아니라 추정이므로, 번역할 텍스트가 없으면 오류 팝업 없이 넘어간다.
                if !handle.clipboard().read_text().is_ok_and(|text| !text.trim().is_empty()) {
                    return;
                }
                log::write("복사 두 번 감지");
                let app = handle.clone();
                let _ = handle.run_on_main_thread(move || translate_clipboard(&app));
            });

            let translate_item = MenuItem::with_id(app, "translate", "번역", true, None::<&str>)?;
            let settings_item =
                MenuItem::with_id(app, "settings", "API 키 설정...", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &translate_item,
                    &settings_item,
                    &PredefinedMenuItem::separator(app)?,
                    &quit_item,
                ],
            )?;

            #[cfg(target_os = "macos")]
            let icon = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))?;
            #[cfg(target_os = "windows")]
            let icon = Image::from_bytes(platform::tray_icon_bytes())?;

            TrayIconBuilder::new()
                .icon(icon)
                .icon_as_template(cfg!(target_os = "macos"))
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "translate" => {
                        log::write("트레이 번역 클릭");
                        translate_clipboard(app);
                    }
                    "settings" => open_settings_window(app),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
