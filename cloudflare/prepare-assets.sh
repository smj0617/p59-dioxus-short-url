#!/usr/bin/env bash
# Dioxus 웹 번들의 정적 자산(wasm·js·css)을 cloudflare/public/assets 로 준비한다.
# index.html 은 복사하지 않는다: `/` 화면은 Railway 의 Dioxus SSR 이 렌더링한다.
#
# 기본: Railway 와 같은 Dockerfile(Linux)로 빌드한 이미지에서 꺼낸다.
#       wasm·js 파일명 해시가 Railway 서버가 내려주는 HTML 의 경로와 일치한다.
# --local: 로컬 dx bundle 결과를 쓴다 (OS가 다르면 해시가 달라질 수 있음.
#          이 경우 Worker 가 Railway 로 넘겨 화면은 정상 동작한다).
set -euo pipefail

cd "$(dirname "$0")/.."
rm -rf cloudflare/public/assets
mkdir -p cloudflare/public

if [[ "${1:-}" == "--local" ]]; then
  dx bundle --web --release
  cp -r target/dx/p59-dioxus-short-url/release/web/public/assets cloudflare/public/
else
  docker build -t p59-dioxus-short-url:assets .
  cid=$(docker create p59-dioxus-short-url:assets)
  trap 'docker rm "$cid" >/dev/null' EXIT
  docker cp "$cid:/app/public/assets" cloudflare/public/assets
fi

echo "준비 완료: cloudflare/public/assets"
ls cloudflare/public/assets
