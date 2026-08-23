use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use macos_accessibility_client::accessibility;
use objc2_app_kit::NSPasteboard;

const VK_C: u16 = 0x08;
const VK_OPTION: u16 = 0x3A;

/// 손쉬운 사용 권한이 있는지 확인한다. 없으면 시스템 승인 창을 띄운다.
pub fn ensure_accessibility_trust() -> bool {
    accessibility::application_is_trusted_with_prompt()
}

/// 클립보드가 바뀔 때마다 증가하는 카운터. 복사가 실제로 일어났는지 판별하는 데 쓴다.
pub fn clipboard_change_count() -> isize {
    NSPasteboard::generalPasteboard().changeCount()
}

/// 포커스된 창에 Cmd+C 를 합성해서 보낸다.
/// 우리 단축키(Cmd+Option+T)의 Option 키가 아직 눌린 채로 남아있을 수 있어
/// 먼저 Option 을 떼야 Cmd+Option+C 가 아니라 Cmd+C 로 인식된다.
pub fn copy_selection() {
    let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else {
        return;
    };

    if let Ok(option_up) = CGEvent::new_keyboard_event(source.clone(), VK_OPTION, false) {
        option_up.post(CGEventTapLocation::HID);
    }
    if let Ok(key_down) = CGEvent::new_keyboard_event(source.clone(), VK_C, true) {
        key_down.set_flags(CGEventFlags::CGEventFlagCommand);
        key_down.post(CGEventTapLocation::HID);
    }
    if let Ok(key_up) = CGEvent::new_keyboard_event(source, VK_C, false) {
        key_up.set_flags(CGEventFlags::CGEventFlagCommand);
        key_up.post(CGEventTapLocation::HID);
    }
}
