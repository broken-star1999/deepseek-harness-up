# DeepSeek Harness Up 🐋

![CI](https://github.com/broken-star1999/deepseek-harness-up/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/github/license/broken-star1999/deepseek-harness-up)
![Release](https://img.shields.io/github/v/release/broken-star1999/deepseek-harness-up)

> DSH 启动器 · 内嵌浏览器 · 更新/卸载管理器 —— 非插件、非侵入的外部工具

DeepSeek Harness Up 是一个 Windows 桌面工具：双击即用，把 DeepSeek Harness (`@deepseek-ai/dsh`) 的启动、运行、更新、卸载装进一个便携 exe。

## ✨ 功能

| 功能 | 说明 |
|------|------|
| 启动器 | 一键检测环境 → 安装 Node/dsh → 启动核心（`dsh web --no-open`，零黑窗） |
| 内嵌显示器 | 全窗口 WebView 显示官方 DSH 界面（不弹系统浏览器） |
| 核心管理 | 绿色启动 / 红色停止（只关核心不退应用） |
| 设置 | 四页签：✕ 行为 / 双更新检查 / 镜像配置 / 壁纸 |
| 壁纸 | 内置 + 自选（系统级文件对话框，data URI 零权限） |
| 系统托盘 | 隐藏时任务栏消失，托盘图标一键唤回 |
| 单实例 | 二次启动自动唤醒已有实例（TCP 端口锁，零插件） |
| 卸载 | 一键卸载 + 可选清除用户配置（默认保留） |
| 日志 | %LOCALAPPDATA%\dsh-up\log.txt 全量运行日志（自动轮转） |

## 🛡 非侵入原则

工具只依赖 dsh 的**公共稳定边界**：CLI 命令、npm 生态、只读观测。绝不接触 `~/.dsh` 内部结构 / dsh 内部 API。

> ⚠️ **上游动态**：DeepSeek Harness 目前处于 developer preview，官方明确宣布**会有破坏性变更**（见上游 README）。本工具只踩公共边界，目的就是把这种变更的影响降到最低；若上游 CLI 有大变动，本工具会跟随适配。

## 📦 使用

```
1. 下载 Releases 里的 DeepSeek Harness Up.exe
2. 双击即可（绿色便携，无安装步骤）
3. 新机器自动引导安装 Node / dsh
```

系统要求：Windows 10/11 x64（WebView2 运行时，Win11 自带）

## 🖼 界面预览

| 启动器 | DSH 界面 |
|---|---|
| ![启动器](screenshots/launcher.jpg) | ![DSH 界面](screenshots/dsh.png) |

> 截图基于 v0.1.0 实际运行画面（Win11 · 自定义壁纸）

## 🔨 从源码构建

```powershell
# 前置：Node.js LTS + Rust (rustup, MSVC) + VS Build Tools
npm install
.\\scripts\\dev.ps1           # 开发调试
.\\scripts\\make-release.ps1  # 一键发布构建（产出 dist\\deepseek-harness-up.exe）
```

## 📄 许可

MIT License（见 LICENSE）

---

*与 DeepSeek Harness 无关的社区工具。DeepSeek 是 DeepSeek AI 的商标。*