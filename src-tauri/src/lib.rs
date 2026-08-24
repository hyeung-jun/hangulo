mod config;
mod platform;
mod popup;
mod translate_api;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// macOS 는 Cmd+Opt+T, Windows 는 Ctrl+Alt+T. Windows 에서 SUPER 는 Win 키라 관례에 맞지 않는다.
#[cfg(target_os = "macos")]
const HOTKEY_MODIFIERS: Modifiers = Modifiers::SUPER.union(Modifiers::ALT);
#[cfg(target_os = "windows")]
const HOTKEY_MODIFIERS: Modifiers = Modifiers::CONTROL.union(Modifiers::ALT);

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

/// 단축키를 누를 때마다 하나씩 늘어난다. 요청이 끝났을 때 이 값이 자기 번호와
/// 다르면(= 그 사이 새 단축키가 또 눌렸으면) 결과를 버린다 — 요청을 무시하지 않고
/// 전부 처리하되, 화면에는 항상 가장 최근 것만 반영하기 위해서다.
static REQUEST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 복사 완료를 기다리는 폴링 간격과 최대 시도 횟수. 합성 Cmd+C 가 실제로 대상 앱에
/// 반영되기까지 걸리는 시간이 앱마다 달라서(느린 앱은 300ms 로도 부족했다) 여유를 뒀다.
const COPY_POLL_INTERVAL_MS: u64 = 20;
const COPY_POLL_ATTEMPTS: u32 = 40;

/// 단축키/메뉴 공통 진입점. 드래그해 둔 텍스트를 복사한 뒤, 실제로 클립보드가
/// 바뀌었는지 최대 800ms(20ms x 40) 동안 폴링해서 새 내용인지 기존 내용인지 판별한다.
fn on_hotkey(app: &tauri::AppHandle) {
    let my_id = REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let is_latest = move || REQUEST_ID.load(std::sync::atomic::Ordering::SeqCst) == my_id;

    popup::capture_anchor(app);
    popup::emit_loading(app);

    let before = platform::clipboard_change_count();
    platform::copy_selection();
    let app = app.clone();
    std::thread::spawn(move || {
        let mut copied = false;
        for _ in 0..COPY_POLL_ATTEMPTS {
            std::thread::sleep(std::time::Duration::from_millis(COPY_POLL_INTERVAL_MS));
            if platform::clipboard_change_count() != before {
                copied = true;
                break;
            }
        }
        if !copied {
            if is_latest() {
                popup::emit_error(&app, "선택하거나 복사한 텍스트가 없습니다");
            }
            return;
        }
        let text = match app.clipboard().read_text() {
            Ok(text) if !text.trim().is_empty() => text,
            Ok(_) => {
                if is_latest() {
                    popup::emit_error(&app, "선택하거나 복사한 텍스트가 없습니다");
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
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        on_hotkey(app);
                    }
                })
                .build(),
        )
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
            popup::popup_content_ready,
            popup::popup_save_position,
            popup::popup_hide,
            popup::popup_resize
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            if !platform::ensure_input_permission() {
                eprintln!("손쉬운 사용 권한이 없어 선택한 텍스트를 복사하지 못합니다.");
            }

            let hotkey = Shortcut::new(Some(HOTKEY_MODIFIERS), Code::KeyT);
            app.global_shortcut().register(hotkey)?;

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
            let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))?;

            TrayIconBuilder::new()
                .icon(icon)
                .icon_as_template(cfg!(target_os = "macos"))
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "translate" => on_hotkey(app),
                    "settings" => open_settings_window(app),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
