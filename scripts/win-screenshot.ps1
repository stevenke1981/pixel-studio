# Launch the GUI several times and capture the screen so rendering (CJK text, SVG icons,
# menus, tooltips, dialogs) can be checked from CI artifacts.
param([string]$Exe = 'target\release\pixel-studio.exe', [string]$OutDir = 'win-shots', [string]$Image = 'crates\studio-core\tests\fixtures\redact_sample.png')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class W32 {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
}
"@
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$img = (Resolve-Path $Image).Path

function Shot([string]$name, [hashtable]$envs, [scriptblock]$interact) {
  foreach ($k in @('PIXEL_STUDIO_TAB', 'PIXEL_STUDIO_MENU', 'PIXEL_STUDIO_DIALOG', 'PIXEL_STUDIO_C2PA_DIALOG')) { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
  $env:PIXEL_STUDIO_LANG = 'zh-TW'
  foreach ($k in $envs.Keys) { Set-Item "env:$k" $envs[$k] }
  $p = Start-Process $Exe -PassThru -ArgumentList "`"$img`""
  Start-Sleep -Seconds 9
  if ($p.HasExited) { throw "GUI exited early with code $($p.ExitCode)" }
  $p.Refresh()
  if ($interact) { & $interact $p }
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $bmp.Save((Join-Path $OutDir "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
  Stop-Process -Id $p.Id -Force
  Start-Sleep -Milliseconds 800
  Write-Host "saved $name.png"
}

# Hover the top-bar "Open" button (client x≈52, y≈46) to show its tooltip.
$hover = {
  param($p)
  $h = $p.MainWindowHandle
  [W32]::SetForegroundWindow($h) | Out-Null
  $pt = New-Object W32+POINT
  $pt.X = 52; $pt.Y = 46
  [W32]::ClientToScreen($h, [ref]$pt) | Out-Null
  [W32]::SetCursorPos($pt.X - 30, $pt.Y + 200) | Out-Null
  Start-Sleep -Milliseconds 300
  [W32]::SetCursorPos($pt.X, $pt.Y) | Out-Null
  Start-Sleep -Milliseconds 1500
}

Shot 'win-main' @{ PIXEL_STUDIO_TAB = '3' } $null
Shot 'win-tooltip' @{ PIXEL_STUDIO_TAB = '2' } $hover
Shot 'win-menu-file' @{ PIXEL_STUDIO_MENU = '0' } $null
Shot 'win-menu-image' @{ PIXEL_STUDIO_MENU = '3' } $null
Shot 'win-dialog-about' @{ PIXEL_STUDIO_DIALOG = 'about' } $null
Shot 'win-dialog-keys' @{ PIXEL_STUDIO_DIALOG = 'keys' } $null
Shot 'win-dialog-resize' @{ PIXEL_STUDIO_DIALOG = 'resize' } $null
Shot 'win-dialog-props' @{ PIXEL_STUDIO_DIALOG = 'props'; PIXEL_STUDIO_TAB = '5' } $null
Shot 'win-dialog-quicksave' @{ PIXEL_STUDIO_DIALOG = 'quicksave' } $null
Shot 'win-dialog-c2pa' @{ PIXEL_STUDIO_C2PA_DIALOG = '1' } $null
Copy-Item (Join-Path $OutDir 'win-main.png') 'win-screenshot.png'
