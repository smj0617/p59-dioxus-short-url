-- 숫자 ↔ 원본 URL 매핑 테이블
--
-- * id: PostgreSQL IDENTITY 시퀀스가 발급한다. GENERATED ALWAYS 이므로
--   애플리케이션(또는 사용자)이 번호를 직접 지정할 수 없다.
--   시퀀스의 nextval()은 트랜잭션과 무관하게 원자적이어서 동시 요청에서도
--   같은 값이 두 번 나오지 않는다. 롤백된 값도 재사용하지 않는다(빈 번호는 생길 수 있음).
-- * PRIMARY KEY: 같은 번호의 두 번째 행은 DB가 거부한다.
-- * CHECK: http:// 또는 https:// 로 시작하는 URL만 저장된다 (서버 검증의 2차 방어선).
CREATE TABLE IF NOT EXISTS short_urls (
    id           BIGINT GENERATED ALWAYS AS IDENTITY (START WITH 1 INCREMENT BY 1 NO CYCLE) PRIMARY KEY,
    original_url TEXT        NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT short_urls_original_url_scheme CHECK (original_url ~* '^https?://'),
    CONSTRAINT short_urls_original_url_length CHECK (char_length(original_url) <= 2048)
);

-- 생성 후 불변: UPDATE / DELETE / TRUNCATE 를 DB 수준에서 거부한다.
-- 애플리케이션에는 수정·삭제 코드가 없지만, 잘못된 코드나 수동 SQL로
-- 기존 연결이 바뀌거나 번호가 재사용되는 일까지 막기 위한 장치다.
CREATE OR REPLACE FUNCTION short_urls_reject_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'short_urls 는 생성 후 변경할 수 없습니다 (% 거부)', TG_OP
        USING ERRCODE = 'restrict_violation';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS short_urls_immutable_row ON short_urls;
CREATE TRIGGER short_urls_immutable_row
    BEFORE UPDATE OR DELETE ON short_urls
    FOR EACH ROW EXECUTE FUNCTION short_urls_reject_mutation();

DROP TRIGGER IF EXISTS short_urls_immutable_truncate ON short_urls;
CREATE TRIGGER short_urls_immutable_truncate
    BEFORE TRUNCATE ON short_urls
    FOR EACH STATEMENT EXECUTE FUNCTION short_urls_reject_mutation();
