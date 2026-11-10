//! Dioxus 풀스택 숫자형 단축 URL 서비스.
//!
//! - 브라우저(wasm, `web` 기능): URL 입력·생성·결과 표시·복사 화면
//! - 서버(`server` 기능): SSR, 생성 서버 함수, `/숫자` 리디렉션, PostgreSQL 저장

use dioxus::prelude::*;

mod api;
#[cfg(feature = "server")]
mod server;
mod validate;

use api::{create_short_url, ShortUrl};

const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    // 브라우저: 서버가 SSR 한 화면을 hydration 한다.
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);

    // 서버: Dioxus 라우터에 단축 URL 미들웨어를 얹어 실행한다.
    // IP / PORT 환경변수로 바인딩 주소를 정한다 (Railway 는 PORT 를 주입한다).
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        // 시작 시 DB 접속·마이그레이션을 먼저 확인한다. 실패하면 서버가 뜨지 않는다.
        server::pool().await?;
        Ok(server::build_router(App))
    });
}

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    Home {},
    // `/숫자` 는 서버 미들웨어가 먼저 가로채 리디렉션한다. 여기 오는 것은 그 밖의 경로뿐이다.
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

#[component]
fn App() -> Element {
    rsx! {
        document::Title { "숫자형 단축 URL" }
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1" }
        document::Stylesheet { href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[derive(Debug, Clone, PartialEq)]
enum CopyState {
    Idle,
    Copied,
    Failed,
}

#[component]
fn Home() -> Element {
    let mut input = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut result = use_signal(|| None::<ShortUrl>);
    let mut origin = use_signal(String::new);
    let mut copy_state = use_signal(|| CopyState::Idle);
    let mut history = use_signal(Vec::<ShortUrl>::new);

    // 단축 주소는 사용자가 접속한 공개 주소(Cloudflare 도메인) 기준으로 보여준다.
    use_effect(move || {
        spawn(async move {
            if let Ok(value) = document::eval("return window.location.origin;").await {
                if let Some(o) = value.as_str() {
                    origin.set(o.to_string());
                }
            }
        });
    });

    let short_url_of = move |id: i64| format!("{}/{}", origin(), id);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        if pending() {
            return;
        }
        // 브라우저 검증은 빠른 안내용이다. 최종 판단은 서버가 다시 한다.
        if let Err(message) = validate::normalize_original_url(&input()) {
            error.set(Some(message));
            return;
        }
        spawn(async move {
            pending.set(true);
            error.set(None);
            copy_state.set(CopyState::Idle);
            match create_short_url(input()).await {
                Ok(created) => {
                    history.write().insert(0, created.clone());
                    result.set(Some(created));
                    input.set(String::new());
                }
                Err(err) => {
                    let message = err
                        .message
                        .clone()
                        .unwrap_or_else(|| "단축 주소를 만들지 못했습니다.".to_string());
                    error.set(Some(message));
                }
            }
            pending.set(false);
        });
    };

    let copy = move |_| {
        let Some(created) = result() else { return };
        let text = short_url_of(created.id);
        spawn(async move {
            let ok = copy_to_clipboard(&text).await;
            copy_state.set(if ok { CopyState::Copied } else { CopyState::Failed });
        });
    };

    rsx! {
        main { class: "page",
            header { class: "hero",
                h1 { "숫자형 단축 URL" }
                p { "긴 주소를 넣으면 " code { "/123" } " 같은 숫자 주소를 만들어 드립니다. 로그인은 필요 없습니다." }
            }

            form { class: "card", onsubmit: submit,
                label { r#for: "url", "원본 URL" }
                div { class: "row",
                    input {
                        id: "url",
                        r#type: "url",
                        name: "url",
                        placeholder: "https://example.org/path?query=1#section",
                        autocomplete: "off",
                        required: true,
                        value: "{input}",
                        oninput: move |e| input.set(e.value()),
                    }
                    button { r#type: "submit", disabled: pending(),
                        if pending() { "만드는 중…" } else { "단축 URL 만들기" }
                    }
                }
                if let Some(message) = error() {
                    p { class: "error", role: "alert", "{message}" }
                }
                p { class: "hint", "http:// 또는 https:// 주소만 등록할 수 있습니다. 한 번 만든 주소는 수정·삭제·만료되지 않습니다." }
            }

            if let Some(created) = result() {
                section { class: "card result", aria_live: "polite",
                    p { class: "label", "단축 주소" }
                    div { class: "row",
                        a {
                            class: "short",
                            href: "{short_url_of(created.id)}",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "{short_url_of(created.id)}"
                        }
                        button { r#type: "button", class: "secondary", onclick: copy,
                            match copy_state() {
                                CopyState::Copied => "복사됨 ✓",
                                CopyState::Failed => "복사 실패",
                                CopyState::Idle => "복사",
                            }
                        }
                    }
                    p { class: "label", "원본 URL" }
                    p { class: "original", "{created.original_url}" }
                }
            }

            if history().len() > 1 {
                section { class: "card",
                    p { class: "label", "이번에 만든 주소" }
                    ul { class: "history",
                        for item in history() {
                            li { key: "{item.id}",
                                a { href: "{short_url_of(item.id)}", target: "_blank", rel: "noopener noreferrer", "/{item.id}" }
                                span { "{item.original_url}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 클립보드에 복사한다. `navigator.clipboard` 가 없으면(비보안 컨텍스트 등) textarea 방식으로 대체한다.
async fn copy_to_clipboard(text: &str) -> bool {
    let eval = document::eval(
        r#"
        const text = await dioxus.recv();
        try {
            await navigator.clipboard.writeText(text);
            return true;
        } catch (_) {
            const ta = document.createElement("textarea");
            ta.value = text;
            ta.setAttribute("readonly", "");
            ta.style.position = "fixed";
            ta.style.opacity = "0";
            document.body.appendChild(ta);
            ta.select();
            let ok = false;
            try { ok = document.execCommand("copy"); } catch (_) {}
            document.body.removeChild(ta);
            return ok;
        }
        "#,
    );
    if eval.send(text.to_string()).is_err() {
        return false;
    }
    matches!(eval.await, Ok(serde_json::Value::Bool(true)))
}

#[component]
fn NotFound(segments: Vec<String>) -> Element {
    let path = format!("/{}", segments.join("/"));
    rsx! {
        main { class: "page",
            section { class: "card center",
                p { class: "code", "404" }
                h1 { "페이지를 찾을 수 없습니다" }
                p { code { "{path}" } " 는 없는 주소입니다. 단축 주소는 " code { "/123" } " 처럼 숫자로만 이루어져 있습니다." }
                Link { class: "button", to: Route::Home {}, "단축 주소 만들기" }
            }
        }
    }
}
