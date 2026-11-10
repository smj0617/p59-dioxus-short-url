# 숫자형 단축 URL 서비스

## 배포 URL

- 서비스 URL: (배포 후 기입)
- 테스트 단축 URL: (배포 후 기입)
- 테스트 원본 URL: (배포 후 기입)

---

Dioxus 0.7 풀스택(Rust)으로 만든 숫자형 단축 URL 서비스입니다. 화면(wasm)과 서버 기능(SSR·생성 API·리디렉션)을 모두 Dioxus 풀스택 한 프로젝트에서 구현했습니다. 다른 백엔드 프레임워크는 붙이지 않았습니다(서버는 Dioxus 풀스택이 내장한 axum 라우터를 씁니다).

- 로그인 없이 긴 URL을 넣고 **단축 URL 만들기**를 누르면 `https://<도메인>/123` 형태의 주소가 나오고, **복사** 버튼으로 클립보드에 복사할 수 있습니다.
- `/123`에 접속하면 서버가 **HTTP 302**로 원본 URL로 보냅니다. 원본의 경로·쿼리·프래그먼트(`#...`)를 그대로 유지합니다.
- 한 번 발급한 번호는 수정·삭제·만료·재사용되지 않습니다. 화면에 해당 버튼이 없을 뿐 아니라 서버와 DB가 변경 요청을 거부합니다.

## 구성

```
브라우저 ──HTTPS──▶ Cloudflare Worker (공개 접점, cloudflare/)
                     ├─ /assets/*  → Workers Static Assets (Dioxus wasm·js·css 번들, 엣지 캐시)
                     ├─ /<숫자>    → Railway 로 프록시, 302 리디렉션을 그대로 전달 (redirect: "manual")
                     ├─ /api/*     → Railway 로 프록시 (Dioxus 서버 함수: 생성)
                     └─ 그 외 (/)  → Railway 로 프록시 (Dioxus SSR HTML → 브라우저가 hydration)
                                       │
                         Railway: Dioxus 풀스택 서버 (Dockerfile, railway.json)
                                       │  DATABASE_URL (서버 환경변수로만 주입)
                         Railway: PostgreSQL (short_urls 테이블)
```

| 파일 | 역할 |
| --- | --- |
| `src/main.rs` | Dioxus 앱 진입점, 라우터, 입력·생성·결과·복사 화면, 404 화면 |
| `src/api.rs` | 서버 함수 `POST /api/short-urls` (생성만 존재) |
| `src/server.rs` | DB 풀·마이그레이션, INSERT/SELECT, `/숫자` 리디렉션 미들웨어, 변경 요청 405 거부, 404 안내 페이지, `/healthz` |
| `src/validate.rs` | http/https URL 검증·정규화, 숫자 경로 파싱 (단위 테스트 포함) |
| `migrations/0001_create_short_urls.sql` | 스키마: IDENTITY PK, CHECK 제약, UPDATE/DELETE/TRUNCATE 거부 트리거 |
| `Dockerfile`, `railway.json` | Railway 빌드·실행·헬스체크 설정 |
| `cloudflare/wrangler.toml`, `cloudflare/src/index.js` | Cloudflare Worker 라우팅·프록시·정적 자산 설정 |
| `cloudflare/prepare-assets.sh` | `dx bundle` 결과의 정적 자산을 `cloudflare/public/`에 복사 |
| `.env.example` | 필요한 환경변수 예시 (비밀 값 없음) |
| `scripts/smoke-test.mjs` | 검증 시나리오 자동 점검 스크립트 |

## 로컬 실행

### 준비물

- Rust (stable), `rustup target add wasm32-unknown-unknown`
- Dioxus CLI 0.7: `cargo binstall dioxus-cli@0.7.10` (또는 `cargo install dioxus-cli --version 0.7.10`)
- Docker (로컬 PostgreSQL 용)
- Node.js 18+ (검증 스크립트·wrangler 용, 선택)

### DB 준비

```bash
docker run -d --name p59-postgres \
  -e POSTGRES_USER=shorturl -e POSTGRES_PASSWORD=<로컬 비밀번호> -e POSTGRES_DB=shorturl \
  -p 5432:5432 postgres:17-alpine
```

테이블은 직접 만들 필요가 없습니다. 서버가 시작할 때 `sqlx::migrate!`로 `migrations/`를 적용합니다(이미 적용한 마이그레이션은 건너뜀).

### 환경변수

| 이름 | 필수 | 설명 |
| --- | --- | --- |
| `DATABASE_URL` | 예 | PostgreSQL 접속 URL. 예: `postgres://shorturl:<비밀번호>@localhost:5432/shorturl` |
| `DATABASE_MAX_CONNECTIONS` | 아니요 | 커넥션 풀 크기 (기본 5) |
| `IP` | 아니요 | 바인딩 주소. 로컬 기본 `127.0.0.1`, 배포는 `0.0.0.0` |
| `PORT` | 아니요 | 포트 (기본 8080, Railway 가 자동 주입) |

`.env.example`을 참고하세요. 서버만 이 값을 읽고, 브라우저 번들에는 들어가지 않습니다.

### 개발 서버

```bash
export DATABASE_URL=postgres://shorturl:<비밀번호>@localhost:5432/shorturl
dx serve --platform web        # http://127.0.0.1:8080
```

### 릴리스 빌드로 실행

```bash
dx bundle --web --release
cd target/dx/p59-dioxus-short-url/release/web
DATABASE_URL=postgres://... IP=127.0.0.1 PORT=8787 ./server      # Windows: server.exe
```

### 테스트

```bash
cargo test --features server               # URL 검증·숫자 경로 파싱 단위 테스트
node scripts/smoke-test.mjs http://127.0.0.1:8787   # 실행 중인 서버에 대한 시나리오 점검
```

## 배포

### 1. Railway (백엔드 + DB)

1. Railway에서 **New Project → Deploy from GitHub repo**로 이 저장소를 고릅니다. 저장소 최상위의 `railway.json`에 따라 `Dockerfile`로 빌드하고, `/healthz`로 헬스체크합니다.
2. 같은 프로젝트에 **New → Database → PostgreSQL**을 추가합니다.
3. 앱 서비스의 **Variables**에 다음을 넣습니다.
   - `DATABASE_URL` = `${{Postgres.DATABASE_URL}}` (Railway 변수 참조. 실제 비밀번호를 파일에 적지 않습니다)
   - `IP`, `PORT`는 Dockerfile 에 기본값이 있고 Railway 가 `PORT`를 주입하므로 따로 넣지 않아도 됩니다.
4. 앱 서비스 **Settings → Networking → Generate Domain**으로 공개 도메인(`https://xxx.up.railway.app`)을 만듭니다.
5. `https://xxx.up.railway.app/healthz`가 `ok`를 돌려주는지 확인합니다.

서버를 다시 시작하거나 재배포해도 데이터는 Railway PostgreSQL 볼륨에 남아 있으므로, 기존 단축 주소는 같은 원본으로 계속 연결됩니다.

### 2. Cloudflare Workers (공개 접점 + 웹 번들)

1. `cloudflare/wrangler.toml`의 `BACKEND_ORIGIN`을 위 Railway 도메인으로 바꿉니다(비밀 값 아님).
2. 웹 번들 정적 자산을 준비합니다.
   ```bash
   bash cloudflare/prepare-assets.sh      # dx bundle 후 public/assets 를 cloudflare/public/ 에 복사
   ```
3. 배포합니다.
   ```bash
   cd cloudflare
   npm install
   npx wrangler login
   npx wrangler deploy
   ```
4. 출력된 `https://p59-dioxus-short-url.<계정>.workers.dev`가 서비스 URL입니다(HTTPS 기본 제공). 커스텀 도메인을 쓰려면 Workers 설정의 **Domains & Routes**에서 연결합니다.

### 라우팅 구성과 이유

- Dioxus 풀스택 웹 클라이언트는 서버가 SSR 한 HTML을 **hydration** 하는 방식이라, HTML 문서는 Dioxus 서버(Railway)가 렌더링해야 합니다. 그래서 Worker는 `/`와 그 밖의 페이지 요청을 Railway로 프록시합니다.
- wasm·js·css 같은 웹 번들은 Workers Static Assets로 Cloudflare 엣지에서 직접 응답합니다(`/assets/*`, 해시가 붙은 파일명이라 1년 immutable 캐시). Railway 빌드와 해시가 달라 파일이 없으면 Railway로 넘겨 화면이 깨지지 않게 했습니다.
- `/<숫자>`는 Railway로 프록시하면서 `redirect: "manual"`로 302 응답을 그대로 브라우저에 넘깁니다. Worker가 리디렉션을 따라가면 원본 사이트 내용을 대신 받아와 버리기 때문입니다.
- `run_worker_first = true`로 모든 요청이 먼저 Worker를 거치게 해서, 정적 `index.html`이 SSR 화면이나 숫자 경로 처리를 가로채지 않게 했습니다(`.assetsignore`로 `index.html`은 올리지도 않습니다).
- 서버 함수는 상대 경로 `/api/short-urls`로 호출되므로, Cloudflare 도메인에서 그대로 같은 출처(same-origin)로 동작합니다. CORS 설정이나 API 주소를 브라우저 코드에 넣을 필요가 없습니다.

## 번호 발급과 불변성 설계

### 번호 발급과 중복 방지

```sql
id BIGINT GENERATED ALWAYS AS IDENTITY (START WITH 1 INCREMENT BY 1 NO CYCLE) PRIMARY KEY
```

- 번호는 애플리케이션이 계산하지 않고(`MAX(id)+1` 같은 경쟁 조건 없음) PostgreSQL IDENTITY 시퀀스가 발급합니다. `nextval()`은 원자적이어서 동시에 여러 요청이 와도 같은 값을 두 번 주지 않습니다.
- `PRIMARY KEY`가 같은 번호의 두 번째 행을 DB 수준에서 거부합니다.
- `GENERATED ALWAYS`라서 누구도 번호를 직접 지정해 넣을 수 없습니다(`cannot insert a non-DEFAULT value into column "id"`).
- 시퀀스는 롤백된 값도 되돌리지 않으므로 번호가 재사용되지 않습니다. 대신 빈 번호가 생길 수 있습니다(번호 연속성은 보장하지 않음).
- 경로는 `/123`처럼 숫자만 씁니다. `/0`, `/007`처럼 앞자리가 0인 형태나 범위를 넘는 숫자는 발급할 수 없는 번호로 보고 404를 돌려줍니다. 그래서 번호 하나에 경로가 하나만 대응합니다.

### 같은 원본 URL을 다시 넣으면

**항상 새 번호를 발급합니다.** 원본 URL에 UNIQUE 제약을 두지 않고, 기존 행을 찾아 재사용하거나 갱신하는 코드도 없습니다. 생성 경로가 `INSERT` 하나뿐이라 동시 요청에서도 조회-후-삽입 경쟁이 없고, 기존 연결을 건드릴 여지가 아예 없습니다.

### 생성 후 불변

여러 겹으로 막았습니다.

1. **API가 없음**: 서버 함수는 `POST /api/short-urls`(생성) 하나뿐입니다. 수정·삭제·만료 API와 화면 버튼이 없고, 만료 컬럼이나 정리 작업도 없습니다.
2. **덮어쓰기 경로 없음**: 저장 SQL은 `INSERT ... RETURNING id`뿐이고 `ON CONFLICT DO UPDATE` 같은 upsert가 없습니다.
3. **변경 메서드 명시적 거부**: 미들웨어가 `/<숫자>`에 대한 GET/HEAD 외 메서드(POST·PUT·PATCH·DELETE)와 `/api/short-urls[/...]`에 대한 PUT·PATCH·DELETE를 **405 Method Not Allowed**로 거부합니다.
4. **DB 트리거**: `short_urls`에 대한 `UPDATE`·`DELETE`·`TRUNCATE`를 트리거가 예외로 거부합니다. 코드 실수나 수동 SQL로도 기존 연결을 바꾸거나 행을 지워 번호를 비울 수 없습니다.

### URL 검증과 리디렉션

- 서버(`src/validate.rs`의 `normalize_original_url`)에서 다시 검증합니다. 화면의 검증은 안내용입니다.
  - 앞뒤 공백 제거, 2048자 이하
  - `http://` 또는 `https://` 스킴만 허용 (`javascript:`, `data:`, `ftp:`, `mailto:` 등 거부)
  - 호스트가 있어야 함, `user:pass@`가 들어간 URL 거부
  - 실패하면 **400**과 한국어 안내 메시지
- DB에도 `CHECK (original_url ~* '^https?://')`와 길이 CHECK가 있습니다.
- `url` 크레이트로 정규화해 저장하므로 저장 값은 ASCII(퍼센트 인코딩)이며 경로·쿼리·프래그먼트를 그대로 담습니다. 리디렉션은 이 값을 `Location` 헤더에 그대로 넣은 **302 Found**입니다. 브라우저는 `Location`의 `#fragment`까지 따라갑니다.
- 없는 번호는 **404**와 Dioxus SSR로 렌더링한 안내 화면("단축 주소를 찾을 수 없습니다")을 돌려줍니다.

## 검증 시나리오와 결과

`scripts/smoke-test.mjs`로 재현할 수 있습니다. 아래는 로컬에서 세 경로로 실행한 결과입니다: ① 릴리스 빌드 서버 + Docker PostgreSQL 17, ② Railway와 같은 `Dockerfile`로 만든 컨테이너, ③ 로컬 `wrangler dev`로 띄운 Cloudflare Worker를 거친 경로. 세 경로 모두 전 항목 통과했습니다. 배포 환경에서의 결과는 배포 후 같은 스크립트를 공개 주소로 실행해 확인합니다.

| 시나리오 | 방법 | 로컬 결과 |
| --- | --- | --- |
| 로그인 없이 생성·복사 | 브라우저에서 URL 입력 → 생성 → 복사 | 번호 발급, 단축 주소 표시, 복사 버튼 "복사됨 ✓" |
| 다른 곳에서 열어 이동 | `GET /<번호>` (리디렉션 미추적) | `302`, `Location`이 원본과 정확히 같음 (경로·쿼리·`#section-2` 보존) |
| 동시 생성 | 서로 다른 URL 30건을 `Promise.all`로 동시 POST | 30건 모두 성공, 번호 30개 모두 다름, 각 번호가 자기 원본으로 이동 |
| 재시작 후 유지 | 서버 프로세스 종료 → 재시작 → 이전 번호 접속 | 이전 번호가 같은 원본으로 `302` |
| 잘못된 URL | `ftp://…`, `javascript:alert(1)`, `not a url`, `https://`, `example.org` | 모두 `400` + 안내 메시지 |
| 없는 번호 | `GET /999999999999` | `404` + "단축 주소를 찾을 수 없습니다" 안내 화면 |
| 기존 연결 변경 시도 | `PUT/PATCH/DELETE/POST /<번호>`, `PUT/PATCH/DELETE /api/short-urls/<번호>` | 모두 `405`, 이후에도 원래 원본으로 이동 |
| DB 직접 변경 시도 | `UPDATE`/`DELETE` SQL, id 지정 `INSERT`, `ftp://` INSERT | 트리거·IDENTITY·CHECK 제약이 모두 거부 |

배포 후 확인:

```bash
node scripts/smoke-test.mjs https://<워커 주소>
```
