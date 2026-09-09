# List every TCP connection the app and its child processes hold, and flag the
# ones that leave this machine.
#
# The WebView (msedgewebview2.exe) runs as a child process, so looking only at
# DocAid.exe shows nothing -- that was the first mistake when checking this.
#
#   powershell -File ui/net-check.ps1 -Out net.txt
#
# Note: `npm run app` (dev) legitimately talks to 127.0.0.1:1420 (the vite dev
# server). That is loopback, so it does not count as leaving the machine, but
# it does not exist in a release build either. The decisive test is the
# firewall test on an installed build (design doc 10-6).

param([string]$Out = "net.txt")

$lines = New-Object System.Collections.Generic.List[string]

$root = Get-Process -Name "DocAid" -ErrorAction SilentlyContinue
if (-not $root) { Write-Error "no DocAid process"; exit 1 }

# Walk the process tree.
#
# ParentProcessId alone is NOT enough. Windows reuses PIDs, so an unrelated
# process can name a dead parent whose PID this app happens to hold now --
# that pulled Outlook and a GPU service into an earlier run of this check and
# made it look like the app was talking to the internet. A child must also
# have started AFTER its parent.
$all = Get-CimInstance Win32_Process
$born = @{}
foreach ($p in $all) { $born[[int]$p.ProcessId] = $p.CreationDate }

$ids = New-Object System.Collections.Generic.HashSet[int]
foreach ($p in $root) { [void]$ids.Add($p.Id) }

for ($i = 0; $i -lt 6; $i++) {
  foreach ($p in $all) {
    $pid_ = [int]$p.ProcessId
    $parent = [int]$p.ParentProcessId
    if ($ids.Contains($pid_)) { continue }
    if (-not $ids.Contains($parent)) { continue }
    $parentBorn = $born[$parent]
    if ($parentBorn -and $p.CreationDate -and $p.CreationDate -lt $parentBorn) {
      # Started before its "parent" -- the PID was reused. Not our child.
      continue
    }
    [void]$ids.Add($pid_)
  }
}

$lines.Add("processes in the app tree:")
foreach ($id in ($ids | Sort-Object)) {
  $p = $all | Where-Object { $_.ProcessId -eq $id } | Select-Object -First 1
  if ($p) { $lines.Add(("  {0,-8} {1}" -f $id, $p.Name)) }
}
$lines.Add("")

$conns = Get-NetTCPConnection -ErrorAction SilentlyContinue |
         Where-Object { $ids.Contains([int]$_.OwningProcess) }

$lines.Add("tcp connections:")
if (-not $conns) { $lines.Add("  (none)") }
foreach ($c in $conns) {
  $lines.Add(("  {0,-12} {1}:{2} -> {3}:{4}" -f $c.State, $c.LocalAddress, $c.LocalPort, $c.RemoteAddress, $c.RemotePort))
}
$lines.Add("")

$loopback = @('127.0.0.1', '::1', '0.0.0.0', '::')
$external = $conns | Where-Object { $loopback -notcontains $_.RemoteAddress }

$lines.Add("connections leaving this machine:")
if (-not $external) {
  $lines.Add("  NONE")
} else {
  foreach ($c in $external) {
    $lines.Add(("  {0,-12} {1}:{2}" -f $c.State, $c.RemoteAddress, $c.RemotePort))
  }
}

[System.IO.File]::WriteAllLines($Out, $lines, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "wrote $Out"
if ($external) { exit 3 }
