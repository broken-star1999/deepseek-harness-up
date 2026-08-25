# DeepSeek Harness Up 一键发布构建
# 用法: .\scripts\make-release.ps1
$ErrorActionPreference = "Stop"
Set-Location "$PSScriptRoot\.."

Write-Output "==> 清理运行进程"
Get-Process "deepseek-harness-up" -ErrorAction SilentlyContinue | Stop-Process -Force

Write-Output "==> 构建 release"
# 路径脱敏：编译期重映射本机路径，避免 exe 泄漏用户名/目录（零硬编码，任何机器通用）
# rustc 只做前缀匹配替换，匹配不到时自动跳过（等价未启用），不会导致编译失败
$projectRoot = (Resolve-Path "$PSScriptRoot\..").Path
$env:RUSTFLAGS = "--remap-path-prefix=$projectRoot=/repo --remap-path-prefix=$env:USERPROFILE\.cargo=/cargo --remap-path-prefix=$env:USERPROFILE/.cargo=/cargo"
$env:Path += ";$env:USERPROFILE\.cargo\bin"
cargo build --release --manifest-path src-tauri\Cargo.toml
if ($LASTEXITCODE -ne 0) { throw "构建失败" }

Write-Output "==> 复制产物到 dist/"
New-Item -ItemType Directory -Path "dist" -Force | Out-Null
Copy-Item "src-tauri\target\release\deepseek-harness-up.exe" "dist\deepseek-harness-up.exe" -Force
$f = Get-Item "dist\deepseek-harness-up.exe"
Write-Output ("✅ 发布产物: dist\deepseek-harness-up.exe (" + [math]::Round($f.Length/1MB,2) + " MB)")

Write-Output "==> 提交建议"
Write-Output "  git add -A && git commit -m 'build: release'"