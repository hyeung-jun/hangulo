//! 릴리스 빌드(Windows)는 콘솔이 없어 `eprintln!` 이 어디에도 남지 않는다. 단축키가
//! 재시작 전까지 먹통이 되는 문제의 원인을 가리기 위해 설정 디렉토리의 `hangulo.log` 에
//! 이벤트를 남긴다. 재시작하면 증상이 사라지므로 이전 실행 기록도 이어 붙여 보존한다.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

/// 이 크기를 넘으면 시작할 때 비운다.
const MAX_LOG_BYTES: u64 = 1024 * 1024;

fn log_path() -> std::path::PathBuf {
    crate::config::app_dir().join("hangulo.log")
}

pub fn init() {
    let path = log_path();
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        let _ = std::fs::remove_file(&path);
    }
    write("앱 시작");
}

pub fn write(message: impl AsRef<str>) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{secs} {}", message.as_ref());
    }
}
