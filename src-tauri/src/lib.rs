mod dsh_locator;
mod dsh_process;
mod env_check;
mod uninstaller;
mod updater;
mod winutil;

use serde::Serialize;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, Rect, State, Window};

/// 全局操作互斥：安装/更新/卸载/启停不允许并发
static OPERATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 获取操作锁（先锁后检查，防竞态窗口）
fn op_guard() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    OPERATION_LOCK.lock().map_err(|_| "操作锁中毒".to_string())
}

pub struct AppState {
    pub dsh: Mutex<dsh_process::DshState>,
    pub embed: Mutex<Option<tauri::webview::Webview>>,
    pub controls: Mutex<Option<tauri::webview::Webview>>,
    /// 定位缓存：(时间戳, 结果)，10 秒内复用避免高频 cmd 调用
    pub locator_cache: Mutex<Option<(u128, dsh_locator::DshLocator)>>,
    pub settings_panel: Mutex<Option<tauri::webview::Webview>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            dsh: Mutex::new(dsh_process::DshState::default()),
            embed: Mutex::new(None),
            controls: Mutex::new(None),
            locator_cache: Mutex::new(None),
            settings_panel: Mutex::new(None),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusInfo {
    pub running: bool,
    pub booting: bool,
    pub installed: bool,
    pub dsh_version: Option<String>,
    pub app_version: String,
    pub detail: String,
    pub port_pid: Option<u32>,
    pub port_open: bool,
    pub update_available: bool,
    pub latest_version: Option<String>,
    pub node_version: Option<String>,
    pub npm_version: Option<String>,
}

#[tauri::command]
fn ping() -> String {
    "pong".into()
}

/// 安装/卸载后强制失效定位缓存（新机器安装 dsh 后立即生效）
#[tauri::command]
fn invalidate_locator_cache(state: State<AppState>) {
    *state.locator_cache.lock().unwrap() = None;
    log_line("locator cache invalidated");
}

#[tauri::command]
fn get_status(state: State<AppState>) -> StatusInfo {
    // 定位缓存：10 秒内复用（Node 目录遍历很慢，避免每次 get_status 跑 5 个 cmd）
    let loc = {
        let mut cache = state.locator_cache.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        if let Some((ts, cached)) = cache.as_ref() {
            if now.saturating_sub(*ts) < 10_000 {
                cached.clone()
            } else {
                let fresh = dsh_locator::locate();
                *cache = Some((now, fresh.clone()));
                fresh
            }
        } else {
            let fresh = dsh_locator::locate();
            *cache = Some((now, fresh.clone()));
            fresh
        }
    };
    let mut dsh = state.dsh.lock().unwrap();
    let running = dsh_process::running(&mut dsh);
    let booting = false;

    let detail = if !loc.installed() {
        "未检测到全局 dsh（环境体检页可一键安装）".into()
    } else if running {
        match dsh_process::port_pid() {
            Some(pid) => format!("正在运行（端口 3080, PID {}）", pid),
            None => "正在运行".into(),
        }
    } else {
        "未运行".into()
    };

    // 状态变化日志（核心启停/端口就绪/意外退出检测）
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static LAST: AtomicBool = AtomicBool::new(false);
        static LAST_PORT: AtomicBool = AtomicBool::new(false);
        let cur = running;
        let own = dsh_process::own_alive(&mut dsh);
        let port = dsh_process::port_open();
        if cur != LAST.swap(cur, Ordering::SeqCst) {
            log_line(&format!(
                "STATE dsh 状态变化: {} (own={} port={} installed={} dsh={:?})",
                if cur { "运行" } else { "停止" },
                own,
                port,
                loc.installed(),
                loc.version()
            ));
        }
        if port != LAST_PORT.swap(port, Ordering::SeqCst) {
            log_line(&format!(
                "STATE 端口 3080: {}",
                if port { "就绪" } else { "未监听" }
            ));
        }
    }
    StatusInfo {
        running,
        booting,
        installed: loc.installed(),
        dsh_version: loc.version(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        detail,
        port_pid: dsh_process::port_pid(),
        port_open: dsh_process::port_open(),
        update_available: false,
        latest_version: None,
        node_version: loc
            .node
            .as_deref()
            .and_then(|n| cmd_version_path(std::path::Path::new(n))),
        npm_version: crate::dsh_locator::npm_cmd_path().and_then(|npm| {
            let npm = npm.to_string_lossy().into_owned();
            // npm.cmd 是批处理文件，必须经隐藏 cmd /C 调用。
            let out = crate::winutil::cmd_hidden(&["/C", &npm, "-v"])
                .output()
                .ok()?;
            if out.status.success() {
                let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            } else {
                None
            }
        }),
    }
}

#[tauri::command]
fn start_dsh(state: State<AppState>) -> Result<serde_json::Value, String> {
    let _guard = op_guard()?;
    log_line("ACTION start_dsh: 请求启动核心");
    // 强制刷新定位缓存（安装后立即启动场景）
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let fresh = dsh_locator::locate();
        *state.locator_cache.lock().unwrap() = Some((now, fresh));
    }

    let loc = dsh_locator::locate();
    if !loc.installed() {
        return Err("未检测到全局 dsh，请先到体检页一键安装".into());
    }
    let node = loc
        .node
        .clone()
        .ok_or("未找到 node 可执行文件（请安装 Node.js）")?;
    let bin_js = loc.bin_js.clone().ok_or("未找到 dsh 包（lib/bin.js）")?;
    let mut dsh = state.dsh.lock().unwrap();
    dsh_process::start(&mut dsh, &node, bin_js.to_str().unwrap_or_default())?;
    Ok(serde_json::json!({ "ok": true, "message": "dsh 已启动" }))
}

#[tauri::command]
fn stop_dsh(state: State<AppState>) -> Result<serde_json::Value, String> {
    let _guard = op_guard()?;
    log_line("ACTION stop_dsh: 请求停止核心");
    let mut dsh = state.dsh.lock().unwrap();
    dsh_process::stop(&mut dsh)?;
    Ok(serde_json::json!({ "ok": true, "message": "dsh 已停止" }))
}

#[tauri::command]
fn check_update() -> Result<serde_json::Value, String> {
    log_line("ACTION check_update: 版本检查开始");
    let loc = dsh_locator::locate();
    let local = loc.version().ok_or("未检测到 dsh 全局包，无法比较版本")?;
    let latest = updater::registry_latest()?;
    let outdated = updater::is_outdated(&local, &latest);
    log_line(&format!(
        "check_update: local={} latest={} outdated={}",
        local, latest, outdated
    ));
    Ok(serde_json::json!({
        "online": true,
        "local": local,
        "latest": latest,
        "outdated": outdated,
    }))
}

#[tauri::command]
fn update_dsh() -> Result<serde_json::Value, String> {
    // 先锁后检查（防检查-执行竞态）
    let _guard = op_guard()?;
    if dsh_process::port_open() {
        return Err("dsh 核心正在运行，请先停止核心再更新".into());
    }
    log_line("ACTION update_dsh: 开始更新 dsh");
    let out = updater::update_dsh().map_err(|e| {
        log_line(&format!("update_dsh 失败: {}", e));
        e
    })?;
    log_line("update_dsh: 更新完成");
    Ok(serde_json::json!({ "ok": true, "message": format!("更新完成\n{}", out) }))
}

#[tauri::command]
fn env_check() -> Vec<env_check::CheckItem> {
    env_check::run()
}

#[tauri::command]
fn env_action(action: String, state: State<AppState>) -> Result<serde_json::Value, String> {
    log_line(&format!("ACTION env_action: {}", action));
    match action.as_str() {
        "node_download" => {
            open_url("https://nodejs.org/en/download")?;
            Ok(
                serde_json::json!({ "ok": true, "message": "已打开 Node.js 官方下载页，请下载 LTS 并安装" }),
            )
        }
        "install_dsh" => {
            let _guard = op_guard()?;
            let out = updater::install_dsh()?;
            Ok(serde_json::json!({ "ok": true, "message": format!("安装成功\n{}", out) }))
        }
        "webview2_download" => {
            open_url("https://developer.microsoft.com/microsoft-edge/webview2/")?;
            Ok(serde_json::json!({ "ok": true, "message": "已打开 WebView2 官方下载页" }))
        }
        "stop_port_owner" => {
            let _guard = op_guard()?;
            let mut dsh = state.dsh.lock().unwrap();
            dsh_process::stop(&mut dsh)?;
            Ok(serde_json::json!({ "ok": true, "message": "dsh 已停止" }))
        }
        other => Err(format!("未知操作: {}", other)),
    }
}

#[tauri::command]
fn uninstall(
    clear_config: bool,
    clear_npx: bool,
    state: State<AppState>,
) -> Result<serde_json::Value, String> {
    let _guard = op_guard()?;
    log_line(&format!(
        "ACTION uninstall: clearConfig={} clearNpx={}",
        clear_config, clear_npx
    ));
    // 先停止 dsh：只对自己启动的进程或已确认的 dsh 执行停止。
    {
        let mut dsh = state.dsh.lock().unwrap();
        if dsh_process::running(&mut dsh) {
            dsh_process::stop(&mut dsh).map_err(|e| format!("卸载前停止 dsh 失败: {}", e))?;
        }
    }
    let log = uninstaller::run(clear_config, clear_npx)?;
    Ok(serde_json::json!({ "ok": true, "message": log }))
}

/// 内嵌显示 127.0.0.1:3080（Windows 上必须在 async command 中创建 child webview，避免死锁）
const EMBED_LABEL: &str = "dsh-embed";
const DSH_UI_URL: &str = "http://127.0.0.1:3080/";

#[tauri::command]
async fn show_embed(
    window: Window,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<serde_json::Value, String> {
    log_line(&format!(
        "ACTION show_embed: rect=({:.0},{:.0} {}x{})",
        x, y, width, height
    ));
    if !dsh_process::port_open() {
        return Err("dsh 未运行（端口 3080 未就绪）".into());
    }
    // 安全校验（fail closed）：占用者未知或非 dsh → 拒绝嵌入
    match dsh_process::port_pid() {
        Some(pid) => {
            if !dsh_process::is_dsh_pid(pid) {
                log_line(&format!(
                    "show_embed: 3080 被非 dsh 进程占用 PID {}, 拒绝嵌入",
                    pid
                ));
                return Err(format!(
                    "端口 3080 被非 dsh 进程占用（PID {}），无法打开。请手动处理。",
                    pid
                ));
            }
        }
        None => {
            log_line("show_embed: 端口 3080 开放但无法确认占用者，拒绝嵌入");
            return Err(
                "端口 3080 开放但无法确认占用者身份，为安全起见拒绝打开。请手动处理。".into(),
            );
        }
    }
    let mut guard = state.embed.lock().unwrap();
    let rect = Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(width, height).into(),
    };
    if let Some(wv) = guard.as_ref() {
        // 已有内嵌：仅更新位置尺寸（窗口缩放时）
        wv.set_bounds(rect).map_err(|e| e.to_string())?;
        return Ok(serde_json::json!({ "ok": true }));
    }
    let builder = tauri::webview::WebviewBuilder::new(
        EMBED_LABEL,
        tauri::WebviewUrl::External(DSH_UI_URL.parse().unwrap()),
    );
    let wv = window
        .add_child(
            builder,
            LogicalPosition::new(x, y),
            LogicalSize::new(width, height),
        )
        .map_err(|e| format!("创建内嵌视图失败: {}", e))?;
    *guard = Some(wv);
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
async fn hide_embed(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.embed.lock().unwrap();
    if let Some(wv) = guard.take() {
        wv.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn update_embed_bounds(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let guard = state.embed.lock().unwrap();
    if let Some(wv) = guard.as_ref() {
        let rect = Rect {
            position: LogicalPosition::new(x, y).into(),
            size: LogicalSize::new(width, height).into(),
        };
        wv.set_bounds(rect).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 快速读取一个命令的版本输出（node --version / npm --version）
/// 从 DSH 页面 HTML 提取主题背景色（顶栏自动适配用）
#[tauri::command]
async fn get_dsh_theme() -> Option<String> {
    if !dsh_process::port_open() {
        return None;
    }
    let resp = ureq::get("http://127.0.0.1:3080/")
        .timeout(std::time::Duration::from_secs(2))
        .call()
        .ok()?;
    let html = resp.into_string().ok()?;
    extract_bg_hex(&html)
}

fn is_hex6(seg: &str) -> bool {
    seg.len() >= 7 && seg.as_bytes()[0] == b'#' && seg[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn extract_bg_hex(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let mut candidates: Vec<String> = Vec::new();
    let mut i = 0;
    // 扫描所有 background 声明（含 background-color / background-image 等）
    while let Some(pos) = lower[i..].find("background") {
        let start = i + pos;
        let look_end = (start + 140).min(lower.len());
        // UTF-8 安全边界：若落在多字节字符中间，向左回退到字符边界（避免 panic）
        let mut safe_end = look_end;
        while safe_end > start && !lower.is_char_boundary(safe_end) {
            safe_end -= 1;
        }
        let look = &lower[start..safe_end];
        // 该段内取第一个 #rrggbb 或 rgb() —— body 级别声明通常最先生效
        if let Some(hpos) = look.find('#') {
            let mut color_end = (hpos + 7).min(look.len());
            while color_end > hpos && !look.is_char_boundary(color_end) {
                color_end -= 1;
            }
            let seg = &look[hpos..color_end];
            if is_hex6(seg) {
                candidates.push(seg.to_string());
            }
        } else if let Some(rpos) = look.find("rgb(") {
            let mut color_end = (rpos + 32).min(look.len());
            while color_end > rpos && !look.is_char_boundary(color_end) {
                color_end -= 1;
            }
            let seg = &look[rpos..color_end];
            let nums: Vec<u32> = seg[4..]
                .split(|c: char| !c.is_ascii_digit())
                .filter(|s| !s.is_empty())
                .take(3)
                .filter_map(|s| s.parse().ok())
                .collect();
            if nums.len() == 3 {
                candidates.push(format!(
                    "#{:02x}{:02x}{:02x}",
                    nums[0].min(255),
                    nums[1].min(255),
                    nums[2].min(255)
                ));
            }
        }
        i = start + 10;
    }
    use std::collections::HashMap;
    let mut counts: HashMap<String, usize> = HashMap::new();
    for c in &candidates {
        *counts.entry(c.clone()).or_insert(0) += 1;
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| c)
}

/// ===== 悬浮控制（页面 2 的 ─ □ ✕）=====
const CONTROLS_LABEL: &str = "dsh-controls";

#[tauri::command]
async fn show_controls(
    window: Window,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<serde_json::Value, String> {
    let mut guard = state.controls.lock().unwrap();
    let rect = Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(width, height).into(),
    };
    if let Some(wv) = guard.as_ref() {
        wv.set_bounds(rect).map_err(|e| e.to_string())?;
        return Ok(serde_json::json!({ "ok": true }));
    }
    let builder = tauri::webview::WebviewBuilder::new(
        CONTROLS_LABEL,
        tauri::WebviewUrl::App("controls.html".into()),
    )
    .transparent(true);
    let wv = window
        .add_child(
            builder,
            LogicalPosition::new(x, y),
            LogicalSize::new(width, height),
        )
        .map_err(|e| format!("创建控制条失败: {}", e))?;
    *guard = Some(wv);
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
async fn hide_controls(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.controls.lock().unwrap();
    if let Some(wv) = guard.take() {
        wv.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn update_controls_bounds(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let guard = state.controls.lock().unwrap();
    if let Some(wv) = guard.as_ref() {
        let rect = Rect {
            position: LogicalPosition::new(x, y).into(),
            size: LogicalSize::new(width, height).into(),
        };
        wv.set_bounds(rect).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 回到页面 1：关闭两个子 webview，并通知主 webview 刷新状态
#[tauri::command]
async fn back_to_launcher(window: Window, state: State<'_, AppState>) -> Result<(), String> {
    log_line("back_to_launcher: 返回主页面");
    // 必须先记录再 take（take 后 Option 为空，后判断永远 false）
    let embed_was = state.embed.lock().unwrap().is_some();
    {
        let mut g = state.embed.lock().unwrap();
        if let Some(wv) = g.take() {
            let _ = wv.close();
        }
    }
    // 顶栏（controls）常驻，不关闭；仅恢复启动器主体
    if embed_was {
        let _ = window.emit("back-to-launcher", ());
    }
    Ok(())
}

/// 设置面板：主窗口内的 child webview（最后创建=顶层，盖住 DSH；
/// 全窗口透明 + DOM 遮罩 → 透出 DSH 背景；与主窗口绑定不分离）
#[tauri::command]
async fn show_settings_window(window: Window, state: State<'_, AppState>) -> Result<(), String> {
    log_line("show_settings_window (child webview)");
    let inner_size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().unwrap_or(1.0);
    if state.settings_panel.lock().unwrap().is_some() {
        return Ok(());
    }
    let builder = tauri::webview::WebviewBuilder::new(
        "settings-panel",
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .transparent(true);
    let wv = window
        .add_child(
            builder,
            LogicalPosition::new(0.0, 0.0),
            LogicalSize::new(
                inner_size.width as f64 / scale,
                inner_size.height as f64 / scale,
            ),
        )
        .map_err(|e| format!("创建设置面板失败: {}", e))?;
    *state.settings_panel.lock().unwrap() = Some(wv);
    Ok(())
}

#[tauri::command]
async fn hide_settings_window(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.settings_panel.lock().unwrap();
    if let Some(wv) = guard.take() {
        let _ = wv.close();
    }
    Ok(())
}

/// 「─」最小化 = 隐藏到系统托盘（任务栏无按钮，托盘图标唤回）
#[tauri::command]
fn win_hide_tray(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_window("main") {
        // 经典组合：先最小化（消除任务栏按钮残留）再隐藏
        let _ = main.minimize();
        match main.hide() {
            Ok(()) => {
                log_line("win_minimize: hidden to tray (button path)");
                return Ok(());
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("主窗口不存在".into())
}

#[tauri::command]
fn win_minimize(app: tauri::AppHandle) -> Result<(), String> {
    log_line("win_minimize: window minimized");
    app.get_window("main")
        .ok_or("主窗口不存在")?
        .minimize()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn win_toggle_maximize(app: tauri::AppHandle) -> Result<(), String> {
    let win = app.get_window("main").ok_or("主窗口不存在")?;
    if win.is_maximized().unwrap_or(false) {
        win.unmaximize().map_err(|e| e.to_string())
    } else {
        win.maximize().map_err(|e| e.to_string())
    }
}

/// 唯一退出出口：停止 dsh 核心 → 记录 → 退出。
/// 所有退出路径（✕ 默认退出 / 弹窗确认 / 托盘菜单退出）必须经过这里，防止漏停核心。
fn quit_app(app: &tauri::AppHandle) {
    // 退出也走操作锁（停核心与启停/更新不并发）
    if let Ok(_guard) = op_guard() {
        let state = app.state::<AppState>();
        let mut dsh = state.dsh.lock().unwrap();
        match dsh_process::stop(&mut dsh) {
            Ok(()) => log_line("quit_app: dsh stopped"),
            Err(e) => log_line(&format!("quit_app: dsh stop note={}", e)),
        }
        drop(dsh);
        log_line("quit_app: app.exit(0)");
        app.exit(0);
    } else {
        // 锁异常时至少保证退出（不阻塞用户关闭）
        log_line("quit_app: 操作锁未获取，直接退出");
        app.exit(0);
    }
}

#[tauri::command]
fn win_close(app: tauri::AppHandle) -> Result<(), String> {
    quit_app(&app);
    Ok(())
}

/// ===== 独立悬浮弹窗窗口（始终置顶，不依赖 child webview 层叠）=====

#[tauri::command]
async fn show_dialog_window(app: tauri::AppHandle) -> Result<(), String> {
    // 诊断：列出全部窗口 label
    let labels: Vec<String> = app.windows().keys().cloned().collect();
    log_line(&format!("show_dialog: all windows={:?}", labels));

    // 已存在：也重新居中（不依赖首次定位）
    if let Some(w) = app.get_window("confirm-dialog") {
        let _ = w.center();
        let _ = w.set_focus();
        return Ok(());
    }
    // 构建后直接以物理坐标精确居中于主窗口（零换算误差）
    let win = tauri::WebviewWindowBuilder::new(
        &app,
        "confirm-dialog",
        tauri::WebviewUrl::App("modal.html".into()),
    )
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .inner_size(380.0, 280.0)
    .visible(false) // 先隐藏：定位完成后才显示，避免"先左上后中心"的闪烁
    .build()
    .map_err(|e| format!("创建弹窗窗口失败: {}", e))?;

    // 精确居中于主窗口：物理坐标直算 + 数据日志（若仍偏，日志可一次性校正）
    if let Some(mw) = app.get_window("main") {
        let p = mw.outer_position().map(|v| (v.x, v.y)).unwrap_or((0, 0));
        let size = mw
            .outer_size()
            .map(|v| (v.width, v.height))
            .unwrap_or((0, 0));
        let scale = mw.scale_factor().unwrap_or(1.0);
        // 弹窗左上角(物理) = 主窗口中心(物理) - 弹窗半宽(物理)
        let cx = p.0 as f64 + size.0 as f64 / 2.0 - 190.0 * scale;
        let cy = p.1 as f64 + size.1 as f64 / 2.0 - 140.0 * scale;
        let r = win.set_position(tauri::PhysicalPosition::new(cx, cy));
        let actual = win.outer_position().map(|a| (a.x, a.y)).ok();
        log_line(&format!(
            "dialog center: p={:?} size={:?} scale={} set=({:.1},{:.1}) res={:?} actual={:?}",
            p,
            size,
            scale,
            cx,
            cy,
            r.map(|_| "ok"),
            actual
        ));
    }
    let _ = win.show();
    Ok(())
}

/// 弹窗确认（单 IPC：存记忆 → 关弹窗 → 执行动作）。
/// 关键：不能在 JS 侧先关弹窗窗口再发第二个命令（webview 销毁后 invoke 无法发出）
#[tauri::command]
async fn dialog_confirm(
    app: tauri::AppHandle,
    mode: String,
    no_remind: bool,
) -> Result<(), String> {
    log_line(&format!(
        "dialog_confirm: mode={} no_remind={}",
        mode, no_remind
    ));
    if no_remind && (mode == "exit" || mode == "minimize") {
        let mut s = settings_snapshot();
        s["close_default"] = serde_json::json!(mode);
        let dir = settings_path().parent().unwrap().to_path_buf();
        let _ = std::fs::create_dir_all(&dir);
        save_settings(&s).map_err(|__e| {
            log_line(&format!("保存设置失败: {}", __e));
            __e
        })?;
        log_line(&format!("dialog_confirm: saved default={}", mode));
    }
    if let Some(w) = app.get_window("confirm-dialog") {
        let _ = w.close();
    }
    if mode == "exit" {
        // 唯一退出出口（停核心 + 退出）
        quit_app(&app);
    } else if let Some(main) = app.get_window("main") {
        // 隐藏到任务栏托盘：先最小化(消除任务栏按钮残留)再隐藏——与「─」一致
        let _ = main.minimize();
        match main.hide() {
            Ok(()) => log_line("dialog_confirm: hidden to tray ok"),
            Err(e) => log_line(&format!("dialog_confirm: hide err={}", e)),
        }
    }
    Ok(())
}

#[tauri::command]
async fn hide_dialog_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_window("confirm-dialog") {
        let _ = w.close();
    }
    Ok(())
}

#[tauri::command]
fn start_drag(window: Window) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

/// ===== 设置存储（%LOCALAPPDATA%/dsh-up/settings.json）=====
fn settings_path() -> std::path::PathBuf {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    std::path::PathBuf::from(la)
        .join("dsh-up")
        .join("settings.json")
}

/// 设置写入锁：保护读-改-写事务，避免多个 IPC 调用互相覆盖。
static SETTINGS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static SETTINGS_TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn settings_tmp_path(p: &std::path::Path) -> std::path::PathBuf {
    use std::sync::atomic::Ordering;
    let n = SETTINGS_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    p.with_file_name(format!("settings.json.{}.{}.tmp", std::process::id(), n))
}

/// 使用替换语义把临时文件移动到目标文件；Windows 上使用 MoveFileExW。
fn replace_file(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let from: Vec<u16> = src.as_os_str().encode_wide().chain([0]).collect();
        let to: Vec<u16> = dst.as_os_str().encode_wide().chain([0]).collect();
        let ok = unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            return Err(format!(
                "替换设置文件失败: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(src, dst).map_err(|e| format!("替换设置文件失败: {}", e))
    }
}

/// 统一设置写入：唯一临时文件 → 备份旧文件 → 原子替换新文件。
fn save_settings(s: &serde_json::Value) -> Result<(), String> {
    use std::io::Write;
    let _guard = SETTINGS_LOCK.lock().map_err(|_| "设置锁中毒".to_string())?;
    let p = settings_path();
    let dir = p.parent().ok_or_else(|| "设置路径没有父目录".to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| format!("创建设置目录失败: {}", e))?;

    let tmp = settings_tmp_path(&p);
    let data = serde_json::to_vec_pretty(s).map_err(|e| format!("序列化设置失败: {}", e))?;
    let write_result = (|| -> Result<(), String> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| format!("创建临时设置文件失败: {}", e))?;
        f.write_all(&data)
            .map_err(|e| format!("写入临时设置文件失败: {}", e))?;
        f.flush()
            .map_err(|e| format!("刷新临时设置文件失败: {}", e))?;
        f.sync_all()
            .map_err(|e| format!("落盘临时设置文件失败: {}", e))?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }

    let bak = p.with_file_name("settings.json.bak");
    if p.is_file() {
        std::fs::copy(&p, &bak).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("备份旧设置失败: {}", e)
        })?;
    }
    if let Err(e) = replace_file(&tmp, &p) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// 运行日志（%LOCALAPPDATA%\dsh-up\log.txt）：
/// - 记录所有运行路径（启动/检查/弹窗/更新/卸载/错误）
/// - 超过 1MB 自动轮转为 log.1.txt（保留最近一份）
fn log_line(msg: &str) {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let p = std::path::PathBuf::from(la).join("dsh-up").join("log.txt");
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    // 轮转：> 1MB → log.1.txt 覆盖保留
    if let Ok(md) = std::fs::metadata(&p) {
        if md.len() > 1024 * 1024 {
            let _ = std::fs::rename(&p, p.with_file_name("log.1.txt"));
        }
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)
    {
        let _ = writeln!(f, "[{}] {}", ts, msg);
    }
}

/// 安装日志尾部（前端安装进度滚动显示）
#[tauri::command]
fn tail_install_log() -> String {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let p = std::path::PathBuf::from(la)
        .join("dsh-up")
        .join("install.log");
    match std::fs::read_to_string(&p) {
        Ok(s) => {
            let tail: String = s
                .chars()
                .rev()
                .take(800)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            tail
        }
        Err(_) => String::new(),
    }
}

/// 打开系统默认浏览器访问外部链接（更新下载/文档）
#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("仅允许 http(s) 链接".into());
    }
    log_line(&format!("open_external: {}", url));
    open_url(&url)
}

/// 设置 dsh 更新镜像（npmmirror / npmjs / custom:<url>）
#[tauri::command]
fn set_mirror(mirror: String) -> Result<(), String> {
    let allowed = mirror == "npmmirror" || mirror == "npmjs" || mirror.starts_with("custom:");
    if !allowed {
        return Err("无效镜像".into());
    }
    let mut s = settings_snapshot();
    s["mirror"] = serde_json::json!(mirror);
    save_settings(&s)?;
    log_line(&format!("set_mirror: {}", mirror));
    Ok(())
}

#[tauri::command]
fn get_mirror() -> Option<String> {
    settings_snapshot()
        .get("mirror")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[tauri::command]
fn open_logs() -> Result<(), String> {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let dir = std::path::PathBuf::from(&la).join("dsh-up");
    crate::winutil::exe_hidden("explorer.exe", &[dir.to_string_lossy().as_ref()])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 前端上报日志（JS console.error / 未捕获异常 → 统一入日志）
#[tauri::command]
fn fe_log(msg: String) {
    log_line(&format!("FRONTEND {}", msg));
}

fn settings_snapshot() -> serde_json::Value {
    let _guard = SETTINGS_LOCK.lock().ok();
    let load = |p: &std::path::Path| -> Option<serde_json::Value> {
        std::fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    };
    let p = settings_path();
    if let Some(v) = load(&p) {
        return v;
    }
    // 主文件损坏/缺失 → 尝试 .bak 恢复。
    let bak = p.with_file_name("settings.json.bak");
    if let Some(v) = load(&bak) {
        log_line("settings_snapshot: 主文件不可用，已从 .bak 恢复");
        if let Err(e) = std::fs::copy(&bak, &p) {
            log_line(&format!("settings_snapshot: 恢复主文件失败: {}", e));
        }
        return v;
    }
    serde_json::json!({})
}

/// 系统文件选择对话框选壁纸（PowerShell OpenFileDialog，独立窗口）
#[tauri::command]
fn pick_and_set_bg() -> Result<String, String> {
    let script = "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Filter = '图片文件 (*.png;*.jpg;*.jpeg;*.webp)|*.png;*.jpg;*.jpeg;*.webp|所有文件 (*.*)|*.*'; $d.Title = '选择启动器壁纸'; if ($d.ShowDialog() -eq 'OK') { Write-Output $d.FileName } else { Write-Output '' }";
    let out = crate::winutil::exe_hidden(
        "powershell.exe",
        &[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ],
    )
    .output()
    .map_err(|e| format!("打开文件对话框失败: {}", e))?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        return Ok("cancelled".into()); // 用户取消
    }
    // 校验：大小 <= 10MB + PNG/JPEG/WebP 魔数（不支持的格式直接拒绝）
    let meta = std::fs::metadata(&path).map_err(|e| format!("读取文件失败: {}", e))?;
    if meta.len() > 10 * 1024 * 1024 {
        return Err("图片超过 10MB 上限，请选择更小的图片".into());
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("读取图片失败: {}", e))?;
    if detect_image_mime(&bytes).is_none() {
        return Err("不支持的图片格式（仅支持 PNG/JPEG/WebP）".into());
    }
    // 写入固定位置 bg.png + 写 settings
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let dir = std::path::PathBuf::from(&la).join("dsh-up");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join("bg.png");
    std::fs::write(&dest, &bytes).map_err(|e| format!("保存图片失败: {}", e))?;
    let mut s = settings_snapshot();
    s["bg"] = serde_json::json!(dest.to_string_lossy().as_ref());
    save_settings(&s).inspect_err(|_| {
        let _ = std::fs::remove_file(&dest);
    })?;
    log_line(&format!("set_bg: from {} -> {}", path, dest.display()));
    Ok(dest.to_string_lossy().to_string())
}

/// 保存自定义壁纸（字节 → %LOCALAPPDATA%/dsh-up/bg.png + settings 记录）
#[tauri::command]
fn set_bg_bytes(data: Vec<u8>) -> Result<String, String> {
    // 壁纸大小上限 10MB（防大图占用内存/IPC）
    if data.len() > 10 * 1024 * 1024 {
        return Err("图片超过 10MB 上限，请选择更小的图片".into());
    }
    if detect_image_mime(&data).is_none() {
        return Err("不支持的图片格式（仅支持 PNG/JPEG/WebP）".into());
    }
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let dir = std::path::PathBuf::from(&la).join("dsh-up");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("bg.png");
    std::fs::write(&path, &data).map_err(|e| e.to_string())?;
    let mut s = settings_snapshot();
    s["bg"] = serde_json::json!(path.to_string_lossy());
    save_settings(&s)?;
    log_line("set_bg: saved");
    Ok(path.to_string_lossy().to_string())
}

/// 恢复默认壁纸（清 settings.bg + 删除自定义文件）
#[tauri::command]
fn reset_bg() -> Result<(), String> {
    let mut s = settings_snapshot();
    if let Some(m) = s.as_object_mut() {
        m.remove("bg");
    }
    save_settings(&s)?;
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let custom = std::path::PathBuf::from(&la).join("dsh-up").join("bg.png");
    let _ = std::fs::remove_file(&custom);
    log_line("reset_bg: 恢复默认壁纸");
    Ok(())
}

/// 用绝对路径执行 <exe> --version（定位链统一入口，不依赖 PATH）
fn cmd_version_path(exe: &std::path::Path) -> Option<String> {
    let out = crate::winutil::exe_hidden(exe.to_str()?, &["--version"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 4 && bytes[..4] == [0x89, 0x50, 0x4E, 0x47] {
        Some("image/png")
    } else if bytes.len() >= 3 && bytes[..3] == [0xFF, 0xD8, 0xFF] {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// 手动 base64（无 crate 依赖）
fn b64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        s.push(T[((n >> 18) & 63) as usize] as char);
        s.push(T[((n >> 12) & 63) as usize] as char);
        s.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}

/// 自定义壁纸 → data URI（绕开 asset 协议权限，前端直接作为背景）
#[tauri::command]
fn get_bg_data() -> Option<String> {
    let path = settings_snapshot()
        .get("bg")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())?;
    let bytes = std::fs::read(&path).ok()?;
    // 二次保护：异常大文件直接拒绝
    if bytes.len() > 20 * 1024 * 1024 {
        return None;
    }
    let lower = path.to_lowercase();
    // MIME 优先按文件头魔数判断；旧配置无魔数时再按扩展名兼容。
    let mime = detect_image_mime(&bytes).or_else(|| {
        if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
            Some("image/jpeg")
        } else if lower.ends_with(".webp") {
            Some("image/webp")
        } else if lower.ends_with(".png") {
            Some("image/png")
        } else {
            None
        }
    })?;
    Some(format!("data:{};base64,{}", mime, b64_encode(&bytes)))
}

#[tauri::command]
fn get_bg() -> Option<String> {
    settings_snapshot()
        .get("bg")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[tauri::command]
fn get_close_default() -> Option<String> {
    settings_snapshot()
        .get("close_default")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[tauri::command]
fn set_close_default(value: String) -> Result<(), String> {
    if value != "exit" && value != "minimize" {
        return Err("无效的默认值".into());
    }
    let mut s = settings_snapshot();
    s["close_default"] = serde_json::json!(value);
    save_settings(&s)?;
    log_line(&format!("set_close_default: 已保存 value={}", value));
    Ok(())
}

/// ===== 设置存储结束 =====
fn open_url(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        crate::winutil::exe_hidden("explorer.exe", &[url])
            .spawn()
            .map_err(|e| format!("打开链接失败: {}", e))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("仅支持 Windows".into())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 崩溃钩子：任何 panic 都写入运行日志（排障闭环）
    std::panic::set_hook(Box::new(|info| {
        crate::log_line(&format!("PANIC 崩溃: {}", info));
    }));
    // ===== 单实例锁（纯 std，零插件）=====
    // 绑定 127.0.0.1:3081 作为实例锁：
    //   成功 = 唯一实例（独占端口，全生命周期持有，唤醒监听）
    //   失败 = 已有实例 → 发送唤醒信号（TCP 连接）→ 本实例退出
    let instance_listener: Option<std::net::TcpListener> =
        match std::net::TcpListener::bind("127.0.0.1:3081") {
            Ok(l) => {
                log_line("single-instance: 唯一实例（3081 锁已持有）");
                Some(l)
            }
            Err(_) => {
                log_line("single-instance: 检测到已有实例，发送唤醒信号");
                let _ = std::net::TcpStream::connect("127.0.0.1:3081");
                std::process::exit(0);
            }
        };

    tauri::Builder::default()
        .setup(move |app| {
            // ===== 唤醒监听线程：收到连接 → 显示并聚焦主窗口 =====
            if let Some(listener) = instance_listener {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    for conn in listener.incoming() {
                        if conn.is_ok() {
                            log_line("single-instance: 唤醒请求 → 显示主窗口");
                            if let Some(w) = handle.get_window("main") {
                                let _ = w.unminimize();
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                });
            }
            // 系统托盘：常驻右下角，恢复窗口/退出入口
            let show_item = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            TrayIconBuilder::with_id("dsh-up-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("DeepSeek Harness Up")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        quit_app(app);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // 左键单击托盘 → 恢复主窗口
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_window("main") {
                            // 先还原最小化态，再显示前置（直接整窗上屏）
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            ping,
            fe_log,
            get_status,
            start_dsh,
            stop_dsh,
            check_update,
            update_dsh,
            env_check,
            env_action,
            uninstall,
            show_embed,
            hide_embed,
            update_embed_bounds,
            show_controls,
            hide_controls,
            update_controls_bounds,
            show_dialog_window,
            hide_dialog_window,
            dialog_confirm,
            back_to_launcher,
            win_minimize,
            win_toggle_maximize,
            win_hide_tray,
            show_settings_window,
            hide_settings_window,
            win_close,
            start_drag,
            get_close_default,
            set_close_default,
            invalidate_locator_cache,
            tail_install_log,
            set_bg_bytes,
            get_bg,
            pick_and_set_bg,
            reset_bg,
            get_bg_data,
            open_external,
            set_mirror,
            get_mirror,
            open_logs,
            get_dsh_theme
        ])
        .on_window_event(|window, event| {
            // 统一拦截系统级关闭（Alt+F4/任务栏关闭等）：先 prevent_close 阻断默认关闭
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let mode = settings_snapshot()
                        .get("close_default")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    match mode.as_deref() {
                        Some("minimize") => {
                            log_line("CloseRequested: 隐藏到托盘");
                            let _ = window.minimize();
                            let _ = window.hide();
                        }
                        Some("exit") => {
                            log_line("CloseRequested: 按设置退出");
                            quit_app(window.app_handle());
                        }
                        _ => {
                            // 未保存默认：与 ✕ 一致，弹确认窗决策
                            log_line("CloseRequested: 未设置默认，弹确认窗");
                            let app = window.app_handle().clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = show_dialog_window(app).await;
                            });
                        }
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
