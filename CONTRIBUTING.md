# 贡献指南

欢迎为 DeepSeek Harness Up 贡献！

## 开发环境

- Windows 10/11 x64 + [Node.js LTS](https://nodejs.org) + [Rust](https://rustup.rs)（MSVC）

## 常用命令

```powershell
.\scripts\dev.ps1           # 开发调试（热窗口）
.\scripts\make-release.ps1  # 发布构建 → dist/
.\scripts\self-check.ps1    # 自检（语法/文件完整）
```

## 提交规范（Conventional Commits）

```
feat: 新功能      fix: 修复       docs: 文档      chore: 杂项      style: 样式
```

## 代码约定

- **非侵入原则**（重要）：只调用 dsh 的公共 CLI / npm 生态 / 只读观测；绝不接触 `~/.dsh` 内部结构或 dsh 内部 API（这是工具不会因 dsh 大更新而失效的根基）
- 进程调用必须走 `src-tauri/src/winutil.rs`（统一 `CREATE_NO_WINDOW`，禁止裸 `cmd`）
- 前端脚本改动后运行 `node --check`；修改 IPC 契约需前后端同步

## Issue / PR

- Bug 报告：描述复现步骤 + 附带 `%LOCALAPPDATA%\dsh-up\log.txt` 日志
- PR：描述动机与影响，关联 issue 编号