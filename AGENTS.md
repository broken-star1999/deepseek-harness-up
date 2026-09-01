# AGENTS.md — AI Agent 协作指南

> 给在此仓库工作的 AI 编码助手（Copilot / Claude / 其他 agent）的说明。

## 项目是什么

DeepSeek Harness Up：Windows 桌面工具（Tauri 2 + Rust 后端 + 无框架 Web 前端），管理 DeepSeek Harness (`@deepseek-ai/dsh`) 的启动/运行/更新/卸载。**非插件、非侵入**。

## 关键约定（必须遵守）

1. **外部管理原则**：正常运行只依赖 dsh 公共 CLI / npm 生态 / 只读观测，不接触 dsh 内部 API 或内部运行逻辑。用户明确确认“彻底卸载”后，可以删除 dsh 安装内容及其用户数据（例如 `~/.dsh`）；除该卸载动作外，不解析、不修改 dsh 用户数据。
2. **窗口零闪烁**：所有进程调用统一走 `src-tauri/src/winutil.rs` 的 `cmd_hidden`/`exe_hidden`/`batch_hidden`（CREATE_NO_WINDOW）。禁止裸 `Command::new("cmd")`。
3. **镜像一致性**：npm 命令必须带 `--registry`（读 settings mirror）。
4. **路径密码**：产物名 `deepseek-harness-up`；本地目录 `C:\Users\star\Desktop\deepseek-harness-up`。

## 命令

```powershell
.\scripts\dev.ps1           # dev 窗口
.\scripts\make-release.ps1  # 发布构建
.\scripts\self-check.ps1    # 自检
cargo build --manifest-path src-tauri\Cargo.toml   # 编译（Rust）
node --check ui/main.js                        # JS 语法
```

## 代码结构

- `ui/`：前端（index 启动器 / controls 顶栏 / modal 弹窗 / settings 设置；均无框架）
- `src-tauri/src/`：Rust（winutil 进程隐藏 / dsh_process 生命周期 / updater 版本 / uninstaller / lib IPC）
- `src-tauri/capabilities/`：Tauri ACL 权限（改动需双端同步）
- 日志：`%LOCALAPPDATA%\dsh-up\log.txt`（排障第一现场）
