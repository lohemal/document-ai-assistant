# Offline verification recorder (design doc 10-6 / 10-8, procedure in
# docs/03-offline checklist). Run this AFTER turning Wi-Fi (airplane mode) on,
# BEFORE launching DocAid, then walk the checklist. Press any key when done.
#
#   powershell -ExecutionPolicy Bypass -File scripts\ui\offline-run.ps1
#
# What it leaves in C:\Temp\docaid-p8\offline\<yyyyMMdd-HHmmss>\ :
#   net-state-start.txt  adapters, gateway, ping, DNS, Ollama version + models  (proof of "offline")
#   net-watch.csv        every TCP connection of DocAid (+WebView2) and ollama (+tray app), with times
#   net-state-end.txt    same as start, taken when you press a key
#   summary.txt          the CSV split into the four report categories and a PASS/FAIL line
#
# Messages are English on purpose: PowerShell 5.1 breaks non-ASCII in .ps1 files.

param(
  [string]$OutRoot = "C:\Temp\docaid-p8\offline",
  [int]$Seconds = 5400,
  [switch]$NoKeyStop
)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$stamp = (Get-Date).ToString("yyyyMMdd-HHmmss")
$dir = Join-Path $OutRoot $stamp
New-Item -ItemType Directory -Force -Path $dir | Out-Null

function Section([System.Collections.Generic.List[string]]$l, [string]$title) {
  $l.Add(""); $l.Add("== $title ==");
}

function NetState([string]$path) {
  $l = New-Object System.Collections.Generic.List[string]
  $l.Add(("taken {0}" -f (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")))

  Section $l "adapters"
  try {
    foreach ($a in (Get-NetAdapter -ErrorAction Stop)) {
      $l.Add(("  {0,-28} {1,-14} {2}" -f $a.Name, $a.Status, $a.InterfaceDescription))
    }
  } catch { $l.Add("  (Get-NetAdapter failed: $_)") }

  Section $l "default gateways"
  $gw = @()
  try {
    foreach ($c in (Get-NetIPConfiguration -ErrorAction Stop)) {
      if ($c.IPv4DefaultGateway) { $gw += ("  {0} -> {1}" -f $c.InterfaceAlias, $c.IPv4DefaultGateway.NextHop) }
    }
  } catch { }
  if ($gw.Count -eq 0) { $l.Add("  NONE") } else { foreach ($g in $gw) { $l.Add($g) } }

  Section $l "ping 8.8.8.8"
  $ping = $false
  try { $ping = Test-Connection -ComputerName 8.8.8.8 -Count 1 -Quiet -ErrorAction SilentlyContinue } catch { }
  $l.Add("  " + $(if ($ping) { "REACHABLE" } else { "unreachable" }))

  Section $l "dns github.com"
  $dns = $false
  try {
    $r = Resolve-DnsName github.com -DnsOnly -ErrorAction Stop
    $dns = $true
    $l.Add("  RESOLVED " + (($r | Where-Object { $_.IPAddress } | ForEach-Object { $_.IPAddress }) -join ", "))
  } catch { $l.Add("  not resolved") }

  Section $l "ollama on 127.0.0.1:11434"
  try {
    $v = Invoke-RestMethod -Uri "http://127.0.0.1:11434/api/version" -TimeoutSec 3 -ErrorAction Stop
    $l.Add("  version " + $v.version)
    $t = Invoke-RestMethod -Uri "http://127.0.0.1:11434/api/tags" -TimeoutSec 5 -ErrorAction Stop
    foreach ($m in $t.models) { $l.Add(("  model {0,-24} {1,6:N1} GB" -f $m.name, ($m.size / 1GB))) }
  } catch { $l.Add("  NOT RUNNING ($_)") }

  Section $l "processes"
  foreach ($n in @("DocAid", "ollama", "ollama app", "msedgewebview2")) {
    $c = @(Get-Process -Name $n -ErrorAction SilentlyContinue).Count
    $l.Add(("  {0,-16} {1}" -f $n, $c))
  }

  [System.IO.File]::WriteAllLines($path, $l, (New-Object System.Text.UTF8Encoding($false)))
  return @{ online = ($ping -or $dns); text = ($l -join "`n") }
}

Write-Output "== DocAid offline verification recorder =="
Write-Output "output folder: $dir"
Write-Output ""

$start = NetState (Join-Path $dir "net-state-start.txt")
Write-Output $start.text
Write-Output ""

if ($start.online) {
  Write-Warning "Internet still looks REACHABLE. Turn Wi-Fi / airplane mode on first, then run this again."
  Write-Warning "(Recording continues anyway so nothing is lost, but the result will not count as offline.)"
}
if (@(Get-Process -Name DocAid -ErrorAction SilentlyContinue).Count -gt 0) {
  Write-Warning "DocAid is already running. Checklist step 1 is 'launch the app while offline' -- close it, then start it again now."
}
if (@(Get-Process -Name ollama -ErrorAction SilentlyContinue).Count -eq 0) {
  Write-Warning "ollama.exe is not running. Start it (tray app or 'ollama serve') before step 1."
}

Write-Output ""
Write-Output "Recording started. Now launch DocAid and walk the checklist."
Write-Output "When every step is done, come back to this window and PRESS ANY KEY."
Write-Output ""

$csv = Join-Path $dir "net-watch.csv"
$args_ = @{ Seconds = $Seconds; Out = $csv; ProcessName = @("DocAid", "ollama*") }
if (-not $NoKeyStop) { $args_.StopOnKey = $true }
& (Join-Path $here "net-watch.ps1") @args_

$end = NetState (Join-Path $dir "net-state-end.txt")

# ---- summary: split rows into the four report categories ----------------------
$rows = @()
if (Test-Path $csv) { $rows = @(Import-Csv $csv) }

function IsLoopback([string]$remote) {
  return ($remote -like "127.*" -or $remote -like "::1:*")
}

$catDocaidOllama = @(); $catDocaidExt = @(); $catWebviewExt = @(); $catOllamaExt = @(); $catOtherLoop = @()
foreach ($r in $rows) {
  $p = $r.process; $rem = $r.remote
  $loop = IsLoopback $rem
  if ($p -eq "DocAid" -and $loop -and $rem -like "*:11434") { $catDocaidOllama += $r; continue }
  if ($p -eq "DocAid" -and -not $loop) { $catDocaidExt += $r; continue }
  if ($p -eq "msedgewebview2" -and -not $loop) { $catWebviewExt += $r; continue }
  if ($p -like "ollama*" -and -not $loop) { $catOllamaExt += $r; continue }
  $catOtherLoop += $r
}

function Fmt($list) {
  if ($list.Count -eq 0) { return @("  NONE") }
  $o = @()
  foreach ($r in $list) {
    $o += ("  {0,-16} {1,-28} {2,-12} {3,4}/{4,-4} {5}..{6}" -f $r.process, $r.remote, $r.state, $r.samples_seen, $r.total_samples, $r.first_seen, $r.last_seen)
  }
  return $o
}

$s = New-Object System.Collections.Generic.List[string]
$s.Add("DocAid offline verification -- $stamp")
$s.Add(("offline at start: {0}   offline at end: {1}" -f (-not $start.online), (-not $end.online)))
$s.Add(("samples: {0}" -f $(if ($rows.Count -gt 0) { $rows[0].total_samples } else { 0 })))
$s.Add("")
$s.Add("[1] DocAid external (non-loopback)          -- must be NONE (an [Update check] click is the only excuse)")
foreach ($x in (Fmt $catDocaidExt)) { $s.Add($x) }
$s.Add("")
$s.Add("[2] DocAid -> localhost Ollama (127.0.0.1:11434) -- expected during search / answer / draft")
foreach ($x in (Fmt $catDocaidOllama)) { $s.Add($x) }
$s.Add("")
$s.Add("[3] WebView2 external (msedgewebview2)      -- must be NONE")
foreach ($x in (Fmt $catWebviewExt)) { $s.Add($x) }
$s.Add("")
$s.Add("[4] Ollama external (ollama / ollama app)   -- must be NONE unless a model download was clicked")
foreach ($x in (Fmt $catOllamaExt)) { $s.Add($x) }
$s.Add("")
$s.Add("[-] other loopback rows (IPC, Ollama server side)")
foreach ($x in (Fmt $catOtherLoop)) { $s.Add($x) }
$s.Add("")
$verdict = if ($catDocaidExt.Count -eq 0 -and $catWebviewExt.Count -eq 0 -and $catOllamaExt.Count -eq 0) { "PASS (no connection left this machine)" } else { "CHECK (rows above in [1]/[3]/[4] need an explanation)" }
if ($start.online -or $end.online) { $verdict += " -- but internet was reachable, so this run does not count as offline" }
$s.Add("verdict: $verdict")

[System.IO.File]::WriteAllLines((Join-Path $dir "summary.txt"), $s, (New-Object System.Text.UTF8Encoding($false)))
Write-Output ""
Write-Output ($s -join "`n")
Write-Output ""
Write-Output "Done. Turn Wi-Fi back on and hand this folder to Claude:"
Write-Output "  $dir"
