<p align="center">
  <img src="assets/icon.png" width="140" alt="DeepSeek Harness Up" />
</p>

# DeepSeek Harness Up 🐋

![CI](https://github.com/broken-star1999/deepseek-harness-up/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/github/license/broken-star1999/deepseek-harness-up)
![Release](https://img.shields.io/github/v/release/broken-star1999/deepseek-harness-up)

> DSH 启动器 · 内嵌浏览器 · 更新/卸载管理器 —— 非插件、非侵入的外部工具

## 🖼 界面预览

<p align="center">
  <img src="screenshots/launcher.jpg" alt="启动器" width="720" />
  <br />
  <sub>启动器 · 一键启动 / 停止 / 更新 DeepSeek Harness</sub>
</p>

<p align="center">
  <img src="screenshots/dsh.png" alt="DSH 界面" width="720" />
  <br />
  <sub>内嵌 WebView 显示的官方 DSH 界面</sub>
</p>

> 截图基于 v0.1.0 实际运行画面（Win11 · 自定义壁纸）

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
| 卸载 | 标准卸载或彻底卸载 dsh，可选择删除用户配置 |
| 日志 | %LOCALAPPDATA%\dsh-up\log.txt 全量运行日志（自动轮转） |

## 🛡 非侵入原则

工具只依赖 dsh 的**公共稳定边界**：CLI 命令、npm 生态、只读观测。正常运行不接触 `~/.dsh` 内部结构 / dsh 内部 API；用户明确确认彻底卸载后，才会删除 dsh 用户数据。

> ⚠️ **上游动态**：DeepSeek Harness 目前处于 developer preview，官方明确宣布**会有破坏性变更**（见上游 README）。本工具只踩公共边界，目的就是把这种变更的影响降到最低；若上游 CLI 有大变动，本工具会跟随适配。

## 🎯 项目目标

把 DeepSeek Harness 变成 **Windows 桌面级体验**：启动、更新、卸载、监控这些"围绕 DSH 的杂事"交给工具，用户只负责使用。正常运行只动 dsh 的公共边界（CLI / npm / 只读观测）；用户主动选择彻底卸载时，可一并清除 dsh 用户数据。

**路线图（每项都保持非侵入）**：

- **健康守护**：核心异常退出自动重启 + 崩溃提醒（只读观测 + 官方 CLI）
- **多实例工作区**：`--profile` 一键切换多套 dsh 环境（官方参数，无需改任何内部数据）
- **生态快捷面板**：会话 / 日志 / 配置目录一键直达（仅系统导航，不解析、不修改内部结构）
- **体验打磨**：快捷键、开机自启选项、主题跟随
- **镜像体验优化**：多镜像测速、一键切换（npm 生态内，官方 registry）

> dsh 的本体由官方迭代，我们只负责让它在你 Windows 上"住得舒服"。

## 📦 使用

```
1. 下载 Releases 里的 DeepSeek Harness Up.exe
2. 双击即可（绿色便携，无安装步骤）
3. 新机器自动引导：一键安装 dsh；Node.js 打开官方下载页（LTS）
```

系统要求：Windows 10/11 x64（WebView2 运行时，Win11 自带）

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
