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

Write-Output "==> 发布闸门检查"
# 1) 版本号内嵌检查
$fi = [Diagnostics.FileVersionInfo]::GetVersionInfo("dist\deepseek-harness-up.exe")
$want = (Select-String -Path "src-tauri\Cargo.toml" -Pattern '^version').Line -replace '.*"([^"]+)".*','$1'
Write-Output ("  版本: " + $fi.FileVersion + " (期望 " + $want + ")")
if ($fi.FileVersion -ne $want.Trim()) { throw "版本号不一致: $($fi.FileVersion) != $want" }
# 2) 路径泄漏检查（本机用户名/CI runner 路径）
$bytes = [IO.File]::ReadAllBytes("dist\deepseek-harness-up.exe")
$text = [Text.Encoding]::ASCII.GetString($bytes)
$users = ([regex]::Matches($text, 'C:\\Users\\')).Count
$reg = ([regex]::Matches($text, '\.cargo\\registry')).Count
$ci = ([regex]::Matches($text, 'D:\\a\\')).Count
Write-Output ("  C:\Users 出现 $users 次 | .cargo\registry $reg 处 | D:\a\ $ci 处")
# 允许 tauri 框架编译期注入的少量 CARGO_MANIFEST_DIR 痕迹（<=3 次），大泄漏视为失败
if ($reg -gt 0 -or $ci -gt 0 -or $users -gt 3) { throw "发布产物包含本机路径泄漏 (users=$users registry=$reg ci=$ci)" }
Write-Output "  ✅ 版本号一致 / 路径泄漏在允许范围"

Write-Output "==> 提交建议"
Write-Output "  git add -A && git commit -m 'build: release'"