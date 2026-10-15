# Launch the GUI and capture the screen so rendering can be checked from CI artifacts.
param([string]$Exe = 'target\release\pixel-studio.exe', [string]$Out = 'win-screenshot.png', [string]$Image = 'crates\studio-core\tests\fixtures\redact_sample.png')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$env:PIXEL_STUDIO_LANG = 'zh-TW'
$env:PIXEL_STUDIO_TAB = '3'
$p = Start-Process $Exe -PassThru -ArgumentList "`"$((Resolve-Path $Image).Path)`""
Start-Sleep -Seconds 12
if ($p.HasExited) { throw "GUI exited early with code $($p.ExitCode)" }
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
Stop-Process -Id $p.Id -Force
Write-Host "saved $Out ($($b.Width)x$($b.Height))"
