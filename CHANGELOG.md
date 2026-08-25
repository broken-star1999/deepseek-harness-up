# Changelog

本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/) 与 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/)。

## [0.2.0] - 2026-08-24

### ✨ 新增
- 软件自身更新检查：内置 GitHub Releases 源，检测到新版本一键跳转下载页
- 打开 DSH 前等待端口就绪（修复启动过快点击黑屏）
- 新机器引导优化：无 Node 时安装按钮置灰提示 + 「↻ 重检」入口

### 🐛 修复
- 默认退出主程序时同步停止 dsh 核心
- 移除旧设置弹窗死代码（打开设置时的 null 报错）
- renderActions 变量声明顺序（TDZ）导致卡在检查环境
- 启动核心后状态刷新静默失败

### 🔧 构建
- release 构建改用 thin LTO（增量重编提速）
- 构建路径脱敏（remap-path-prefix，产物不含本机用户名）
- GitHub Actions 供应链 pin（commit SHA）+ Release 自动校验和

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