# Launch the GUI several times and capture the screen so rendering (CJK text, SVG icons,
# menus, tooltips, dialogs) can be checked from CI artifacts.
param([string]$Exe = 'target\release\pixel-studio.exe', [string]$OutDir = 'win-shots', [string]$Image = 'crates\studio-core\tests\fixtures\redact_sample.png')
$ErrorActionPreference = 'Stop'
# This harness drives the disposable GitHub runner, never the developer's desktop.
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Run this screenshot harness on a GitHub Actions Windows runner.' }
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class W32 {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
  [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT rect);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT rect);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int command);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
}
"@
$oldDpiContext = [W32]::SetThreadDpiAwarenessContext([IntPtr]::new(-4)) # per-monitor aware v2
if ($oldDpiContext -eq [IntPtr]::Zero) { throw 'Cannot establish physical-pixel capture coordinates.' }
try {
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$img = (Resolve-Path $Image).Path

function Geometry([IntPtr]$handle) {
  $window = New-Object W32+RECT
  $client = New-Object W32+RECT
  $origin = New-Object W32+POINT
  if (-not [W32]::GetWindowRect($handle, [ref]$window)) { throw 'GetWindowRect failed.' }
  if (-not [W32]::GetClientRect($handle, [ref]$client)) { throw 'GetClientRect failed.' }
  if (-not [W32]::ClientToScreen($handle, [ref]$origin)) { throw 'ClientToScreen failed.' }
  $dpi = [W32]::GetDpiForWindow($handle)
  if ($dpi -eq 0) { throw 'GetDpiForWindow failed.' }
  $screen = [System.Windows.Forms.Screen]::FromHandle($handle)
  $bounds = [System.Drawing.Rectangle]::new($origin.X, $origin.Y, $client.Right - $client.Left, $client.Bottom - $client.Top)
  return [pscustomobject]@{
    Window = @{ X=$window.Left; Y=$window.Top; Width=$window.Right-$window.Left; Height=$window.Bottom-$window.Top }
    Client = $bounds
    Screen = $screen.Bounds
    WorkArea = $screen.WorkingArea
    Dpi = $dpi
    Maximized = [W32]::IsZoomed($handle)
    LogicalClient = @{ Width=$bounds.Width * 96.0 / $dpi; Height=$bounds.Height * 96.0 / $dpi }
    FullyVisible = $screen.WorkingArea.Contains($bounds)
  }
}

function Shot([string]$name, [hashtable]$envs, [scriptblock]$interact) {
  foreach ($k in @('PIXEL_STUDIO_TAB', 'PIXEL_STUDIO_MENU', 'PIXEL_STUDIO_DIALOG', 'PIXEL_STUDIO_C2PA_DIALOG')) { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
  $env:PIXEL_STUDIO_LANG = 'zh-TW'
  foreach ($k in $envs.Keys) { Set-Item "env:$k" $envs[$k] }
  # Do not let the previous scene's hover switch a startup menu during resize.
  $workArea = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
  if (-not [W32]::SetCursorPos($workArea.Right - 2, $workArea.Bottom - 2)) { throw 'Cannot park the capture cursor.' }
  $p = Start-Process $Exe -PassThru -ArgumentList "`"$img`""
  try {
  Start-Sleep -Seconds 9
  if ($p.HasExited) { throw "GUI exited early with code $($p.ExitCode)" }
  $p.Refresh()
  $h = $p.MainWindowHandle
  if ($h -eq [IntPtr]::Zero) { throw 'GUI has no main window.' }
  $before = Geometry $h
  # The app prefers 1400x860 logical pixels; runners may only expose 1024x768.
  # Exercise the existing resizable layout through normal window maximization.
  [W32]::ShowWindow($h, 3) | Out-Null
  [W32]::SetForegroundWindow($h) | Out-Null
  Start-Sleep -Milliseconds 1200
  if ($interact) { & $interact $p }
  $after = Geometry $h
  @{ Name=$name; Before=$before; After=$after; SlintScaleFactor=$env:SLINT_SCALE_FACTOR; SlintBackend=$env:SLINT_BACKEND } |
    ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 (Join-Path $OutDir "$name.json")
  if (-not $after.FullyVisible) { throw "Client area is outside the visible work area: $name" }
  if ([W32]::GetForegroundWindow() -ne $h) { throw "GUI is not foreground: $name" }
  $b = $after.Client
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = $null
  try {
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $bmp.Save((Join-Path $OutDir "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
  } finally { if ($g) { $g.Dispose() }; $bmp.Dispose() }
  } finally {
    if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
    $p.Dispose()
  }
  Start-Sleep -Milliseconds 800
  Write-Host "saved $name.png"
}

# Hover the top-bar "Open" button (client x≈52, y≈46) to show its tooltip.
$hover = {
  param($p)
  $h = $p.MainWindowHandle
  [W32]::SetForegroundWindow($h) | Out-Null
  $pt = New-Object W32+POINT
  $scale = [W32]::GetDpiForWindow($h) / 96.0
  $pt.X = [int](52 * $scale); $pt.Y = [int](46 * $scale)
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
} finally { [W32]::SetThreadDpiAwarenessContext($oldDpiContext) | Out-Null }
