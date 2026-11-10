//! Dioxus 서버 함수 (생성 API).
//!
//! `#[post]` 매크로는 서버 빌드에서는 axum 핸들러를, 브라우저(wasm) 빌드에서는
//! 같은 경로로 요청을 보내는 클라이언트 함수를 만든다. 생성 외의 API(수정·삭제·만료)는
//! 존재하지 않는다. 변경 요청 메서드에 대한 명시적 거부는 `server.rs` 미들웨어가 담당한다.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

/// 생성 API 경로. 미들웨어의 변경 거부 규칙과 공유한다.
#[cfg_attr(not(feature = "server"), allow(dead_code))]
pub const CREATE_PATH: &str = "/api/short-urls";

/// 발급 결과.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortUrl {
    /// 발급된 번호. 단축 경로는 `/{id}`.
    pub id: i64,
    /// 서버가 검증·정규화해 저장한 원본 URL.
    pub original_url: String,
}

/// 원본 URL을 받아 새 번호를 발급한다. 로그인 없이 누구나 호출할 수 있다.
///
/// 같은 URL을 다시 넣어도 항상 **새 번호**를 발급한다(README 참고).
/// 기존 행을 찾아 갱신하는 경로가 없으므로 기존 연결을 덮어쓸 방법이 없다.
#[post("/api/short-urls")]
pub async fn create_short_url(url: String) -> Result<ShortUrl, HttpError> {
    // 1) 서버에서 다시 검증한다. 브라우저 검증은 안내용일 뿐 신뢰하지 않는다.
    let original_url = match crate::validate::normalize_original_url(&url) {
        Ok(u) => u,
        Err(message) => return HttpError::bad_request(message),
    };

    // 2) INSERT 만 수행한다. 번호는 DB IDENTITY 시퀀스가 원자적으로 발급한다.
    match crate::server::insert_short_url(&original_url).await {
        Ok(id) => Ok(ShortUrl { id, original_url }),
        Err(err) => {
            eprintln!("[create_short_url] DB 오류: {err:#}");
            HttpError::internal_server_error("단축 주소를 저장하지 못했습니다. 잠시 후 다시 시도해 주세요.")
        }
    }
}
