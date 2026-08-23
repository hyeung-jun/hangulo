use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Serialize)]
struct Config {
    api_key: Option<String>,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .expect("설정 디렉토리를 찾을 수 없습니다")
        .join("hangulo")
        .join("config.json")
}

/// 설정에서 DeepL 키를 읽는다. 없거나 비어있으면 기본 파일을 만들고 None 을 돌려준다.
///
/// 서명된 .app 번들은 자기 내부에 쓸 수 없어, 실행 파일 옆이 아니라
/// `~/Library/Application Support/hangulo/config.json` 에 둔다.
pub fn load_api_key() -> Option<String> {
    let path = config_path();

    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, r#"{"api_key": ""}"#);
        return None;
    }

    let contents = std::fs::read_to_string(&path).ok()?;
    let config: Config = serde_json::from_str(&contents).ok()?;
    let key = config.api_key?;
    let key = key.trim();
    (!key.is_empty()).then(|| key.to_string())
}

/// 설정 창에서 입력한 DeepL 키를 config.json 에 저장한다.
pub fn save_api_key(key: &str) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let config = Config {
        api_key: Some(key.trim().to_string()),
    };
    let contents = serde_json::to_string(&config).map_err(|err| err.to_string())?;
    std::fs::write(&path, contents).map_err(|err| err.to_string())
}
