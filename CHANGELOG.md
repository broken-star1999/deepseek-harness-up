# Changelog

本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/) 与 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/)。

## [0.2.7] - 2026-08-30

### 🐛 修复
- 设置保存真正原子化：唯一临时文件 + `.bak` 备份 + Windows MoveFileExW 替换（损坏自动恢复）
- 操作锁覆盖 start/stop/update/install/uninstall/stop_port_owner/quit，全部先锁后检查
- Node/npm 定位链完全绝对路径：npm.cmd 经隐藏 cmd /C 执行（修复批量脚本执行失败）
- dsh 进程身份精确匹配已定位的 `@deepseek-ai\dsh\lib\bin.js`（删除宽松 `@deepseek-ai` 兜底）
- 卸载路径加强：拒绝 drive-relative `C:foo`、多冒号路径、UNC；用户目录/npx 目录 fail closed
- 前端设置页“立即更新”加 busy 防重复；离线/失败隐藏按钮
- npm 流式日志先建日志后启动子进程，wait 失败清理孤儿进程

### 🔧 加固
- 图片魔数统一检测（PNG/JPEG/WebP），picker 和 set_bg_bytes 均校验
- 发布白名单改为 end-anchored 精确匹配并限制命中数量
- 新增 dsh 进程身份精确匹配单测

## [0.2.6] - 2026-08-30

### 🐛 修复
- 系统关闭真正拦截：CloseRequested 使用 api.prevent_close()，Minimize=隐藏托盘 / Exit=停核心退出 / 未设置=弹确认窗（与 ✕ 一致）
- Node/npm 版本检测改用绝对路径执行（patch 后重检立即生效）
- 打开 DSH 失败不再白屏：失败恢复启动器界面
- 3080 身份校验收紧（去掉裸 bin.js；占用者未知时 fail closed 拒绝）
- 后端 dsh 更新操作加互斥锁 + 核心运行即拒绝（绕过前端也拦）

### ⚙️ 可靠性
- 壁纸校验落到实际入口 pick_and_set_bg（10MB + PNG/JPEG/WebP 魔数）+ get_bg_data 二次保护
- 设置写入统一 save_settings + 线程锁（并发防覆盖）
- 卸载路径规范化防御（相对/UNC/盘根/用户目录祖先，纯字符串规范化）
- 动态内容全部 textContent（消除 innerHTML 注入面）

### 🧪 质量
- 单测 4/4（新增相对路径/UNC/..防御）
- 发布闸门精确白名单（明示 tauri 框架 1 处痕迹，其余一律拒绝）

## [0.2.5] - 2026-08-30

### 🐛 修复
- 返回启动器事件失效（take 后判断永远 false）——Esc 返回启动器恢复
- 拦截系统级关闭（Alt+F4/任务栏关闭）：按设置统一退出（停核心）或隐藏托盘
- Node 安装后重检新增常见目录兜底（%ProgramFiles%\nodejs）
- dsh shim 定位兼容 .cmd/.bat（自定义 npm prefix 场景）
- 主题解析 UTF-8 安全切片（多字节字符边界，防 panic）
- 清理 main.js 旧设置弹窗死代码（IPC 契约断裂恢复）
- 3080 打开前校验占用者身份（防打开非 dsh 服务）

### ⚙️ 可靠性
- npm 安装/更新日志改流式实时（spawn + 按行追加，前端可见实时进度）
- 更新/安装 dsh 前检查核心运行（防文件占用失败）
- 壁纸：10MB 限制 + MIME 魔数判断（jpg/webp 不再误报 png）
- 设置保存统一原子写入（tmp + rename，防 JSON 损坏）
- 卸载路径防御（拒绝盘符根/用户目录，npx 文案明确）

### 🧪 质量
- 新增单元测试：semver 版本比较 / 镜像参数映射 / 卸载路径防御
- cargo fmt 全量格式化 + CI 扩充（fmt/clippy/test/版本一致性/IPC 契约）
- 发布闸门：版本号一致性 + 路径泄漏扫描
- SECURITY.md 新增；REQUIREMENTS.md 归档为历史文档

## [0.2.4] - 2026-08-26

### 🐛 安全修复
- 结束 3080 占用进程前先校验进程身份（is_dsh_pid）：非 dsh 进程拒绝结束，防误杀
- 移除过时校验和残留文件

### 📝 文档
- README：Node.js 引导措辞改为“打开官方下载页（LTS）”

## [0.2.3] - 2026-08-26

### ✨ 改进
- 软件自更新检查改为 WebView 直连（系统证书/代理自适应）：国内代理/MITM 环境下不再误报离线
- 检查失败时自动提供「去下载」降级入口（永不绝路）

## [0.2.2] - 2026-08-26

### 🔧 加固
- 崩溃钩子：任何 panic 都会写入运行日志（PANIC 崩溃: …），排障闭环

## [0.2.1] - 2026-08-26

### 🛠 优化
- 日志覆盖补充：dsh 更新 / 设置保存 / 最小化 / 返回主页 / **端口就绪时刻**（时序诊断盲区消除）
- 统一退出出口 `quit_app`：托盘右键、✕ 默认退出、弹窗确认退出三条路径全部先停核心
- 代码质量：clippy 5 条清零 / 脚本去重 / 镜像逻辑三处统一 / 设置保存失败落日志

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
