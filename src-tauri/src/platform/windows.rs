use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, HWND};
use windows::Win32::System::DataExchange::{
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, RegisterClipboardFormatW,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongW, GWL_EXSTYLE, WS_EX_TOPMOST};

/// 클립보드가 바뀔 때마다 증가하는 카운터. 복사가 일어났는지 판별하는 데 쓴다.
pub fn clipboard_change_count() -> isize {
    unsafe { GetClipboardSequenceNumber() as isize }
}

/// 복사 단축키의 수식키(Ctrl)가 지금 눌려 있는지 확인한다. 터미널의 드래그 복사처럼 키 없이
/// 클립보드를 연달아 고쳐 쓰는 동작을 복사 두 번으로 오인하지 않기 위해 쓴다.
pub fn copy_modifier_held() -> bool {
    unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) < 0 }
}

/// 지금 클립보드 내용이 외부로 보내면 안 되는 것으로 표시돼 있는지 확인한다.
/// 비밀번호 관리자 등은 클립보드 감시 프로그램이 내용을 건드리지 않도록 이 포맷을 함께 넣는다.
pub fn clipboard_is_sensitive() -> bool {
    unsafe {
        let format = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        IsClipboardFormatAvailable(format).is_ok()
    }
}

/// 창이 항상-위(WS_EX_TOPMOST) 상태인지 확인한다. tauri 가 쓰는 windows 크레이트와 버전이
/// 달라 HWND 타입을 그대로 받을 수 없어 원시 포인터로 받는다.
pub fn is_topmost(hwnd: *mut core::ffi::c_void) -> bool {
    let ex_style = unsafe { GetWindowLongW(HWND(hwnd), GWL_EXSTYLE) } as u32;
    ex_style & WS_EX_TOPMOST.0 != 0
}

/// 작업표시줄(트레이 포함)이 라이트 테마인지 확인한다.
/// 레지스트리 값을 못 읽으면 다크 테마로 간주한다.
fn system_uses_light_theme() -> bool {
    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value: Vec<u16> = "SystemUsesLightTheme"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;

    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };

    result == ERROR_SUCCESS && data != 0
}

/// 트레이 아이콘은 macOS 의 템플릿 아이콘과 달리 시스템이 자동으로 색을 바꿔주지 않아,
/// 작업표시줄 테마에 맞는 버전을 직접 골라야 한다. 테마는 트레이 아이콘을 만드는 시점에만
/// 확인하며, 앱이 떠 있는 동안 테마가 바뀌어도 다시 반영되지는 않는다.
pub fn tray_icon_bytes() -> &'static [u8] {
    if system_uses_light_theme() {
        include_bytes!("../../icons/tray-icon-windows-light.png")
    } else {
        include_bytes!("../../icons/tray-icon-windows-dark.png")
    }
}
