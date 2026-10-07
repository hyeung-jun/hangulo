use objc2::rc::autoreleasepool;
use objc2_app_kit::NSPasteboard;

/// 클립보드가 바뀔 때마다 증가하는 카운터. 복사가 일어났는지 판별하는 데 쓴다.
/// 폴링 스레드에서 계속 불리므로, autorelease 된 객체가 쌓이지 않게 풀로 감싼다.
pub fn clipboard_change_count() -> isize {
    autoreleasepool(|_| NSPasteboard::generalPasteboard().changeCount())
}
