# 🚀 dsh-up

> **DSH 快速启动器 · 自动更新 · 彻底清理**
>
> 让 DeepSeek Harness 启动更快、永远最新、卸载干净。

dsh-up 是一个独立的**非插件**工具：绕过 `npx` 的慢启动，用 npm 全局安装的 `dsh` 一键拉起；启动前静默检查更新，保持跟随官方版本；需要离开时，把 DSH 在这台机器上的所有痕迹一次性清干净。

```
npx @deepseek-ai/dsh web   # 慢：联网检查 + 解析 246MB 缓存
dsh-up                     # 快：检查更新(可选) → 直接拉起全局 dsh
```

---

## ✨ 功能

| 命令 | 作用 |
|------|------|
| `dsh-up` | 检查更新 → 启动 Web UI（默认行为） |
| `dsh-up web` | 快速启动（跳过更新检查） |
| `dsh-up update` | 手动检查并更新全局 dsh 到最新版 |
| `dsh-up status` | 查看本地版本 vs npm 最新版 |
| `dsh-up clean` | **彻底清理**：卸载全局包 + 删配置 + 清 npx 缓存 |
| `dsh-up --help` | 帮助 |

## 🎯 为什么做

- **npx 慢**：每次 `npx @deepseek-ai/dsh` 都要联网解析 npm registry、校验 246MB 的 npx 缓存，冷启动明显慢于直接调用全局命令。
- **全局安装快但缺更新**：`npm i -g @deepseek-ai/dsh` 后 `dsh web` 秒起，但不会自动跟进官方每日更新的开发预览版。
- **卸载不干净**：npx 缓存、全局包、`profiles/`、`settings.yaml`、PowerShell shim 散落多处，手动清理容易遗漏。

**dsh-up 把这三件事合到一起。**

