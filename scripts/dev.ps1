# 开发模式启动
$ErrorActionPreference = "Stop"
Set-Location "$PSScriptRoot\.."
$env:Path += ";$env:USERPROFILE\.cargo\bin"
npx tauri dev
