# 앱을 사람 손 없이 조작하기

Tauri 의 WebView2 창은 Windows UI Automation 으로 다룰 수 있습니다.
사람이 눌러 보지 않고도 화면을 읽고 버튼을 누를 수 있어서,
**업데이트 시험(P8)** 과 **오프라인 검증(설계안 10-6)** 에 씁니다.

```powershell
# 앱을 띄운 뒤
npm run app:sandbox

# 화면에 뭐가 있는지 읽는다
powershell -ExecutionPolicy Bypass -File scripts/ui/dump.ps1 -Out dump.txt

# 첫 입력칸에 글자를 넣고 첫 버튼을 누른다
powershell -ExecutionPolicy Bypass -File scripts/ui/type-and-press.ps1 -Text "Test"

# N번째 버튼을 누른다 (0부터)
powershell -ExecutionPolicy Bypass -File scripts/ui/click.ps1 -Index 2
```

## 알아 둘 것 두 가지

**접근성 트리는 미루어 만들어집니다.** `FindAll(Subtree, TrueCondition)` 로 한 번
훑어 깨운 뒤 다시 찾아야 버튼이 보입니다. 스크립트가 이미 그렇게 합니다.

**스크립트 본문은 ASCII 로만 씁니다.** Windows PowerShell 5.1 은 BOM 없는 UTF-8
`.ps1` 을 ANSI 로 읽어서, 한글 문자열이 깨지고 파싱이 실패합니다.
그래서 버튼을 이름이 아니라 **번호로** 찾습니다. 화면에서 읽어 온 한글은
UTF-8 파일로 내보내고 다른 도구로 읽습니다.

## 자료집 고르기 (`pick-combo.ps1`)

WebView2 는 `<select>` 의 항목을 **팝업이 열릴 때까지 접근성 트리에 넣지
않습니다.** 그래서 `SelectionItemPattern` 으로는 아무것도 찾을 수 없습니다.
화살표 키로 고릅니다 — 맨 위로 보낸 뒤(`{HOME}`) 아래로 N번 내립니다.

```powershell
powershell -File ui/pick-combo.ps1 -Combo 0 -Down 2
```

`-Down` 은 화면마다 다릅니다. 검색 화면의 첫 항목은 `전체 자료` 이므로
자료집은 1부터 시작하고, 등록 화면은 자료집이 0부터 시작합니다.

## 오래 걸리는 일을 기다릴 때

색인(청크당 0.8초)과 답변(한 물음에 100초)은 UI 자동화보다 느립니다.
`dump.ps1` 을 되풀이해 부르며 기다릴 문구가 나오는지 봅니다.

```bash
for i in $(seq 1 40); do
  powershell -File scripts/ui/dump.ps1 -Out d.txt
  grep -q '검증 상태' d.txt && break
done
```

## 외부 통신 표본 (`net-watch.ps1`)

앱(`DocAid.exe`)과 그 자식 프로세스(WebView2)의 TCP 연결을 2초마다 떠서 CSV 로
남깁니다. 오프라인 검증(설계안 10-6·10-8)에 씁니다 — 뒤에서 돌려 두고 전 기능을 돕니다.

```bash
powershell -File scripts/ui/net-watch.ps1 -Seconds 420 -Out net.csv
# process,remote,state,samples_seen,total_samples
# DocAid,127.0.0.1:11434,Established,1,172      ← Ollama (루프백)
# DocAid,20.200.245.247:443,Established,1,172   ← [업데이트 확인] 을 눌렀을 때의 github.com
```

`127.0.0.1` 밖 주소는 전부 설명할 수 있어야 합니다. 앱만 켜 두었을 때 나가는 것이 있으면
실패입니다.
