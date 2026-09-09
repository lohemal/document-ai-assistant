# Put text into the Nth text box by pasting it from the clipboard.
#
# The text is read from a UTF-8 file, not written in this script -- Windows
# PowerShell 5.1 mangles non-ASCII literals in BOM-less .ps1 files.
#
# Pasting (not SendKeys) for two reasons: SendKeys cannot type Hangul, and a
# paste fires a real input event so React notices the change. Setting the DOM
# value through UI Automation would not.
#
#   powershell -File ui/paste.ps1 -TextFile q.txt -Index 0 [-Enter]

param(
  [Parameter(Mandatory = $true)][string]$TextFile,
  [int]$Index = 0,
  [switch]$Enter,
  [switch]$Clear
)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms

$text = [System.IO.File]::ReadAllText((Resolve-Path $TextFile), [System.Text.Encoding]::UTF8).TrimEnd("`r", "`n")

$proc = Get-Process -Name "DocAid" -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { Write-Error "no DocAid window"; exit 1 }

$root = [System.Windows.Automation.AutomationElement]::FromHandle($proc.MainWindowHandle)
$scope = [System.Windows.Automation.TreeScope]::Subtree
$true_ = [System.Windows.Automation.Condition]::TrueCondition
$null = $root.FindAll($scope, $true_)
Start-Sleep -Milliseconds 900
$all = $root.FindAll($scope, $true_)

$edits = @()
foreach ($el in $all) {
  if ($el.Current.ControlType.ProgrammaticName -eq "ControlType.Edit") { $edits += $el }
}
if ($Index -ge $edits.Count) { Write-Error "no text box at $Index (found $($edits.Count))"; exit 2 }

$null = (New-Object -ComObject WScript.Shell).AppActivate($proc.Id)
Start-Sleep -Milliseconds 400
$edits[$Index].SetFocus()
Start-Sleep -Milliseconds 300

if ($Clear) {
  [System.Windows.Forms.SendKeys]::SendWait("^a")
  Start-Sleep -Milliseconds 150
}

Set-Clipboard -Value $text
Start-Sleep -Milliseconds 250
[System.Windows.Forms.SendKeys]::SendWait("^v")
Start-Sleep -Milliseconds 500

if ($Enter) {
  [System.Windows.Forms.SendKeys]::SendWait("{ENTER}")
  Start-Sleep -Milliseconds 1200
}

Write-Host "pasted into box $Index"
