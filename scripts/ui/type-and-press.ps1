# Type a name into the first text box and press the first button.
# ASCII only (PowerShell 5.1 would mangle non-ASCII literals in this file).
# Uses real keystrokes, not ValuePattern.SetValue -- React would not notice
# a value set directly on the DOM node.

param([string]$Text = "Test Collection A")

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms

$proc = Get-Process -Name "DocAid" -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { Write-Error "no DocAid window"; exit 1 }

$root = [System.Windows.Automation.AutomationElement]::FromHandle($proc.MainWindowHandle)
$scope = [System.Windows.Automation.TreeScope]::Subtree
$true_ = [System.Windows.Automation.Condition]::TrueCondition

# The WebView2 tree is built lazily: walk once to wake it, then walk again.
$null = $root.FindAll($scope, $true_)
Start-Sleep -Milliseconds 1200
$all = $root.FindAll($scope, $true_)

$edit = $null
$btn = $null
foreach ($el in $all) {
  $t = $el.Current.ControlType.ProgrammaticName
  if (-not $edit -and $t -eq "ControlType.Edit") { $edit = $el }
  if (-not $btn -and $t -eq "ControlType.Button") { $btn = $el }
}
if (-not $edit) { Write-Error "no text box"; exit 2 }
if (-not $btn) { Write-Error "no button"; exit 3 }

$null = (New-Object -ComObject WScript.Shell).AppActivate($proc.Id)
Start-Sleep -Milliseconds 400

$edit.SetFocus()
Start-Sleep -Milliseconds 300
[System.Windows.Forms.SendKeys]::SendWait($Text)
Start-Sleep -Milliseconds 600

Write-Host ("pressing: " + $btn.Current.Name)
$btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
Start-Sleep -Milliseconds 1000
Write-Host "done"
