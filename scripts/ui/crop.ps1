# Crop a screenshot and scale it up, so a small detail can actually be seen.
#   powershell -File ui/crop.ps1 -In shot.png -Out zoom.png -X 900 -Y 360 -W 500 -H 220 -Zoom 2

param(
  [Parameter(Mandatory = $true)][string]$In,
  [Parameter(Mandatory = $true)][string]$Out,
  [int]$X = 0, [int]$Y = 0, [int]$W = 400, [int]$H = 300, [int]$Zoom = 2
)

Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'

$src = [System.Drawing.Image]::FromFile((Resolve-Path $In))
try {
  $W = [Math]::Min($W, $src.Width - $X)
  $H = [Math]::Min($H, $src.Height - $Y)
  $rect = New-Object System.Drawing.Rectangle($X, $Y, $W, $H)

  $big = New-Object System.Drawing.Bitmap(($W * $Zoom), ($H * $Zoom))
  $g = [System.Drawing.Graphics]::FromImage($big)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
  $g.DrawImage($src, (New-Object System.Drawing.Rectangle(0, 0, ($W * $Zoom), ($H * $Zoom))), $rect, [System.Drawing.GraphicsUnit]::Pixel)
  $g.Dispose()
  $big.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
  $big.Dispose()
  Write-Host ("saved " + $Out + " (" + ($W * $Zoom) + "x" + ($H * $Zoom) + ")")
} finally {
  $src.Dispose()
}
