# 构建脚本：出全名产物
$ErrorActionPreference = "Stop"
Set-Location "C:\Users\star\Desktop\dsh-up\src-tauri"
cargo build --release
$src = "target\release\harness-up.exe"
$dst = "target\release\DeepSeek Harness Up.exe"
Copy-Item $src $dst -Force
Start-Sleep -Milliseconds 300
Remove-Item $src -ErrorAction SilentlyContinue
Write-Output ("产物: " + (Get-Item $dst).FullName)
