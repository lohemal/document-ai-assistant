# Click a text element by name, using a real mouse click.
#
# The name is read from a UTF-8 file, not written in this script -- Windows
# PowerShell 5.1 mangles non-ASCII literals in BOM-less .ps1 files.
#
#   echo -n "찾을 글자" > target.txt   (UTF-8, no BOM)
#   powershell -File ui/click-text.ps1 -NameFile target.txt

param(
  [Parameter(Mandatory = $true)][string]$NameFile,
  [int]$Occurrence = 0
)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Mouse {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, int e);
  public static void Click(int x, int y) {
    SetCursorPos(x, y);
    System.Threading.Thread.Sleep(120);
    mouse_event(0x0002, 0, 0, 0, 0);   // left down
    mouse_event(0x0004, 0, 0, 0, 0);   // left up
  }
}
"@

$name = [System.IO.File]::ReadAllText($NameFile, [System.Text.Encoding]::UTF8).Trim()

$proc = Get-Process -Name "DocAid" -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { Write-Error "no DocAid window"; exit 1 }

$root = [System.Windows.Automation.AutomationElement]::FromHandle($proc.MainWindowHandle)
$scope = [System.Windows.Automation.TreeScope]::Subtree
$true_ = [System.Windows.Automation.Condition]::TrueCondition
$null = $root.FindAll($scope, $true_)
Start-Sleep -Milliseconds 1000
$all = $root.FindAll($scope, $true_)

$hits = @()
foreach ($el in $all) { if ($el.Current.Name -eq $name) { $hits += $el } }
if ($hits.Count -eq 0) { Write-Error "not found"; exit 2 }
if ($Occurrence -ge $hits.Count) { Write-Error "occurrence out of range"; exit 3 }

$r = $hits[$Occurrence].Current.BoundingRectangle
if ($r.Width -le 0) { Write-Error "element has no size"; exit 4 }

$null = (New-Object -ComObject WScript.Shell).AppActivate($proc.Id)
Start-Sleep -Milliseconds 300
[Mouse]::Click([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2))
Start-Sleep -Milliseconds 700
Write-Host ("clicked at " + [int]($r.X + $r.Width / 2) + "," + [int]($r.Y + $r.Height / 2))
