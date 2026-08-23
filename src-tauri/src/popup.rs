use std::sync::Mutex;
use tauri::{Emitter, Manager};

const POPUP_LABEL: &str = "popup";
/// 커서 근처에 띄울 때 겹치지 않도록 살짝 띄우는 간격 (논리 픽셀).
const ANCHOR_OFFSET: f64 = 12.0;

#[derive(Default)]
pub struct PopupState {
    /// 사용자가 드래그로 옮긴 위치. 앱이 떠 있는 동안 유지되고, 있으면 앵커보다 우선한다.
    user_position: Mutex<Option<(f64, f64)>>,
    /// 단축키를 누른 순간의 커서 위치. 다음 번역 결과를 여기 기준으로 배치한다.
    anchor: Mutex<(f64, f64)>,
}

#[derive(Clone, serde::Serialize)]
pub struct TranslationPayload {
    pub direction: String,
    pub count: usize,
    pub text: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ErrorPayload {
    pub message: String,
}

/// 단축키를 누른 시점의 커서 위치를 앵커로 저장한다. 복사 대기 + DeepL 왕복 동안
/// 마우스가 움직여도 결과는 이 좌표 기준으로 뜬다.
///
/// 저장된 위치(user_position)도 여기서 함께 지운다. 창을 옮긴 뒤에도 그 자리를
/// 계속 기억하면, 나중에 다른 모니터·다른 위치에서 번역할 때 엉뚱한 곳에 뜨는
/// 것처럼 느껴져서 매 번역마다 커서 기준으로 다시 잡기로 했다.
pub fn capture_anchor(app: &tauri::AppHandle) {
    let state = app.state::<PopupState>();
    *state.user_position.lock().unwrap() = None;
    if let Ok(pos) = app.cursor_position() {
        *state.anchor.lock().unwrap() = (pos.x, pos.y);
    }
}

pub fn emit_translation(app: &tauri::AppHandle, payload: TranslationPayload) {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        let _ = window.emit("translation", payload);
    }
}

pub fn emit_error(app: &tauri::AppHandle, message: impl Into<String>) {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        let _ = window.emit("popup-error", ErrorPayload { message: message.into() });
    }
}

/// 번역 요청을 시작하자마자(응답을 기다리지 않고) 프런트엔드에 로딩 상태를 알린다.
/// 실제 표시는 여전히 프런트엔드가 `popup_content_ready`를 호출해야 이뤄진다.
pub fn emit_loading(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        let _ = window.emit("loading", ());
    }
}

/// JS 가 콘텐츠 크기를 실측해 알려주면, 앵커/저장된 위치와 모니터 작업 영역을 기준으로
/// 최종 위치를 계산해 창을 그 크기·자리로 맞추고 그제서야 보여준다.
#[tauri::command]
pub fn popup_content_ready(
    app: tauri::AppHandle,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window(POPUP_LABEL) else {
        return Err("popup window not found".into());
    };

    let state = app.state::<PopupState>();
    let saved_position = *state.user_position.lock().unwrap();

    let (x, y) = if let Some(pos) = saved_position {
        pos
    } else {
        let anchor = *state.anchor.lock().unwrap();
        place_near_anchor(&app, anchor, width, height)
    };

    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;

    Ok(())
}

/// 커서가 놓인 모니터를 찾는다. `monitor_from_point` 를 그대로 못 쓰는 이유는, 이
/// 사용자의 보조 모니터처럼 주 모니터 왼쪽에 음수 좌표로 배치된 경우 그 함수가
/// 종종 `None` 을 돌려주기 때문이다(tao 쪽 히트테스트 버그로 보인다). 대신
/// `available_monitors()` 목록을 직접 훑어 좌표가 그 모니터의 물리 사각형 안에
/// 있는지 우리가 판정한다.
fn find_monitor(app: &tauri::AppHandle, x: f64, y: f64) -> Option<tauri::Monitor> {
    let monitors = app.available_monitors().ok()?;
    monitors.into_iter().find(|m| {
        let left = m.position().x as f64;
        let top = m.position().y as f64;
        let right = left + m.size().width as f64;
        let bottom = top + m.size().height as f64;
        x >= left && x < right && y >= top && y < bottom
    })
}

fn place_near_anchor(app: &tauri::AppHandle, anchor: (f64, f64), width: f64, height: f64) -> (f64, f64) {
    // cursor_position() 은 물리 픽셀 좌표를 쓰지만, width/height 와 최종 set_position 은
    // 논리 픽셀 기준이다. 물리 좌표를 그대로 섞어 쓰면 Retina(2x) 등 배율이 있는
    // 모니터에서 커서 위치가 실제보다 배율만큼 어긋난 곳에 창이 뜬다.
    let (anchor_x_physical, anchor_y_physical) = anchor;

    let Some(monitor) = find_monitor(app, anchor_x_physical, anchor_y_physical)
        .or_else(|| app.primary_monitor().ok().flatten())
    else {
        return (anchor_x_physical + ANCHOR_OFFSET, anchor_y_physical + ANCHOR_OFFSET);
    };

    let scale = monitor.scale_factor();
    let anchor_x = anchor_x_physical / scale;
    let anchor_y = anchor_y_physical / scale;
    let work_left = monitor.work_area().position.x as f64 / scale;
    let work_top = monitor.work_area().position.y as f64 / scale;
    let work_right = work_left + monitor.work_area().size.width as f64 / scale;
    let work_bottom = work_top + monitor.work_area().size.height as f64 / scale;

    let mut x = anchor_x + ANCHOR_OFFSET;
    let mut y = anchor_y + ANCHOR_OFFSET;

    // 아래로 띄우면 화면을 벗어나면 커서 위로 뒤집는다.
    if y + height > work_bottom {
        y = anchor_y - height - ANCHOR_OFFSET;
    }

    x = x.clamp(work_left, (work_right - width).max(work_left));
    y = y.clamp(work_top, (work_bottom - height).max(work_top));

    (x, y)
}

/// 헤더 드래그(startDragging)나 그립 리사이즈(startResizeDragging)가 끝난 뒤 호출된다.
/// 그 뒤로는 다음 번역도 이 위치를 그대로 쓴다. 크기는 매번 콘텐츠에 맞춰 새로 잡으므로 저장하지 않는다.
#[tauri::command]
pub fn popup_save_position(window: tauri::WebviewWindow, state: tauri::State<PopupState>) -> Result<(), String> {
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    *state.user_position.lock().unwrap() = Some((pos.x as f64 / scale, pos.y as f64 / scale));
    Ok(())
}

#[tauri::command]
pub fn popup_hide(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

/// 그립을 드래그하는 동안 실시간으로 창 크기를 바꾼다.
/// macOS 의 tao 백엔드가 아직 `startResizeDragging` 을 구현하지 않아
/// (항상 `NotSupported` 를 반환) OS 네이티브 리사이즈 대신 마우스 이동만큼
/// 직접 `set_size` 를 호출하는 방식으로 구현했다.
#[tauri::command]
pub fn popup_resize(window: tauri::WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| e.to_string())
}
