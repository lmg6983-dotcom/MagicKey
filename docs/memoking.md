# ✦ MagicKey × 📝 메모킹

두 앱은 **따로 설치하고 따로 돌아가는 별개의 프로그램**이에요.
메모킹이 이 PC 에 있으면 MagicKey 에 메모 기능이 켜지고, 없으면 MagicKey 는 원래대로 동작해요.

서로의 코드를 가져다 쓰지 않아요. **명령줄 인자**와 **읽기만 하는 파일**로만 이야기해요.

## 🔌 약속

| 방향 | 무엇 | 어떻게 |
|---|---|---|
| MagicKey → 메모킹 설정 | 메모 폴더 위치 | `%APPDATA%\MemoKing\settings.json` 의 `vaultPath`. 비어 있으면 `문서\메모킹` |
| MagicKey → 메모킹 설정 | 매직키 단축키 · 꺼 둔 기능 | 같은 파일의 `shortcuts.magic` · `magic.modules` · `magic.paused` — MagicKey 에서 아직 바꾸지 않았을 때만 따라요 |
| MagicKey → 메모 폴더 | 메모 찾기 | `*.md` 를 **읽기만** (맨 위 `_memoking` · `attachments` 폴더, `.` · `~$` 로 시작하는 파일은 뺌) |
| MagicKey → 메모킹 | 메모 열기 | `%LOCALAPPDATA%\Programs\memoking\MemoKing.exe --open-memo=<id>` (꺼져 있으면 켜면서 연다) |
| MagicKey → 메모킹 | 새 메모 | `MemoKing.exe --new-memo-file=%TEMP%\magickey-*.txt [--open-new] [--remind]` — 메모킹 **0.77.0~** |

- 메모 파일 모양: `---` 머리말(`id` · `updated` …) `---` + 마크다운 본문. 첫 줄이 제목이에요.
- `id` 가 없는 파일은 메모킹이 아직 정리하지 않은 파일이라 건너뛰어요.
- 🔒 잠근 메모(`<!-- memoking-lock … -->`)는 제목만 보여 주고, 내용은 찾지 않아요.
- 메모 파일을 쓰는 일(새 메모 · 덧붙이기)은 **메모킹만** 해요. 머리말 형식 · 버전 기록 · 동기화가 꼬이지 않게요.
- 메모킹 실행 파일을 못 찾으면 메모 파일을 Windows 기본 앱으로 열어요.
- 새 메모: MagicKey 가 글을 `%TEMP%\magickey-<번호>.txt` 로 쓰고 넘기면, 메모킹이 읽어 메모로 만들고 그 파일을 지워요.
  - 메모킹은 **임시 폴더 바로 아래의 1MB 이하 `magickey-*.txt`** 만 받아요 (아무 파일이나 메모로 읽지 않게).
  - `--open-new` 가 있으면 만들고 바로 열고, 없으면 "새 메모를 만들었어요" 알림만 띄워요.
  - `--remind` 가 있으면 메모킹이 첫 줄의 첫 날짜(내일 오후 3시 · 다음 주 화요일 …)를 읽어 그때 알림을 맞춰요. 못 읽으면 알림 없이 저장.
  - 날짜를 정확히 읽는 일은 메모킹(`findDates`) 한 곳에서만 해요. MagicKey 는 날짜 같은 말이 보이면 "알림 맞춰 저장" 줄을 띄우기만 해요.
  - MagicKey 는 메모킹 설정의 `seenVersion`(마지막으로 켜진 버전)이 0.77.0 보다 낮으면 "새 메모" 줄을 숨겨요.

## 🛣️ 로드맵

- [x] **1단계** — MagicKey 에서 메모 찾기 · 메모킹으로 열기 (메모킹은 고칠 것 없음)
- [x] **2단계** — 새 메모 만들기: 메모킹에 `--new-memo-file=<임시 파일>` 인자를 더하고, MagicKey 의 "새 메모" 줄(맨 위, Enter 저장 · Ctrl+Enter 저장하고 열기)과 "알림 맞춰 저장" 줄이 그걸 불러요
- [ ] **3단계** — 메모킹 안의 매직키를 걷어내요
  - [x] **3-1** 설정 따라가기: MagicKey 에서 단축키 · 캡슐을 아직 바꾸지 않았으면 메모킹 매직키에서 쓰던 그대로 (옮겨 적지 않고 읽기만, 바꾸는 순간 MagicKey 설정으로)
  - [x] **3-2** 🏢 **OPMS 검색**(사내망)은 따로 설치하는 [MagicSystem](https://github.com/lmg6983-dotcom/MagicOPMS)(Rust · Tauri 2)로 떼어 내요.
    MagicKey 는 사건번호 · 이름 · 초성처럼 OPMS 에 물을 글이면 "OPMS 에서 찾기" 줄로 MagicSystem 을 불러요 — **기본값은 꺼짐**
    - 저절로 찾기: 온전한 사건번호 · MagicSystem 이 배운 번호 양식과 같은 꼴이면 고르기 전에 사건 6건 (MagicSystem 과 같은 코어 `magicsystem-core` 를 git 으로 가져다 씀)
  - [x] **3-4a** 메모킹 0.77: MagicKey 가 켜져 있으면(`%APPDATA%\MemoKing\magickey.json` 의 `exe` · `pid`) 매직키 단축키를 넘기고, 끄면 다시 잡아요. 없으면 지금처럼 + MagicKey 안내
  - [ ] **3-4b** 몇 버전 뒤: 메모킹에서 매직키 · OPMS · "모두 보기" 창 · 파일 목록을 걷어내요
  - 🤖 **매직 에이전트**(이 PC 의 카나나2 AI)는 옮기지 않아요 — 메모킹 목록 창 검색창 ✦ 에 그대로 남아요 (두 벌이 되면 서로 다르게 답할 수 있어서)
- [ ] **4단계** — 릴리스: MagicKey · MagicSystem 을 먼저, 메모킹을 그다음에 (매직키가 하나도 없는 때가 없게)
  - [x] MagicKey 자동 업데이트 (서명된 설치 파일 · `latest.json`)

## 📦 릴리스 순서

| 차례 | 무엇 | 왜 |
|---|---|---|
| 1 | **MagicKey v0.3.0** — 메모 찾기 · 새 메모 · 알림 맞춰 저장 · OPMS 줄 · 자동 업데이트 | 메모킹 0.77 이 키를 넘길 곳이 먼저 있어야 해요. 메모킹이 옛 버전이어도 새 메모 줄만 숨고 깨지지 않아요 |
| 2 | **MagicOPMS v0.1.0** (지금 이름 MagicSystem) | MagicKey 의 OPMS 줄이 부를 앱 (OPMS 줄은 기본값 꺼짐이라 늦어도 괜찮아요) |
| 3 | **메모킹 0.77.0** — `--new-memo-file` · `--remind` · MagicKey 가 있으면 매직키 넘기기 | 자동 업데이트로 바로 퍼져요. MagicKey 가 없는 사람은 지금처럼 메모킹 매직키를 써요 |
| 4 | 몇 버전 뒤 메모킹에서 매직키 · OPMS · 모두 보기 · 파일 목록 걷어내기 (3-4b) | MagicKey · MagicSystem 으로 옮겨 갈 시간을 두고 |
