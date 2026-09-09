# Move/resize the app window, and scroll the pane under the mouse.
#
#   powershell -File ui/window.ps1 -Fit
#   powershell -File ui/window.ps1 -ScrollAt 800,600 -Ticks -5
#
# Screenshots are useless when the window hangs off the screen edge, and the
# interesting part of a long page is usually below the fold.

param(
  [switch]$Fit,
  [string]$ScrollAt = "",
  [int]$Ticks = 0
)

Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool repaint);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, int d, int e);
}
"@

$proc = Get-Process -Name "DocAid" -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { Write-Error "no DocAid window"; exit 1 }
$h = $proc.MainWindowHandle

if ($Fit) {
  $screen = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
  $w = [Math]::Min(1400, $screen.Width)
  $ht = [Math]::Min(1000, $screen.Height)
  [void][W]::MoveWindow($h, $screen.X, $screen.Y, $w, $ht, $true)
  Start-Sleep -Milliseconds 500
  Write-Host ("fit to " + $w + "x" + $ht + " at " + $screen.X + "," + $screen.Y)
}

if ($ScrollAt -ne "" -and $Ticks -ne 0) {
  [void][W]::SetForegroundWindow($h)
  Start-Sleep -Milliseconds 300
  $xy = $ScrollAt.Split(',')
  [void][W]::SetCursorPos([int]$xy[0], [int]$xy[1])
  Start-Sleep -Milliseconds 200
  # MOUSEEVENTF_WHEEL = 0x0800, one notch = 120
  for ($i = 0; $i -lt [Math]::Abs($Ticks); $i++) {
    [W]::mouse_event(0x0800, 0, 0, $(if ($Ticks -lt 0) { -120 } else { 120 }), 0)
    Start-Sleep -Milliseconds 90
  }
  Start-Sleep -Milliseconds 500
  Write-Host "scrolled $Ticks at $ScrollAt"
}
