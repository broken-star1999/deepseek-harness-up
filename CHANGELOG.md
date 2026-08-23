# Changelog

本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/) 与 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/)。

## [0.1.0] - 2026-08-24

### ✨ 功能
- 启动器：环境检测 → 安装 Node/dsh → 启动核心（`dsh web --no-open`，零黑窗）
- 内嵌显示器：全窗口 WebView 显示官方 DSH 界面（不弹系统浏览器）
- 核心管理：绿色启动 / 红色停止（只关核心不退应用）
- 设置四页签：✕ 行为 / 双更新检查 / 镜像配置 / 壁纸
- 系统托盘（隐藏时任务栏消失，一键唤回）· 单实例唤醒
- 卸载（可选清除用户配置，默认保留）· 全量运行日志（自动轮转）

### 🛡 设计
- 非侵入原则：只依赖 dsh 公共 CLI/npm 生态/只读观测

### 🧪 质量
- 交互 22/22 场景矩阵验证 · 编译零警告（消除 4 处 unused import）