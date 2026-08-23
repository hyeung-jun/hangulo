mod config;
mod platform;
mod popup;
mod translate_api;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
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

    #[cfg(target_os = "macos")]
    {
        let before = platform::macos::clipboard_change_count();
        platform::macos::copy_selection();
        let app = app.clone();
        std::thread::spawn(move || {
            for _ in 0..COPY_POLL_ATTEMPTS {
                std::thread::sleep(std::time::Duration::from_millis(COPY_POLL_INTERVAL_MS));
                if platform::macos::clipboard_change_count() != before {
                    break;
                }
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
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
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
        .invoke_handler(tauri::generate_handler![
            greet,
            popup::popup_content_ready,
            popup::popup_save_position,
            popup::popup_hide,
            popup::popup_resize
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            if !platform::macos::ensure_accessibility_trust() {
                eprintln!("손쉬운 사용 권한이 없어 선택한 텍스트를 복사하지 못합니다.");
            }

            let hotkey = Shortcut::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::KeyT);
            app.global_shortcut().register(hotkey)?;

            let translate_item = MenuItem::with_id(app, "translate", "번역", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &translate_item,
                    &PredefinedMenuItem::separator(app)?,
                    &quit_item,
                ],
            )?;

            let icon = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))?;

            TrayIconBuilder::new()
                .icon(icon)
                .icon_as_template(true)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "translate" => on_hotkey(app),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
