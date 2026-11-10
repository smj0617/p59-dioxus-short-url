//! 서버 전용 코드: DB 연결, 번호 발급(INSERT), `/숫자` 리디렉션, 404 안내 화면.
//!
//! 이 모듈은 `server` 기능으로 빌드할 때만 컴파일된다. DB 접속 정보는 서버 환경변수
//! (`DATABASE_URL`)로만 받으며 브라우저 번들에는 포함되지 않는다.

use std::time::Duration;

use anyhow::Context;
use axum::{
    extract::Request,
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use dioxus::prelude::*;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::sync::OnceCell;

use crate::api::CREATE_PATH;
use crate::validate::{parse_numeric_path, NumericPath};

static POOL: OnceCell<PgPool> = OnceCell::const_new();

/// 커넥션 풀을 돌려준다. 처음 호출될 때 접속하고 마이그레이션을 적용한다.
pub async fn pool() -> anyhow::Result<&'static PgPool> {
    POOL.get_or_try_init(|| async {
        let database_url =
            std::env::var("DATABASE_URL").context("DATABASE_URL 환경변수가 필요합니다")?;
        let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&database_url)
            .await
            .context("PostgreSQL에 접속하지 못했습니다")?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .context("마이그레이션 실패")?;

        Ok(pool)
    })
    .await
}

/// 새 번호를 발급하며 원본 URL을 저장한다.
///
/// 번호는 애플리케이션이 계산하지 않는다(`MAX(id)+1` 같은 경쟁 조건 없음).
/// `GENERATED ALWAYS AS IDENTITY` 시퀀스가 동시 요청에도 서로 다른 값을 주고,
/// PRIMARY KEY 가 혹시 모를 중복을 최종적으로 거부한다.
/// 이 함수는 INSERT 만 하며 `ON CONFLICT ... DO UPDATE` 같은 덮어쓰기 경로가 없다.
pub async fn insert_short_url(original_url: &str) -> anyhow::Result<i64> {
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO short_urls (original_url) VALUES ($1) RETURNING id",
    )
    .bind(original_url)
    .fetch_one(pool().await?)
    .await?;
    Ok(id)
}

/// 번호로 원본 URL을 찾는다.
pub async fn find_original_url(id: i64) -> anyhow::Result<Option<String>> {
    let url = sqlx::query_scalar::<_, String>("SELECT original_url FROM short_urls WHERE id = $1")
        .bind(id)
        .fetch_optional(pool().await?)
        .await?;
    Ok(url)
}

/// Dioxus 풀스택 라우터(SSR·서버 함수·정적 자산)에 단축 URL 처리와 헬스체크를 더한다.
pub fn build_router(app: fn() -> Element) -> Router {
    dioxus::server::router(app)
        .route("/healthz", get(healthz))
        .layer(axum::middleware::from_fn(short_url_middleware))
}

/// 모든 요청 앞단에서 동작하는 미들웨어.
///
/// - `/숫자` → DB 조회 후 HTTP 302 리디렉션 또는 404 안내 화면
/// - `/숫자` 에 대한 POST/PUT/PATCH/DELETE → 405 (기존 연결 변경 불가)
/// - `/api/short-urls[/...]` 에 대한 PUT/PATCH/DELETE → 405 (수정·삭제 API 없음)
/// - 그 밖의 요청은 Dioxus 라우터로 넘긴다.
async fn short_url_middleware(req: Request, next: Next) -> Response {
    let path = req.uri().path();

    if let Some(numeric) = parse_numeric_path(path) {
        return match *req.method() {
            Method::GET | Method::HEAD => redirect_or_not_found(numeric).await,
            _ => immutable_rejection("GET, HEAD"),
        };
    }

    let is_short_url_api = path == CREATE_PATH || path.starts_with("/api/short-urls/");
    if is_short_url_api && matches!(*req.method(), Method::PUT | Method::PATCH | Method::DELETE) {
        return immutable_rejection("POST");
    }

    next.run(req).await
}

async fn redirect_or_not_found(numeric: NumericPath) -> Response {
    let id = match numeric {
        NumericPath::Id(id) => id,
        NumericPath::Invalid => return not_found_page(None),
    };

    match find_original_url(id).await {
        Ok(Some(original_url)) => redirect_to(&original_url),
        Ok(None) => not_found_page(Some(id)),
        Err(err) => {
            eprintln!("[redirect] DB 오류 (id={id}): {err:#}");
            error_page()
        }
    }
}

/// 원본 URL로 HTTP 302 리디렉션한다.
///
/// 저장된 값은 `url` 크레이트가 직렬화한 문자열이라 ASCII 만으로 이루어져 있고
/// 경로·쿼리·프래그먼트(#...)를 그대로 담고 있다. 이를 `Location` 헤더에 그대로 넣는다.
fn redirect_to(original_url: &str) -> Response {
    match HeaderValue::from_str(original_url) {
        Ok(location) => (
            StatusCode::FOUND,
            [
                (header::LOCATION, location),
                (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            ],
        )
            .into_response(),
        Err(_) => error_page(),
    }
}

fn immutable_rejection(allow: &'static str) -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, HeaderValue::from_static(allow))],
        "한 번 발급한 단축 주소는 수정·삭제·덮어쓰기할 수 없습니다.\n",
    )
        .into_response()
}

async fn healthz() -> Response {
    let ok = match pool().await {
        Ok(pool) => sqlx::query("SELECT 1").execute(pool).await.is_ok(),
        Err(_) => false,
    };
    if ok {
        (StatusCode::OK, "ok").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "database unavailable").into_response()
    }
}

/// 존재하지 않는 번호에 대한 404 안내 화면 (Dioxus SSR 로 렌더링).
fn not_found_page(id: Option<i64>) -> Response {
    let detail = match id {
        Some(id) => format!("/{id} 번 단축 주소는 발급된 적이 없습니다."),
        None => "발급될 수 없는 형식의 번호입니다.".to_string(),
    };
    let body = dioxus_ssr::render_element(rsx! {
        main { class: "card",
            p { class: "code", "404" }
            h1 { "단축 주소를 찾을 수 없습니다" }
            p { "{detail}" }
            p { "주소를 다시 확인하거나 새 단축 주소를 만들어 보세요." }
            a { class: "button", href: "/", "단축 주소 만들기" }
        }
    });
    (StatusCode::NOT_FOUND, Html(html_document("찾을 수 없는 단축 주소", &body))).into_response()
}

fn error_page() -> Response {
    let body = dioxus_ssr::render_element(rsx! {
        main { class: "card",
            p { class: "code", "500" }
            h1 { "잠시 후 다시 시도해 주세요" }
            p { "서버에서 단축 주소를 처리하지 못했습니다." }
            a { class: "button", href: "/", "처음으로" }
        }
    });
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(html_document("서버 오류", &body)),
    )
        .into_response()
}

fn html_document(title: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · 숫자형 단축 URL</title>
<style>
  body {{ margin:0; min-height:100vh; display:grid; place-items:center; background:#f4f5f7;
         font-family: system-ui, -apple-system, "Segoe UI", "Malgun Gothic", sans-serif; color:#1f2328; }}
  .card {{ background:#fff; padding:40px 32px; border-radius:16px; box-shadow:0 8px 30px rgba(0,0,0,.08);
          max-width:420px; margin:16px; text-align:center; }}
  .code {{ font-size:56px; font-weight:800; margin:0; color:#d1242f; }}
  h1 {{ font-size:22px; margin:8px 0 16px; }}
  p {{ line-height:1.6; color:#57606a; }}
  .button {{ display:inline-block; margin-top:12px; padding:10px 18px; border-radius:10px;
            background:#0969da; color:#fff; text-decoration:none; font-weight:600; }}
</style>
</head>
<body>{body}</body>
</html>"#
    )
}
