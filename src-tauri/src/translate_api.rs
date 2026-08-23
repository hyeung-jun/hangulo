use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct TranslateResponse {
    translations: Vec<Translation>,
}

#[derive(Deserialize)]
struct Translation {
    text: String,
    detected_source_language: String,
}

pub struct TranslationResult {
    pub text: String,
    pub source_lang: String,
    pub target_lang: String,
}

fn contains_hangul(text: &str) -> bool {
    text.chars().any(|c| ('가'..='힣').contains(&c))
}

/// 매 호출마다 클라이언트를 새로 만들면 TCP/TLS 핸드셰이크를 반복하게 되므로,
/// 커넥션 풀링이 재사용되도록 프로세스 전체에서 하나만 유지한다.
fn client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::blocking::Client::new)
}

/// 한글이 섞여 있으면 영어로, 아니면 한국어로 번역한다.
pub fn translate(text: &str, api_key: &str) -> Result<TranslationResult, String> {
    let host = if api_key.ends_with(":fx") {
        "api-free.deepl.com"
    } else {
        "api.deepl.com"
    };
    let target_lang = if contains_hangul(text) { "EN-US" } else { "KO" };

    let response = client()
        .post(format!("https://{host}/v2/translate"))
        .header("Authorization", format!("DeepL-Auth-Key {api_key}"))
        .json(&serde_json::json!({ "text": [text], "target_lang": target_lang }))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        return Err(format!("DeepL 요청 실패: {}", response.status()));
    }

    let mut body: TranslateResponse = response.json().map_err(|err| err.to_string())?;
    let translation = body.translations.pop().ok_or("번역 결과가 비어있습니다")?;

    Ok(TranslationResult {
        text: translation.text,
        source_lang: translation.detected_source_language,
        target_lang: target_lang.split('-').next().unwrap_or(target_lang).to_string(),
    })
}
