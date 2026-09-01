# 项目自检：JS 语法 + 前后端命令契约 + DOM 引用
$ErrorActionPreference = "Stop"
Set-Location "$PSScriptRoot\.."
Write-Output "==> JS 语法检查"
foreach ($js in (Get-ChildItem ui -Filter *.js)) {
  node --check $js.FullName
  if ($LASTEXITCODE -ne 0) { throw "JS 语法失败: $($js.Name)" }
}
Write-Output "所有 JS 语法通过 ✅"

Write-Output "==> 关键文件存在性"
foreach ($k in @("README.md","LICENSE","src-tauri\Cargo.toml","src-tauri\tauri.conf.json","ui\index.html")) {
  if (Test-Path $k) { Write-Output "  $k ✅" } else { Write-Output "  $k ❌ 缺失"; exit 1 }
}
Write-Output "==> IPC 契约检查"
$rs = Get-Content src-tauri/src/lib.rs -Raw
$handler = [regex]::Match($rs, 'generate_handler!\s*\[(?<body>[\s\S]*?)\]\s*\)')
if (-not $handler.Success) { throw "未找到 generate_handler 列表" }
$registered = [regex]::Matches($handler.Groups['body'].Value, '(?m)^\s*([a-z_][a-z0-9_]*)\s*(?:,|$)') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
$ui = (Get-ChildItem ui -Filter *.js | ForEach-Object { Get-Content $_.FullName -Raw }) -join "`n"
$invokes = [regex]::Matches($ui, 'invoke\(\s*[\x27\x22]([a-z_][a-z0-9_]*)[\x27\x22]') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
$missing = @()
foreach ($name in $invokes) { if ($registered -notcontains $name) { $missing += $name } }
if ($missing.Count -gt 0) { throw "IPC 契约断裂，后端缺失: $($missing -join ', ')" }
Write-Output ("IPC 检查通过: " + ($invokes -join ', '))

Write-Output "==> DOM 引用检查"
$html = (Get-ChildItem ui -Filter *.html | ForEach-Object { Get-Content $_.FullName -Raw }) -join "`n"
$ids = [regex]::Matches($html, 'id=[\x27\x22]([^\x27\x22]+)[\x27\x22]') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
$refs = [regex]::Matches($ui, 'getElementById\([\x27\x22]([^\x27\x22]+)[\x27\x22]\)') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
$missingDom = @()
foreach ($id in $refs) { if ($ids -notcontains $id) { $missingDom += $id } }
if ($missingDom.Count -gt 0) { throw "DOM 引用缺失: $($missingDom -join ', ')" }
Write-Output "DOM 检查通过"

$inlineHandlers = [regex]::Matches($html, '(?i)<[^>]+\bon[a-z]+\s*=')
if ($inlineHandlers.Count -gt 0) { throw "HTML 存在 inline 事件属性: $($inlineHandlers -join ', ')" }
Write-Output "HTML 事件属性检查通过"

Write-Output "==> 单 EXE 发布契约检查"
$tauriConfig = Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json
if (-not $tauriConfig.bundle -or $tauriConfig.bundle.active -ne $false) {
  throw "发布目标必须是单 EXE：tauri bundle.active 应为 false"
}
Write-Output "发布目标为单 EXE ✅"

Write-Output "自检完成 ✅"
