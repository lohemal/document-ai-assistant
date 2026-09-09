# Choose the Nth option of the Nth combo box (both 0-based).
#
# An HTML <select> shows up as a ComboBox whose options are ListItems. The
# options are not in the tree until the box is expanded, so expand first,
# then select, then collapse. React sees a real change event this way.
#
#   powershell -File ui/pick-combo.ps1 -Combo 0 -Option 2
#   powershell -File ui/pick-combo.ps1 -Combo 0 -List      # print the options
param(
  [int]$Combo = 0,
  [int]$Option = 0,
  [switch]$List,
  [int]$Down = -1
)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$proc = Get-Process -Name "DocAid" -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { Write-Error "no DocAid window"; exit 1 }

$root = [System.Windows.Automation.AutomationElement]::FromHandle($proc.MainWindowHandle)
# Wake the accessibility tree up
$null = $root.FindAll([System.Windows.Automation.TreeScope]::Subtree,
                      [System.Windows.Automation.Condition]::TrueCondition)

$cond = New-Object System.Windows.Automation.PropertyCondition(
  [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
  [System.Windows.Automation.ControlType]::ComboBox)
$boxes = $root.FindAll([System.Windows.Automation.TreeScope]::Subtree, $cond)
if ($boxes.Count -le $Combo) { Write-Error "combo $Combo not found (have $($boxes.Count))"; exit 1 }
$box = $boxes.Item($Combo)

if ($Down -ge 0) {
  Add-Type -AssemblyName System.Windows.Forms
  $box.SetFocus()
  Start-Sleep -Milliseconds 200
  # go to the top first so the result does not depend on what was selected
  [System.Windows.Forms.SendKeys]::SendWait("{HOME}")
  Start-Sleep -Milliseconds 120
  for ($i = 0; $i -lt $Down; $i++) {
    [System.Windows.Forms.SendKeys]::SendWait("{DOWN}")
    Start-Sleep -Milliseconds 120
  }
  Write-Host "moved down $Down"
  exit 0
}

try {
  $ec = $box.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
  $ec.Expand()
  Start-Sleep -Milliseconds 200
} catch { }

$icond = New-Object System.Windows.Automation.PropertyCondition(
  [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
  [System.Windows.Automation.ControlType]::ListItem)
$items = $box.FindAll([System.Windows.Automation.TreeScope]::Subtree, $icond)

if ($List) {
  $out = New-Object System.Collections.Generic.List[string]
  for ($i = 0; $i -lt $items.Count; $i++) {
    $out.Add(("{0} {1}" -f $i, $items.Item($i).Current.Name))
  }
  [System.IO.File]::WriteAllLines("combo.txt", $out, (New-Object System.Text.UTF8Encoding($false)))
  Write-Host "wrote combo.txt ($($items.Count) options)"
  exit 0
}

if ($items.Count -le $Option) { Write-Error "option $Option not found (have $($items.Count))"; exit 1 }
$item = $items.Item($Option)
try {
  $si = $item.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern)
  $si.Select()
  Write-Host "selected option $Option"
} catch {
  Write-Error "cannot select: $_"
  exit 1
}
try { $ec.Collapse() } catch { }

# --- keyboard fallback -------------------------------------------------
# WebView2 does not put <select> options in the tree until the native popup
# opens, so SelectionItemPattern finds nothing. With the select focused,
# arrow keys move the selection directly and fire a change event.
#
#   powershell -File ui/pick-combo.ps1 -Combo 0 -Down 2
