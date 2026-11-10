// Cloudflare Worker: 서비스의 공개 HTTPS 접점.
//
// 라우팅 표
//   /assets/*        → Workers Static Assets 에 올린 Dioxus 웹 번들(wasm·js·css)을 엣지에서 바로 응답.
//                      (없으면 Railway 로 넘겨 버전 차이가 나도 화면이 깨지지 않게 한다)
//   /<숫자>          → Railway 로 프록시. 백엔드의 302 리디렉션을 따라가지 않고 그대로 브라우저에 전달.
//   /api/*           → Railway 로 프록시 (Dioxus 서버 함수: 단축 주소 생성).
//   그 밖의 경로 (/)  → Railway 로 프록시 (Dioxus SSR HTML. 브라우저가 이 HTML 을 hydration 한다).
//
// BACKEND_ORIGIN 은 Railway 서비스의 공개 도메인(비밀 아님)이다. DB 접속 정보는 여기에 두지 않는다.

const NUMERIC_PATH = /^\/\d+$/;
const STATIC_PATH = /^\/assets\//;

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    // HTTPS 강제 (로컬 wrangler dev 는 http 이므로 제외)
    const isLocal = url.hostname === "localhost" || url.hostname === "127.0.0.1";
    if (url.protocol === "http:" && !isLocal) {
      url.protocol = "https:";
      return Response.redirect(url.toString(), 301);
    }

    if (!env.BACKEND_ORIGIN || env.BACKEND_ORIGIN.includes("REPLACE-WITH")) {
      return new Response("BACKEND_ORIGIN 이 설정되지 않았습니다 (wrangler.toml 의 [vars] 확인).", {
        status: 500,
        headers: { "content-type": "text/plain; charset=utf-8" },
      });
    }

    // 정적 자산: 엣지에서 응답 (파일명에 해시가 들어 있어 오래 캐시해도 안전)
    if (STATIC_PATH.test(url.pathname) && (request.method === "GET" || request.method === "HEAD")) {
      const asset = await env.ASSETS.fetch(request);
      if (asset.ok) {
        const res = new Response(asset.body, asset);
        res.headers.set("cache-control", "public, max-age=31536000, immutable");
        res.headers.set("x-served-by", "cloudflare-assets");
        return res;
      }
    }

    return proxyToBackend(request, url, env.BACKEND_ORIGIN, NUMERIC_PATH.test(url.pathname));
  },
};

async function proxyToBackend(request, url, backendOrigin, isShortPath) {
  const target = new URL(url.pathname + url.search, backendOrigin);

  const headers = new Headers(request.headers);
  headers.set("x-forwarded-host", url.host);
  headers.set("x-forwarded-proto", "https");

  const hasBody = request.method !== "GET" && request.method !== "HEAD";
  const upstream = await fetch(target.toString(), {
    method: request.method,
    headers,
    body: hasBody ? request.body : undefined,
    // 중요: 리디렉션을 Worker 가 따라가면 원본 사이트 내용을 대신 받아오게 된다.
    // manual 로 두어 백엔드의 302 + Location 을 브라우저에 그대로 돌려준다.
    redirect: "manual",
  });

  const res = new Response(upstream.body, upstream);
  res.headers.set("x-served-by", isShortPath ? "cloudflare-worker->railway(redirect)" : "cloudflare-worker->railway");
  return res;
}
