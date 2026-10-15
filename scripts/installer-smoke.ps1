# Silent per-user install / verify / uninstall of the Inno Setup installer.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$setup = Join-Path $root 'dist\pixel-studio-setup-windows-x64.exe'
$dir = Join-Path $env:LOCALAPPDATA 'Programs\Pixel Studio'
function Check($cond, $msg) { if (-not $cond) { throw "FAILED: $msg" } ; Write-Host "ok: $msg" }

$p = Start-Process $setup -Wait -PassThru -ArgumentList @(
    '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER', '/LANG=english',
    '/TASKS=desktopicon,contextmenu,assoc', '/LOG=install.log')
Check ($p.ExitCode -eq 0) "installer exit code 0 (got $($p.ExitCode))"
foreach ($f in 'pixel-studio.exe', 'studio-cli.exe', 'models\text-detection.onnx', 'models\text-recognition.onnx', 'README.md', 'LICENSE.txt', 'unins000.exe') {
    Check (Test-Path (Join-Path $dir $f)) "installed $f"
}
Check (Test-Path (Join-Path ([Environment]::GetFolderPath('Programs')) 'Pixel Studio.lnk')) "Start menu shortcut"
Check (Test-Path (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Pixel Studio.lnk')) "Desktop shortcut"

$edit = 'HKCU:\Software\Classes\SystemFileAssociations\.png\shell\PixelStudio.edit'
Check (Test-Path $edit) "PNG right-click 'Edit with Pixel Studio'"
Check ((Get-ItemProperty $edit).MUIVerb -eq 'Edit with Pixel Studio') "English menu label"
$cmd = (Get-ItemProperty "$edit\command").'(default)'
Check ($cmd -like "*pixel-studio.exe*--edit*%1*") "edit command: $cmd"
Check (Test-Path 'HKCU:\Software\Classes\SystemFileAssociations\.webp\shell\PixelStudio.open') "WebP right-click 'Open with Pixel Studio'"
$owp = Get-ItemProperty 'HKCU:\Software\Classes\.png\OpenWithProgids'
Check ($null -ne $owp.'PixelStudio.Image') "PNG Open With entry"
Check ($null -ne (Get-ItemProperty 'HKCU:\Software\Classes\.webp\OpenWithProgids').'PixelStudio.Image') "WebP Open With entry"
Check (Test-Path 'HKCU:\Software\Classes\PixelStudio.Image\shell\open\command') "ProgID registered"
$def = (Get-ItemProperty 'HKCU:\Software\Classes\.png' -ErrorAction SilentlyContinue).'(default)'
Check ($def -ne 'PixelStudio.Image') "default PNG handler not hijacked"

$ver = & (Join-Path $dir 'studio-cli.exe') --version
Check ($LASTEXITCODE -eq 0) "studio-cli --version: $ver"
$tmp = Join-Path $env:RUNNER_TEMP 'pssmoke'
New-Item -ItemType Directory -Force $tmp | Out-Null
& (Join-Path $dir 'studio-cli.exe') redact (Join-Path $root 'crates\studio-core\tests\fixtures\redact_sample.png') (Join-Path $tmp 'out.webp')
Check ($LASTEXITCODE -eq 0) "installed CLI redacts offline with bundled models"
Check (Test-Path (Join-Path $tmp 'out.webp')) "redacted WebP written"

$p = Start-Process (Join-Path $dir 'unins000.exe') -Wait -PassThru -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART'
Check ($p.ExitCode -eq 0) "uninstaller exit code 0"
Start-Sleep -Seconds 3
Check (-not (Test-Path (Join-Path $dir 'pixel-studio.exe'))) "pixel-studio.exe removed"
Check (-not (Test-Path $edit)) "context menu removed"
Check (-not (Test-Path 'HKCU:\Software\Classes\PixelStudio.Image')) "ProgID removed"
$owp = Get-ItemProperty 'HKCU:\Software\Classes\.png\OpenWithProgids' -ErrorAction SilentlyContinue
Check (($null -eq $owp) -or ($null -eq $owp.'PixelStudio.Image')) "Open With entry removed"
Check (-not (Test-Path (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Pixel Studio.lnk'))) "Desktop shortcut removed"
Write-Host "Installer smoke test passed"
