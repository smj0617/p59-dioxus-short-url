// 검증 시나리오 자동 점검 스크립트 (Node 18+)
//
// 사용법:
//   node scripts/smoke-test.mjs http://127.0.0.1:8787          # 로컬 서버
//   node scripts/smoke-test.mjs https://<워커>.workers.dev       # Cloudflare 공개 주소
//
// 확인 항목: 생성, 리디렉션(경로·쿼리·프래그먼트 보존), 동시 생성 시 번호 중복 없음,
// 잘못된 URL 거부, 없는 번호 404, 기존 연결 변경 요청 거부.

const base = (process.argv[2] ?? "http://127.0.0.1:8787").replace(/\/$/, "");
let failed = 0;

function check(name, ok, detail = "") {
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `  (${detail})` : ""}`);
  if (!ok) failed++;
}

async function create(url) {
  const res = await fetch(`${base}/api/short-urls`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ url }),
  });
  const text = await res.text();
  let body;
  try { body = JSON.parse(text); } catch { body = text; }
  return { status: res.status, body };
}

// 1) 생성
const original = "https://example.org/docs/page?lang=ko&q=%ED%95%9C%EA%B8%80#section-2";
const created = await create(original);
check("생성 200", created.status === 200, `status=${created.status}`);
check("숫자 번호 발급", Number.isInteger(created.body?.id) && created.body.id > 0, `id=${created.body?.id}`);
const id = created.body?.id;

// 2) 리디렉션 (자동 추적하지 않고 응답 자체를 본다)
const redirect = await fetch(`${base}/${id}`, { redirect: "manual" });
check("/숫자 → 302", redirect.status === 302, `status=${redirect.status}`);
check(
  "Location 이 원본과 같음 (경로·쿼리·프래그먼트 보존)",
  redirect.headers.get("location") === original,
  redirect.headers.get("location") ?? "",
);

// 3) 동시 생성: 서로 다른 URL 30개를 한꺼번에 보낸다
const urls = Array.from({ length: 30 }, (_, i) => `https://example.com/concurrent/${i}?n=${i}#k${i}`);
const results = await Promise.all(urls.map(create));
const ids = results.map((r) => r.body?.id);
check("동시 생성 30건 모두 성공", results.every((r) => r.status === 200));
check("동시 생성 번호 중복 없음", new Set(ids).size === ids.length, `unique=${new Set(ids).size}`);
const mismatched = [];
await Promise.all(
  ids.map(async (n, i) => {
    const r = await fetch(`${base}/${n}`, { redirect: "manual" });
    if (r.headers.get("location") !== urls[i]) mismatched.push(n);
  }),
);
check("동시 생성 후 각 번호가 자기 원본으로 연결", mismatched.length === 0, mismatched.join(","));

// 4) 잘못된 URL
for (const bad of ["ftp://example.org", "javascript:alert(1)", "not a url", "https://", "example.org"]) {
  const r = await create(bad);
  check(`잘못된 URL 거부: ${bad}`, r.status === 400, `status=${r.status}`);
}

// 5) 없는 번호
const missing = await fetch(`${base}/999999999999`, { redirect: "manual" });
const missingBody = await missing.text();
check("없는 번호 404", missing.status === 404, `status=${missing.status}`);
check("404 안내 문구", missingBody.includes("단축 주소를 찾을 수 없습니다"));

// 6) 기존 연결 변경 시도
for (const method of ["PUT", "PATCH", "DELETE", "POST"]) {
  const r = await fetch(`${base}/${id}`, {
    method,
    headers: { "content-type": "application/json" },
    body: method === "DELETE" ? undefined : JSON.stringify({ url: "https://evil.example" }),
    redirect: "manual",
  });
  check(`${method} /${id} 거부`, r.status === 405, `status=${r.status}`);
}
for (const method of ["PUT", "PATCH", "DELETE"]) {
  const r = await fetch(`${base}/api/short-urls/${id}`, {
    method,
    headers: { "content-type": "application/json" },
    body: method === "DELETE" ? undefined : JSON.stringify({ url: "https://evil.example" }),
  });
  check(`${method} /api/short-urls/${id} 거부`, r.status === 405, `status=${r.status}`);
}
const after = await fetch(`${base}/${id}`, { redirect: "manual" });
check("변경 시도 후에도 원래 원본으로 이동", after.headers.get("location") === original);

console.log(failed === 0 ? "\n모든 항목 통과" : `\n실패 ${failed}건`);
process.exit(failed === 0 ? 0 : 1);
