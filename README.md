# Scouter Server Rust

Scouter Server 2.21.3의 핵심 수집 및 조회 경로를 Rust/Tokio로 이식하는 프로젝트다.
개발 진행 상황과 남은 호환성 작업은 [TODO.md](TODO.md)에서 관리한다.

## 현재 확인된 동작

- UDP/TCP `6100` listener
- Scouter Host Agent 2.21.3 TCP Agent V1 연결과 Object 등록
- Scouter Client 2.21.3 로그인
- Object 목록 표시
- Host CPU 실시간 차트
- 종료 시 listener, 입력 processor, core queue 순차 종료 및 DB flush

현재는 개발 중인 호환성 구현이며 기존 Scouter Server의 모든 명령과 DB 형식을 지원하지 않는다.

## 개발 명령

```shell
cargo run
cargo test
cargo check
cargo clippy --all-targets -- -D warnings
```

기본 설정은 `conf/scouter.toml`을 사용한다. Counter XML은 현재 로컬 Scouter 원본 저장소를 참조하며,
독립 배포 리소스로 포함하는 작업이 남아 있다.

## 변경 기록

### 2026-08-14 — feature/server-lifecycle

- HTTP 비활성화 상태에서도 UDP/TCP 서버가 유지되도록 lifecycle supervision을 수정했다.
- UDP/TCP/HTTP listener, monitor, cleaner, UDP processor와 core worker에 graceful shutdown을 연결했다.
- 입력을 먼저 중단한 뒤 core queue를 drain하고 DB를 flush하도록 종료 순서를 보장했다.
- DB flush 및 task join 오류가 누락되지 않도록 처리했다.
- 실제 Host Agent와 Client를 연결해 로그인, Object 목록, CPU 실시간 counter를 검증했다.
- TCP client blocking mode, counter XML, ObjectPack, CHECK_JOB 및 counter 응답 형식의 호환 문제를 수정했다.
- `cargo test`, `cargo check`와 실제 Client/Agent smoke test로 검증했다.
