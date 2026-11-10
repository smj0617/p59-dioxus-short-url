#!/usr/bin/env bash
# Dioxus 웹 번들을 만들고, 정적 자산(wasm·js·css)만 cloudflare/public 으로 복사한다.
# index.html 은 복사하지 않는다: `/` 화면은 Railway 의 Dioxus SSR 이 렌더링한다.
set -euo pipefail

cd "$(dirname "$0")/.."

dx bundle --web --release

OUT="target/dx/p59-dioxus-short-url/release/web/public"
rm -rf cloudflare/public/assets
mkdir -p cloudflare/public
cp -r "$OUT/assets" cloudflare/public/

echo "복사 완료: cloudflare/public/assets"
ls cloudflare/public/assets
