use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_MENU,
};

const VK_C: VIRTUAL_KEY = VIRTUAL_KEY(0x43);

/// Windows 는 입력 합성에 별도 권한이 필요하지 않다.
pub fn ensure_input_permission() -> bool {
    true
}

/// 클립보드가 바뀔 때마다 증가하는 카운터. 복사가 실제로 일어났는지 판별하는 데 쓴다.
pub fn clipboard_change_count() -> isize {
    unsafe { GetClipboardSequenceNumber() as isize }
}

fn key_event(key: VIRTUAL_KEY, pressed: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if pressed {
                    KEYBD_EVENT_FLAGS(0)
                } else {
                    KEYEVENTF_KEYUP
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// 포커스된 창에 Ctrl+C 를 합성해서 보낸다.
/// 우리 단축키(Ctrl+Alt+T)의 Alt 가 아직 눌린 채로 남아있을 수 있어
/// 먼저 Alt 를 떼야 Ctrl+Alt+C 가 아니라 Ctrl+C 로 인식된다.
pub fn copy_selection() {
    let inputs = [
        key_event(VK_MENU, false),
        key_event(VK_CONTROL, true),
        key_event(VK_C, true),
        key_event(VK_C, false),
        key_event(VK_CONTROL, false),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}
