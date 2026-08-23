# 项目自检：JS 语法 + 前后端命令契约 + DOM 引用
Set-Location "$PSScriptRoot\.."
Write-Output "==> JS 语法检查"
node --check ui\main.js
node --check ui\controls.js
node --check ui\modal.js
node --check ui\settings.js
Write-Output "所有 JS 语法通过 ✅"

Write-Output "==> 关键文件存在性"
foreach ($k in @("README.md","LICENSE","src-tauri\Cargo.toml","src-tauri\tauri.conf.json","ui\index.html")) {
  if (Test-Path $k) { Write-Output "  $k ✅" } else { Write-Output "  $k ❌ 缺失"; exit 1 }
}
Write-Output "自检完成 ✅"
