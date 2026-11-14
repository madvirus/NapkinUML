# NapkinUML

![NapkinUML](docs/napkinuml-logo-512-white.png)

NapkinUML은 개인적으로 사용하려고 생성한 UML 다이어그램 에디터입니다. 복잡하거나 화려하거나 특별한 기능 없이 간단하게 그릴 목적으로 만들었습니다.

## 지원 다이어그램

UML 9종:
* 클래스 다이어그램
* 컴포넌트 다이어그램
* 배치 다이어그램
* Use Case 다이어그램
* 시퀀스 다이어그램
* 스테이트 머신 다이어그램
* 액티비티 다이어그램
* 오브젝트 다이어그램
* 패키지 다이어그램

기타 2종:
* ER 다이어그램
* 네트워크 다이어그램

![class diagram](docs/napkinuml-shot.png)

## 실험 기능: AI 어시스턴스
로컬에 설치한 AI 에이전트를 이용해서 다이어그램 제작을 도움 받을 수 있습니다.

![AI assistance](docs/napkin-ai-assistance.png)

AI 에이전트(Claude Code, Codex CLI, Gemini CLI, Ollama 등)는 NapkinUML에 포함되어 있지 않습니다. 사용자가 직접 설치한 프로그램을 실행하며, 각 프로그램의 라이선스와 이용 약관을 따릅니다.

## 설치

[GitHub Releases](https://github.com/madvirus/NapkinUML)에서 운영체제에 맞는 파일을 내려받습니다. 별도 설치 과정 없이 바로 실행할 수 있습니다.

| 운영체제 | 파일 |
|---|---|
| Windows (x64) | `NapkinUML-x64.exe` |
| Windows (ARM64) | `NapkinUML-arm64.exe` |
| macOS (Apple Silicon / Intel) | `NapkinUML-macos.dmg` 또는 `NapkinUML-macos.zip` |

### 보안 경고 안내

NapkinUML은 개인 용도로 만든 프로그램이라 코드 서명 인증서로 서명하지 않았습니다. 그래서 다운로드하거나 처음 실행할 때 경고가 표시될 수 있습니다. 소스 코드는 이 저장소에 공개되어 있고, 배포 파일은 GitHub Actions(`.github/workflows/build.yml`)에서 빌드됩니다. 원하면 직접 빌드해서 사용해도 됩니다.

**Windows**

* 브라우저(Edge, Chrome 등)에서 "일반적으로 다운로드되지 않는 파일" 같은 경고가 뜨면 `...` 메뉴에서 **유지**(또는 **계속**)를 선택합니다.
* 실행할 때 "Windows의 PC 보호"(SmartScreen) 창이 뜨면 **추가 정보** → **실행**을 누릅니다.

**macOS**

* `.dmg`를 열고 NapkinUML 앱을 Applications 폴더로 드래그해서 복사합니다(`.zip`은 압축을 풀어 옮깁니다).
* 앱은 ad-hoc 서명만 되어 있어서 처음 실행할 때 "확인되지 않은 개발자" 또는 "열 수 없습니다" 경고가 뜹니다.
* macOS 14 이하: Finder에서 앱을 **우클릭(Control+클릭) → 열기**를 선택한 뒤 다시 **열기**를 누릅니다.
* macOS 15 이상: 앱 실행을 한 번 시도한 뒤 **시스템 설정 → 개인정보 보호 및 보안**에서 **그래도 열기**를 누릅니다.
* 또는 터미널에서 격리 속성을 제거해도 됩니다.
  ```sh
  xattr -dr com.apple.quarantine /Applications/NapkinUML.app
  ```

## 라이선스

NapkinUML의 소스 코드는 다음 두 라이선스 중 하나를 선택해 사용할 수 있습니다.

* [Apache License 2.0](LICENSE-APACHE)
* [MIT License](LICENSE-MIT)

NapkinUML 이름, 로고와 아이콘(`icons/`, `docs/`의 이미지)은 위 라이선스에 포함되지 않습니다. 수정한 버전을 배포할 때는 다른 이름과 아이콘을 사용해 주세요.

### 서드파티 오픈소스

NapkinUML은 egui, winit, serde, image, rfd 등 오픈소스 Rust 크레이트를 사용하며, 배포 파일에는 egui에 내장된 폰트(Hack, Ubuntu, Noto Emoji 등, SIL OFL 1.1 및 Ubuntu Font Licence)가 포함됩니다. 의존성 목록과 라이선스 전문은 릴리즈에 함께 올라가는 `THIRD-PARTY-LICENSES.html`(macOS 앱은 `NapkinUML.app/Contents/Resources/` 안)에서 확인할 수 있습니다.

새 의존성은 허용 라이선스(MIT, Apache-2.0, BSD, ISC, Zlib 등 비 카피레프트)만 사용할 수 있으며, `deny.toml`에 정의된 정책을 CI에서 `cargo deny check licenses`로 검사합니다.
