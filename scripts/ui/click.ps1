# Press the Nth control of a given type (0-based) in the app window.
# ASCII only -- control names are Korean, so we address them by index.
#   powershell -File ui/click.ps1 -Index 2
#   powershell -File ui/click.ps1 -Type Hyperlink -Index 1

param(
  [int]$Index = 0,
  [string]$Type = 'Button',
  [switch]$List,
  # Wait until the control is enabled. The app disables buttons while it is
  # busy (registering a PDF, for example) -- clicking a disabled button does
  # nothing and the rest of the script then runs out of step.
  [int]$WaitSeconds = 0
)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$proc = Get-Process -Name "DocAid" -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { Write-Error "no DocAid window"; exit 1 }

$root = [System.Windows.Automation.AutomationElement]::FromHandle($proc.MainWindowHandle)
$scope = [System.Windows.Automation.TreeScope]::Subtree
$true_ = [System.Windows.Automation.Condition]::TrueCondition
$null = $root.FindAll($scope, $true_)
Start-Sleep -Milliseconds 1000
$all = $root.FindAll($scope, $true_)

$wanted = "ControlType." + $Type
$hits = @()
foreach ($el in $all) {
  if ($el.Current.ControlType.ProgrammaticName -eq $wanted) { $hits += $el }
}
Write-Host ("$Type count: " + $hits.Count)

if ($List) {
  for ($i = 0; $i -lt $hits.Count; $i++) {
    Write-Host ("  [$i] " + $hits[$i].Current.AutomationId)
  }
  exit 0
}

if ($Index -ge $hits.Count) { Write-Error "index out of range"; exit 2 }

$b = $hits[$Index]

if ($WaitSeconds -gt 0) {
  $deadline = (Get-Date).AddSeconds($WaitSeconds)
  while (-not $b.Current.IsEnabled -and (Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    $all = $root.FindAll($scope, $true_)
    $hits = @()
    foreach ($el in $all) {
      if ($el.Current.ControlType.ProgrammaticName -eq $wanted) { $hits += $el }
    }
    if ($Index -lt $hits.Count) { $b = $hits[$Index] }
  }
  if (-not $b.Current.IsEnabled) { Write-Error "still disabled after $WaitSeconds s"; exit 5 }
}
$pattern = $b.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
$pattern.Invoke()
Start-Sleep -Milliseconds 900
Write-Host "invoked"
