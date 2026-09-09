# Dump the accessibility tree of the running app window.
# ASCII only: Windows PowerShell 5.1 reads BOM-less UTF-8 scripts as ANSI and
# would mangle any non-ASCII literal in this file.
#
#   powershell -ExecutionPolicy Bypass -File ui-dump.ps1 -Out dump.txt [-Click "button name"]

param(
  [string]$Out = "dump.txt",
  [string]$Click = ""
)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$proc = Get-Process -Name "DocAid" -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } |
        Select-Object -First 1
if (-not $proc) { Write-Error "no DocAid window found"; exit 1 }

$root = [System.Windows.Automation.AutomationElement]::FromHandle($proc.MainWindowHandle)
if (-not $root) { Write-Error "cannot attach to window"; exit 1 }

$scope = [System.Windows.Automation.TreeScope]::Subtree
$true_ = [System.Windows.Automation.Condition]::TrueCondition

# The WebView2 accessibility tree is built lazily. Walk it once to wake it up,
# then walk again -- the first pass usually returns almost nothing.
$null = $root.FindAll($scope, $true_)
Start-Sleep -Milliseconds 1200
$all = $root.FindAll($scope, $true_)

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("window: " + $root.Current.Name)
$lines.Add("elements: " + $all.Count)
$lines.Add("")

foreach ($el in $all) {
  $name = $el.Current.Name
  if ([string]::IsNullOrWhiteSpace($name)) { continue }
  $type = $el.Current.ControlType.ProgrammaticName -replace "ControlType.", ""
  $lines.Add(("{0,-12} {1}" -f $type, $name))
}

[System.IO.File]::WriteAllLines($Out, $lines, (New-Object System.Text.UTF8Encoding($false)))
Write-Host ("wrote " + $lines.Count + " lines to " + $Out)

if ($Click -ne "") {
  $target = $null
  foreach ($el in $all) {
    if ($el.Current.Name -eq $Click) { $target = $el; break }
  }
  if (-not $target) { Write-Error ("no element named: " + $Click); exit 2 }
  $pattern = $target.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
  $pattern.Invoke()
  Write-Host "invoked"
}
