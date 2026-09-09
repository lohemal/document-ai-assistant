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
