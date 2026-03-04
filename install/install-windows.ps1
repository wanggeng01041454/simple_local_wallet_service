# install-windows.ps1
# Installs local-wallet as a Task Scheduler task (no admin required)
param()

$BinaryPath = ".\local-wallet.exe"
$InstallDir  = "$env:LOCALAPPDATA\local-wallet"
$InstallPath = "$InstallDir\local-wallet.exe"
$TaskName    = "LocalWalletService"

if (-not (Test-Path $BinaryPath)) {
    Write-Error "local-wallet.exe not found. Build first with: cargo build --release --target x86_64-pc-windows-msvc"
    exit 1
}

Write-Host "Installing local-wallet to $InstallPath..."
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item $BinaryPath $InstallPath -Force

Write-Host "Registering Task Scheduler task '$TaskName'..."
$Action   = New-ScheduledTaskAction -Execute $InstallPath
$Trigger  = New-ScheduledTaskTrigger -AtLogOn
$Settings = New-ScheduledTaskSettingsSet `
    -RestartCount 3 `
    -RestartInterval (New-TimeSpan -Minutes 1) `
    -ExecutionTimeLimit ([TimeSpan]::Zero)

Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Settings $Settings -Force | Out-Null

Write-Host "Starting local-wallet..."
Start-ScheduledTask -TaskName $TaskName

Write-Host ""
Write-Host "Done. local-wallet is running."
Write-Host "  Admin UI: http://localhost:9292"
Write-Host "  REST API: http://localhost:9293 (localhost only)"
Write-Host "  Data dir: $env:APPDATA\local-wallet"
