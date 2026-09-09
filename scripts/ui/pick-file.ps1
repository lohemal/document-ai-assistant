# Type a path into the Windows file-open dialog and confirm it.
# Run this right after the app opens a file dialog.
# ASCII only -- so the path must be ASCII too (copy fixtures to a temp folder).
#   powershell -File ui/pick-file.ps1 -Path C:\Temp\a.pdf

param(
  [string]$Path,
  # Give the folder and file name separately when the caller's shell mangles
  # backslashes (git-bash does).
  [string]$Dir,
  [string]$Name
)

if (-not $Path) {
  if (-not $Dir -or -not $Name) { Write-Error "give -Path, or -Dir and -Name"; exit 1 }
  $Path = Join-Path $Dir $Name
}
# The Windows common dialog does not accept forward slashes in its name box.
$Path = $Path -replace '/', '\'

Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class Fg {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  public static string Cls() {
    var sb = new StringBuilder(64);
    GetClassName(GetForegroundWindow(), sb, 64);
    return sb.ToString();
  }
}
"@

# Wait until the Windows common dialog is actually in front. Its window class
# is #32770. Typing before it appears sends the path into the app instead.
$deadline = (Get-Date).AddSeconds(15)
while ((Get-Date) -lt $deadline -and [Fg]::Cls() -ne '#32770') {
  Start-Sleep -Milliseconds 250
}
if ([Fg]::Cls() -ne '#32770') { Write-Error "file dialog did not open"; exit 1 }
Start-Sleep -Milliseconds 300

# The file name box has focus when the dialog opens.
[System.Windows.Forms.SendKeys]::SendWait($Path)
Start-Sleep -Milliseconds 600

# One Enter is not always enough: the name box shows an autocomplete list and
# the first Enter can go to that list instead of the dialog. So press until the
# dialog is really gone, and say so only when it is.
$ok = $false
for ($i = 0; $i -lt 4; $i++) {
  [System.Windows.Forms.SendKeys]::SendWait("{ENTER}")
  Start-Sleep -Milliseconds 900
  if ([Fg]::Cls() -ne '#32770') { $ok = $true; break }
}
if (-not $ok) { Write-Error "dialog did not close for: $Path"; exit 2 }
Write-Host "picked: $Path"
