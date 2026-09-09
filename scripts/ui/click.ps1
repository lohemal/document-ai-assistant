# Press the Nth button (0-based) in the app window.
# ASCII only -- button names are Korean, so we address them by index.
#   powershell -File ui-click.ps1 -Index 2

param([int]$Index = 0)

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

$buttons = @()
foreach ($el in $all) {
  if ($el.Current.ControlType.ProgrammaticName -eq "ControlType.Button") { $buttons += $el }
}
Write-Host ("buttons: " + $buttons.Count)
if ($Index -ge $buttons.Count) { Write-Error "index out of range"; exit 2 }

$b = $buttons[$Index]
$b.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
Start-Sleep -Milliseconds 900
Write-Host "invoked"
