# Sample the TCP connections of one or more process trees for a while and write
# every distinct (process, remote address, state) seen: how many samples it
# appeared in, and when it was first and last seen. Used for the offline
# verification (design doc 10-6 / 10-8): run this in the background while
# exercising every feature, then read the file.
#
#   powershell -File ui/net-watch.ps1 -Seconds 600 -Out net-watch.csv
#   powershell -File ui/net-watch.ps1 -ProcessName DocAid,ollama* -StopOnKey
#
# -ProcessName takes several names (wildcards allowed). Each name is a root;
# its child processes (WebView2 under DocAid) are followed too.
# -StopOnKey ends the run early when any key is pressed in the console
# (ignored when there is no console, e.g. under a redirected stdin).
#
# Loopback (127.0.0.1 / ::1) rows are kept -- they are the Ollama and IPC traffic
# and prove the app is talking to the local engine only. Anything else is what
# the report has to explain: WebView2's own Microsoft traffic (msedgewebview2.exe)
# versus the app's (DocAid.exe) versus Ollama's (ollama.exe, "ollama app.exe").
#
# ASCII only (PowerShell 5.1 mangles non-ASCII in BOM-less scripts).

param(
  [int]$Seconds = 300,
  [string]$Out = "net-watch.csv",
  [string[]]$ProcessName = @("DocAid"),
  [switch]$StopOnKey
)

$seen = @{}
$samples = 0
$deadline = (Get-Date).AddSeconds($Seconds)

function ChildIds([int[]]$roots) {
  $all = Get-CimInstance Win32_Process
  $born = @{}
  foreach ($p in $all) { $born[[int]$p.ProcessId] = $p.CreationDate }
  $ids = New-Object System.Collections.Generic.HashSet[int]
  foreach ($r in $roots) { [void]$ids.Add($r) }
  for ($i = 0; $i -lt 6; $i++) {
    foreach ($p in $all) {
      $pid_ = [int]$p.ProcessId
      $parent = [int]$p.ParentProcessId
      if ($ids.Contains($pid_)) { continue }
      if (-not $ids.Contains($parent)) { continue }
      $pb = $born[$parent]
      if ($pb -and $p.CreationDate -and $p.CreationDate -lt $pb) { continue }  # reused PID
      [void]$ids.Add($pid_)
    }
  }
  return $ids
}

function KeyPressed() {
  try {
    if ([Console]::KeyAvailable) { [void][Console]::ReadKey($true); return $true }
  } catch { }
  return $false
}

Write-Output ("watching {0} for up to {1}s (2s samples){2}" -f ($ProcessName -join ", "), $Seconds, $(if ($StopOnKey) { " -- press any key to stop" } else { "" }))

while ((Get-Date) -lt $deadline) {
  if ($StopOnKey -and (KeyPressed)) { Write-Output "stopped by key"; break }

  $roots = @()
  foreach ($n in $ProcessName) {
    $roots += @(Get-Process -Name $n -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
  }
  if ($roots.Count -gt 0) {
    $ids = ChildIds $roots
    $names = @{}
    foreach ($id in $ids) {
      $p = Get-Process -Id $id -ErrorAction SilentlyContinue
      if ($p) { $names[$id] = $p.ProcessName }
    }
    $conns = Get-NetTCPConnection -ErrorAction SilentlyContinue | Where-Object { $ids.Contains([int]$_.OwningProcess) }
    $now = (Get-Date).ToString("HH:mm:ss")
    foreach ($c in $conns) {
      if ($c.RemoteAddress -eq "0.0.0.0" -or $c.RemoteAddress -eq "::") { continue }
      $key = "{0}|{1}:{2}|{3}" -f $names[[int]$c.OwningProcess], $c.RemoteAddress, $c.RemotePort, $c.State
      if ($seen.ContainsKey($key)) {
        $seen[$key].count++
        $seen[$key].last = $now
      } else {
        $seen[$key] = @{ count = 1; first = $now; last = $now }
      }
    }
    $samples++
  }
  Start-Sleep -Seconds 2
}

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("process,remote,state,samples_seen,total_samples,first_seen,last_seen")
foreach ($k in ($seen.Keys | Sort-Object)) {
  $parts = $k -split '\|'
  $v = $seen[$k]
  $lines.Add(("{0},{1},{2},{3},{4},{5},{6}" -f $parts[0], $parts[1], $parts[2], $v.count, $samples, $v.first, $v.last))
}
[System.IO.File]::WriteAllLines($Out, $lines, (New-Object System.Text.UTF8Encoding($false)))
Write-Output ("wrote {0} rows from {1} samples to {2}" -f $seen.Count, $samples, $Out)
