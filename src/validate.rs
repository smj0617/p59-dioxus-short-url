//! 원본 URL 검증과 숫자 경로 파싱.
//!
//! 브라우저(빠른 안내)와 서버(최종 판단) 양쪽에서 같은 규칙을 쓰지만,
//! 저장 여부는 항상 서버의 [`normalize_original_url`] 결과로만 결정한다.

use url::Url;

/// 저장할 수 있는 원본 URL의 최대 길이.
pub const MAX_URL_LEN: usize = 2048;

/// 입력을 검증하고 저장할 형태로 정규화한다.
///
/// - 앞뒤 공백 제거
/// - `http://` 또는 `https://` 스킴만 허용
/// - 호스트가 있어야 함 (`http://` 만 있는 값 거부)
/// - 사용자 정보(`user:pass@`)가 들어간 URL 거부
/// - 경로·쿼리·프래그먼트(#...)는 그대로 보존 (`Url` 직렬화는 이를 유지한다)
pub fn normalize_original_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("URL을 입력해 주세요.".into());
    }
    if trimmed.len() > MAX_URL_LEN {
        return Err(format!("URL은 {MAX_URL_LEN}자 이하만 등록할 수 있습니다."));
    }

    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("http:// 또는 https:// 로 시작하는 URL만 등록할 수 있습니다.".into());
    }

    let url = Url::parse(trimmed).map_err(|_| "올바른 URL 형식이 아닙니다.".to_string())?;

    if url.scheme() != "http" && url.scheme() != "https" {
        return Err("http:// 또는 https:// URL만 등록할 수 있습니다.".into());
    }
    match url.host_str() {
        Some(host) if !host.is_empty() => {}
        _ => return Err("URL에 호스트(도메인)가 없습니다.".into()),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("아이디·비밀번호가 포함된 URL은 등록할 수 없습니다.".into());
    }

    let normalized = url.to_string();
    if normalized.len() > MAX_URL_LEN {
        return Err(format!("URL은 {MAX_URL_LEN}자 이하만 등록할 수 있습니다."));
    }
    Ok(normalized)
}

/// 단축 경로 판별 결과.
#[cfg_attr(not(any(feature = "server", test)), allow(dead_code))]
#[derive(Debug, PartialEq, Eq)]
pub enum NumericPath {
    /// `/123` 처럼 숫자만으로 된 경로이고 유효한 번호다.
    Id(i64),
    /// 숫자만으로 된 경로지만 발급될 수 없는 번호다(`/0`, `/007`, 범위 초과 등) → 404.
    Invalid,
}

#[cfg_attr(not(any(feature = "server", test)), allow(dead_code))]
/// 경로가 `/숫자` 형태인지 판별한다. 숫자 경로가 아니면 `None`.
pub fn parse_numeric_path(path: &str) -> Option<NumericPath> {
    let digits = path.strip_prefix('/')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // 한 번호에는 한 경로만 대응시킨다: 앞자리 0 (/007) 이나 0 은 발급하지 않는 형태.
    if digits.starts_with('0') {
        return Some(NumericPath::Invalid);
    }
    match digits.parse::<i64>() {
        Ok(id) if id > 0 => Some(NumericPath::Id(id)),
        _ => Some(NumericPath::Invalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_and_keeps_path_query_fragment() {
        let u = normalize_original_url("  https://example.org/a/b?x=1&y=%ED%95%9C#frag  ").unwrap();
        assert_eq!(u, "https://example.org/a/b?x=1&y=%ED%95%9C#frag");
        assert_eq!(
            normalize_original_url("http://example.org").unwrap(),
            "http://example.org/"
        );
    }

    #[test]
    fn rejects_other_schemes_and_garbage() {
        for bad in [
            "",
            "   ",
            "example.org",
            "ftp://example.org",
            "javascript:alert(1)",
            "data:text/html,hi",
            "http://",
            "https://",
            "https://user:pw@example.org",
            "mailto:a@b.c",
        ] {
            assert!(normalize_original_url(bad).is_err(), "{bad} should be rejected");
        }
        let long = format!("https://example.org/{}", "a".repeat(MAX_URL_LEN));
        assert!(normalize_original_url(&long).is_err());
    }

    #[test]
    fn parses_numeric_paths_only() {
        assert_eq!(parse_numeric_path("/123"), Some(NumericPath::Id(123)));
        assert_eq!(parse_numeric_path("/1"), Some(NumericPath::Id(1)));
        assert_eq!(parse_numeric_path("/0"), Some(NumericPath::Invalid));
        assert_eq!(parse_numeric_path("/007"), Some(NumericPath::Invalid));
        assert_eq!(
            parse_numeric_path("/99999999999999999999999"),
            Some(NumericPath::Invalid)
        );
        assert_eq!(parse_numeric_path("/"), None);
        assert_eq!(parse_numeric_path("/abc"), None);
        assert_eq!(parse_numeric_path("/12a"), None);
        assert_eq!(parse_numeric_path("/12/34"), None);
        assert_eq!(parse_numeric_path("/assets/main.css"), None);
    }
}
