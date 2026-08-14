# Scouter Server Rust 개발 TODO

이 문서는 Java/Scala 기반 `scouter.server`를 Rust로 이식하는 작업의 진행표다.
작업은 위에서 아래로 진행하되, 각 항목은 구현과 검증이 모두 끝난 뒤에만 완료 처리한다.

## 현재 기준선

- [x] Rust 프로젝트 기본 구조 구성
- [x] UDP/TCP/HTTP 서버 기본 구조 구성
- [x] Counter, XLog, Profile, Text, Alert, Status, Summary, Object 기본 수집 경로 구성
- [x] 날짜별 DB 컨테이너 및 기본 저장소 구성
- [x] 기본 TCP 조회 핸들러 42개 등록
- [x] 단위 테스트 32개 통과 (`cargo test`)
- [ ] Clippy 경고 없이 통과 (`cargo clippy --all-targets -- -D warnings`)
- [ ] Java Scouter agent/client와 end-to-end 호환성 확인

현재 원본 서버에는 229개의 TCP 서비스 명령이 있고 Rust 서버에는 42개가 등록되어 있다.
이 수치는 기능 완성도가 아니라 명령 등록 범위를 나타내며, 등록된 명령도 호환성 테스트가 필요하다.

## P0: 서버를 안전하게 실행할 수 있는 상태

### 1. 서버 생명주기와 종료 처리

- [ ] HTTP가 비활성화되어도 UDP/TCP 서버가 종료되지 않도록 수정
- [ ] UDP, TCP, HTTP task의 비정상 종료 정책 정의
- [ ] 모든 background worker에 `CancellationToken` 연결
- [ ] 종료 시 고정 sleep 대신 queue drain 완료를 확인
- [ ] DB flush 및 worker 종료 결과를 확인하고 오류를 기록
- [ ] 서버 생명주기 통합 테스트 추가

완료 조건:

- 기본 설정(`net_http_server_enabled = false`)으로 서버가 계속 실행된다.
- `Ctrl-C` 수신 후 모든 listener와 worker가 제한 시간 안에 종료된다.
- 종료 직전 수신한 데이터가 저장되었음을 테스트로 확인한다.

### 2. 프로토콜 호환성 기준 테스트

- [ ] 이식 대상 Java/Scala 서버 버전과 Git commit 기록
- [ ] Java 구현에서 Pack 종류별 golden binary 생성
- [ ] Rust decoder가 golden binary를 동일한 값으로 읽는 테스트 추가
- [ ] Rust encoder 결과를 Java decoder로 읽는 역방향 테스트 추가
- [ ] Value 타입 전체의 Java/Rust round-trip 테스트 추가
- [ ] 잘린 패킷, 잘못된 길이, 알 수 없는 타입 테스트 추가
- [ ] 큰 패킷과 최대 크기 경계 테스트 추가
- [ ] 날짜, signed integer, UTF-8 문자열 경계값 테스트 추가

완료 조건:

- 지원한다고 표시한 모든 Value/Pack에 양방향 호환 테스트가 존재한다.
- malformed input이 panic이나 과도한 메모리 할당 없이 거부된다.

### 3. TCP agent 프로토콜 완성

- [ ] TCP Agent V1 응답의 정확한 message framing 구현
- [ ] TCP Agent V2 길이 값 검증 및 최대 크기 제한
- [ ] `HAS_NEXT`, `NO_NEXT`, 오류 flag 처리 검증
- [ ] keep-alive timeout과 연결 제거 동작 검증
- [ ] 동일 object의 복수 agent connection 정책 검증
- [ ] agent command 요청/응답 correlation 및 동시성 정책 구현
- [ ] Java agent를 사용하는 TCP 통합 테스트 추가

완료 조건:

- V1/V2 응답을 부분 read 및 여러 메시지가 합쳐진 read에서도 정확히 처리한다.
- 연결 중단이나 비정상 길이로 worker가 무한 대기하거나 과도한 메모리를 할당하지 않는다.

### 4. MultiPacket과 UDP 안정성

- [ ] Java `MultiPacketProcessor`와 재조립 규칙 대조
- [ ] out-of-order fragment 처리
- [ ] duplicate fragment 처리
- [ ] 누락 fragment timeout 및 메모리 회수
- [ ] fragment 개수와 전체 크기 상한 설정
- [ ] UDP socket receive buffer 설정 적용 여부 검증
- [ ] queue full 시 drop counter와 rate-limited warning 추가
- [ ] 실제 agent 트래픽 replay 테스트 추가

완료 조건:

- 순서 변경, 중복, 유실 패킷 테스트를 모두 통과한다.
- 비정상 입력으로 재조립 메모리가 무제한 증가하지 않는다.

### 5. 인증과 권한

- [ ] 모든 로그인을 허용하는 임시 구현 제거
- [ ] Account/Group 데이터 모델과 영속 저장 구현
- [ ] 비밀번호 해시 및 검증 구현
- [ ] 계정 추가/수정/삭제 서비스 구현
- [ ] 그룹 정책과 명령별 권한 검사 구현
- [ ] session activity 기반 TTL 갱신
- [ ] 로그아웃 및 강제 session 만료 구현
- [ ] 로그인 실패 제한과 감사 로그 추가
- [ ] 인증/인가 통합 테스트 추가

완료 조건:

- 잘못된 비밀번호와 권한 없는 명령이 거부된다.
- 서버 재시작 후에도 계정과 그룹 설정이 유지된다.

## P1: 핵심 수집 데이터 완성

### 6. 현재 폐기되는 Pack 처리

- [ ] `StackPack` core, 저장소, 조회 구현
- [ ] `BatchPack` core, 저장소, 조회 구현
- [ ] `InteractionPerfCounterPack` cache, 저장소, 조회 구현
- [ ] `SpanPack` / `SpanContainerPack` 저장소와 조회 구현
- [ ] `MapPack`의 수신 문맥과 처리 방식 확인 후 구현
- [ ] `DroppedXLogPack` 처리 방식을 Java 구현과 일치시킴
- [ ] XLog/Profile delaying cache 및 복구 처리 구현

완료 조건:

- 디코딩된 지원 Pack을 로그만 남기고 버리는 dispatcher 분기가 없다.
- 각 Pack에 수신부터 조회까지 이어지는 통합 테스트가 있다.

### 7. DB 포맷 호환성과 장애 복구

- [ ] Java가 만든 DB를 Rust가 조회하는 fixture 테스트
- [ ] Rust가 만든 DB를 Java가 조회하는 fixture 테스트
- [ ] XLog index/data 포맷 호환성 검증
- [ ] Profile index/data 포맷 호환성 검증
- [ ] Text 영구/일별 DB 및 중복 제거 구현
- [ ] Realtime/Daily counter 포맷 구현 및 검증
- [ ] Object, Status, Span, Batch, Visitor 저장소 구현
- [ ] 부분 write와 손상된 tail 복구
- [ ] 비정상 종료 후 index 복구
- [ ] 날짜 변경 중 write/read 테스트
- [ ] 동시 read/write 테스트
- [ ] index 용량 확장 테스트
- [ ] disk full 및 권한 오류 처리
- [ ] 파일 open의 truncate/append 의도를 명시하고 검증

완료 조건:

- Java/Rust DB fixture의 주요 조회 결과가 동일하다.
- 강제 종료 후 재시작해도 완료된 레코드를 조회할 수 있다.

### 8. Counter 기능 완성

- [ ] realtime total/group/object/object-type 조회
- [ ] realtime multi/all-multi 조회
- [ ] past time/all/total/group 조회
- [ ] past date total 및 long-date 조회
- [ ] today total 조회
- [ ] counter map realtime 조회
- [ ] counter 존재 날짜 조회
- [ ] DailyCounter aggregation
- [ ] 5분 sampling 및 meter counter 동작
- [ ] Java 응답과 binary 호환 테스트

### 9. XLog과 Profile 기능 완성

- [ ] realtime/load-time 기본 명령 구현
- [ ] load-time-group V2 구현
- [ ] full profile 조회
- [ ] load by txids 구현
- [ ] quick search 구현
- [ ] 조건 기반 XLog search 구현
- [ ] GXID/TXID index edge case 검증
- [ ] XLog group 성능 통계 구현
- [ ] profile step 전체 타입 호환성 검증

### 10. Object와 원격 agent 기능

- [ ] object today full list 구현
- [ ] inactive/remove/remove-in-memory 구현
- [ ] object environment 조회
- [ ] active service list 및 group 조회
- [ ] thread list/detail/dump/control 구현
- [ ] heap histogram 및 system GC 구현
- [ ] class/file/socket 관련 원격 요청 구현 범위 결정
- [ ] agent timeout, wakeup, reconnect 동작 검증
- [ ] remote control과 job 상태 관리 구현

## P1: 클라이언트 서비스 호환성

### 11. Config 서비스

- [ ] 서버 설정 전체 조회/수정/목록 구현
- [ ] 설정 값 타입/설명 서비스 구현
- [ ] WAS 설정 조회/수정/목록 구현
- [ ] Telegraf 설정 조회/수정 구현
- [ ] counter site 설정 구현
- [ ] 설정 파일 atomic write
- [ ] 설정 hot reload 및 observer 구현

### 12. Alert 기능

- [ ] alert title count와 summary 구현
- [ ] alert rule engine 구현
- [ ] scripting config/script 조회 및 저장
- [ ] script compile/load 오류 조회
- [ ] real counter description 구현
- [ ] alert plugin 호출 지점 구현

### 13. 추가 서비스

- [ ] Visitor realtime/history/group/total
- [ ] TagCount 저장과 조회
- [ ] Stack analyzer index/detail
- [ ] Batch active/history/detail/stack
- [ ] Host process/disk/memory/network 명령
- [ ] Database/MariaDB 명령
- [ ] CUBRID 명령
- [ ] server log/thread/debug 명령
- [ ] DB delete 관리 명령
- [ ] export 명령

각 서비스군은 실제 사용하는 Scouter Client 화면과 명령 로그를 기준으로 세부 우선순위를 정한다.

## P2: 운영 및 확장 기능

### 14. 관측성과 운영 안정성

- [ ] 수신 packet 수/bytes/drop/decode error 메트릭
- [ ] core별 queue depth/drop/processing latency 메트릭
- [ ] DB write/read/flush latency와 오류 메트릭
- [ ] TCP 명령별 요청 수, 오류, latency 메트릭
- [ ] 서버 CPU/메모리/open file/thread/task 메트릭
- [ ] health/readiness endpoint
- [ ] 구조화 로그와 request ID
- [ ] slow request 및 rate-limited 오류 로그

### 15. 데이터 보존 정책

- [ ] 날짜 기반 삭제 테스트
- [ ] 최대 DB 용량 기준 삭제
- [ ] 최소 디스크 여유 공간 기준 삭제
- [ ] 활성 DailyContainer와 cleanup race 방지
- [ ] 삭제 dry-run 및 운영 로그
- [ ] 잘못된 날짜/심볼릭 링크 등 안전성 검증

### 16. 플러그인과 부가 기능

- [ ] Rust plugin 인터페이스 설계
- [ ] Alert/Counter/XLog/Profile/Text/Summary/Object hook
- [ ] plugin timeout, panic 격리, backpressure 정책
- [ ] built-in plugin 지원 범위 결정
- [ ] GeoIP 지원 범위 결정
- [ ] Kubernetes Pod sequence manager
- [ ] terminal 관리 도구 지원 범위 결정

## P2: 성능 검증과 최적화

### 17. Benchmark 기반선

- [ ] 재현 가능한 agent traffic dataset 준비
- [ ] Java 서버 기준 benchmark 측정
- [ ] Rust 서버 동일 조건 benchmark 측정
- [ ] UDP packets/sec 및 loss 측정
- [ ] Counter/XLog/Profile write throughput 측정
- [ ] TCP query p50/p95/p99 측정
- [ ] idle 및 부하 상태 메모리 측정
- [ ] 24시간 soak test
- [ ] flamegraph/프로파일 기반 병목 기록

최적화는 benchmark로 병목이 확인된 부분부터 수행한다.

### 18. 성능 개선 후보

- [ ] blocking file I/O와 Tokio runtime 분리 검증
- [ ] 파일별 mutex 경합 측정 및 축소
- [ ] serialization allocation 축소
- [ ] cache 크기 상한과 eviction 정책
- [ ] queue 크기와 batching 조정
- [ ] flush/fsync 정책과 내구성 수준 문서화
- [ ] mmap/direct I/O 적용 필요성 측정

## 프로젝트 품질과 배포

### 19. 코드 품질과 CI

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] protocol/DB integration test를 CI에 추가
- [ ] dependency audit 추가
- [ ] unsafe code 사용 정책 정의
- [ ] fuzzing 대상 선정 및 CI/정기 실행 구성

### 20. 문서와 배포

- [ ] README 작성
- [ ] 지원 Pack/명령 호환성 표 작성
- [ ] 설정 항목과 기본값 문서화
- [ ] Java 서버에서 이전하는 절차 작성
- [ ] DB 호환/비호환 및 rollback 절차 작성
- [ ] Docker 이미지
- [ ] systemd 서비스 예제
- [ ] release build 및 artifact 생성 자동화
- [ ] 버전 정책과 CHANGELOG 작성

## 매 작업 공통 완료 절차

각 체크박스를 완료하기 전에 다음을 수행한다.

1. Java/Scala 원본 동작과 프로토콜을 확인한다.
2. 실패하는 테스트 또는 재현 절차를 먼저 만든다.
3. 최소 범위로 구현한다.
4. `cargo fmt --check`를 실행한다.
5. `cargo clippy --all-targets -- -D warnings`를 실행한다.
6. `cargo test`를 실행한다.
7. 필요하면 Java agent/client 통합 테스트를 실행한다.
8. 이 문서의 체크박스와 관련 설명을 갱신한다.

## 작업 기록

완료한 작업은 아래에 날짜, 변경 내용, 검증 결과를 기록한다.

| 날짜 | 작업 | 검증 |
|---|---|---|
| 2026-08-14 | 초기 TODO와 기준선 작성 | `cargo test`: 32 passed |
