# Railway 백엔드 이미지: Dioxus 풀스택 서버(SSR + 서버 함수 + /숫자 리디렉션)
#
# 1단계: dx bundle 로 서버 바이너리와 웹(wasm) 번들을 함께 빌드
# (dx 0.7 의 사전 빌드 바이너리가 glibc 2.39 이상을 요구하므로 Debian 13 trixie 기반을 쓴다)
FROM rust:1-trixie AS builder

RUN rustup target add wasm32-unknown-unknown
RUN curl -L --proto '=https' --tlsv1.2 -sSf \
      https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
 && cargo binstall dioxus-cli@0.7.10 -y --force

WORKDIR /app
COPY . .
RUN dx bundle --web --release

# 2단계: 실행 이미지
FROM debian:trixie-slim AS runtime

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
# server 바이너리와 public/(index.html, assets/) 를 함께 둔다. 서버는 실행 파일 옆 public 을 서빙한다.
COPY --from=builder /app/target/dx/p59-dioxus-short-url/release/web/ /app/

# Railway 는 PORT 를 주입한다. 외부에서 접속할 수 있도록 0.0.0.0 에 바인딩한다.
ENV IP=0.0.0.0
ENV PORT=8080
EXPOSE 8080

CMD ["/app/server"]
