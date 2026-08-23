mod dsh_locator;
mod dsh_process;
mod env_check;
mod uninstaller;
mod updater;

use serde::Serialize;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, Rect, State, Window};

pub struct AppState {
    pub dsh: Mutex<dsh_process::DshState>,
    pub embed: Mutex<Option<tauri::webview::Webview>>,
    pub controls: Mutex<Option<tauri::webview::Webview>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            dsh: Mutex::new(dsh_process::DshState::default()),
            embed: Mutex::new(None),
            controls: Mutex::new(None),
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
    pub update_available: bool,
    pub latest_version: Option<String>,
    pub node_version: Option<String>,
    pub npm_version: Option<String>,
}

#[tauri::command]
fn ping() -> String {
    "pong".into()
}

#[tauri::command]
fn get_status(state: State<AppState>) -> StatusInfo {
    let mut dsh = state.dsh.lock().unwrap();
    let loc = dsh_locator::locate();
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

    StatusInfo {
        running,
        booting,
        installed: loc.installed(),
        dsh_version: loc.version(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        detail,
        port_pid: dsh_process::port_pid(),
        update_available: false,
        latest_version: None,
        node_version: cmd_version("node"),
        npm_version: cmd_version("npm"),
    }
}

#[tauri::command]
fn start_dsh(state: State<AppState>) -> Result<serde_json::Value, String> {
    let loc = dsh_locator::locate();
    if !loc.installed() {
        return Err("未检测到全局 dsh，请先到体检页一键安装".into());
    }
    let node = loc.node.clone().ok_or("未找到 node 可执行文件（请安装 Node.js）")?;
    let bin_js = loc.bin_js.clone().ok_or("未找到 dsh 包（lib/bin.js）")?;
    let mut dsh = state.dsh.lock().unwrap();
    dsh_process::start(&mut dsh, &node, bin_js.to_str().unwrap_or_default())?;
    Ok(serde_json::json!({ "ok": true, "message": "dsh 已启动" }))
}

#[tauri::command]
fn stop_dsh(state: State<AppState>) -> Result<serde_json::Value, String> {
    let mut dsh = state.dsh.lock().unwrap();
    dsh_process::stop(&mut dsh)?;
    Ok(serde_json::json!({ "ok": true, "message": "dsh 已停止" }))
}

#[tauri::command]
fn check_update() -> Result<serde_json::Value, String> {
    let loc = dsh_locator::locate();
    let local = loc
        .version()
        .ok_or("未检测到 dsh 全局包，无法比较版本")?;
    let latest = updater::registry_latest()?;
    let outdated = updater::is_outdated(&local, &latest);
    Ok(serde_json::json!({
        "online": true,
        "local": local,
        "latest": latest,
        "outdated": outdated,
    }))
}

#[tauri::command]
fn update_dsh() -> Result<serde_json::Value, String> {
    let out = updater::update_dsh()?;
    Ok(serde_json::json!({ "ok": true, "message": format!("更新完成\n{}", out) }))
}

#[tauri::command]
fn env_check() -> Vec<env_check::CheckItem> {
    env_check::run()
}

#[tauri::command]
fn env_action(action: String) -> Result<serde_json::Value, String> {
    match action.as_str() {
        "node_download" => {
            open_url("https://nodejs.org/en/download")?;
            Ok(serde_json::json!({ "ok": true, "message": "已打开 Node.js 官方下载页，请下载 LTS 并安装" }))
        }
        "install_dsh" => {
            let out = updater::install_dsh()?;
            Ok(serde_json::json!({ "ok": true, "message": format!("安装成功\n{}", out) }))
        }
        "webview2_download" => {
            open_url("https://developer.microsoft.com/microsoft-edge/webview2/")?;
            Ok(serde_json::json!({ "ok": true, "message": "已打开 WebView2 官方下载页" }))
        }
        "stop_port_owner" => {
            if let Some(pid) = dsh_process::port_pid() {
                let ok = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                if ok {
                    return Ok(serde_json::json!({ "ok": true, "message": format!("已结束占用端口 3080 的进程 PID {}", pid) }));
                }
            }
            Err("无法结束占用进程（可能无权限），请手动处理".into())
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
    // 先停止 dsh（若本工具管理的进程在跑）
    {
        let mut dsh = state.dsh.lock().unwrap();
        if dsh_process::own_alive(&mut dsh) {
            let _ = dsh_process::stop(&mut dsh);
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
    if !dsh_process::port_open() {
        return Err("dsh 未运行（端口 3080 未就绪）".into());
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
        .add_child(builder, LogicalPosition::new(x, y), LogicalSize::new(width, height))
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
    seg.len() >= 7
        && seg.as_bytes()[0] == b'#'
        && seg[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn extract_bg_hex(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let mut candidates: Vec<String> = Vec::new();
    let mut i = 0;
    // 扫描所有 background 声明（含 background-color / background-image 等）
    while let Some(pos) = lower[i..].find("background") {
        let start = i + pos;
        let look_end = (start + 140).min(lower.len());
        let look = &lower[start..look_end];
        // 该段内取第一个 #rrggbb 或 rgb() —— body 级别声明通常最先生效
        if let Some(hpos) = look.find('#') {
            let seg = &look[hpos..(hpos + 7).min(look.len())];
            if is_hex6(seg) {
                candidates.push(seg.to_string());
            }
        } else if let Some(rpos) = look.find("rgb(") {
            let seg = &look[rpos..(rpos + 32).min(look.len())];
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

fn cmd_version(cmd: &str) -> Option<String> {
    let out = std::process::Command::new(cmd).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
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
        .add_child(builder, LogicalPosition::new(x, y), LogicalSize::new(width, height))
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
async fn back_to_launcher(
    window: Window,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut g = state.embed.lock().unwrap();
        if let Some(wv) = g.take() {
            let _ = wv.close();
        }
    }
    // 顶栏（controls）常驻，不关闭；仅恢复启动器主体
    let embed_was = state.embed.lock().unwrap().is_some();
    if embed_was {
        let _ = window.emit("back-to-launcher", ());
    }
    Ok(())
}

#[tauri::command]
fn win_minimize(app: tauri::AppHandle) -> Result<(), String> {
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

#[tauri::command]
fn win_close(app: tauri::AppHandle) -> Result<(), String> {
    log_line("win_close: app.exit(0)");
    app.exit(0);
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
        let p = mw
            .outer_position()
            .map(|v| (v.x, v.y))
            .unwrap_or((0, 0));
        let size = mw
            .outer_size()
            .map(|v| (v.width, v.height))
            .unwrap_or((0, 0));
        let scale = mw.scale_factor().unwrap_or(1.0);
        // 弹窗左上角(物理) = 主窗口中心(物理) - 弹窗半宽(物理)
        let cx = p.0 as f64 + size.0 as f64 / 2.0 - 190.0 * scale;
        let cy = p.1 as f64 + size.1 as f64 / 2.0 - 140.0 * scale;
        let r = win.set_position(tauri::PhysicalPosition::new(cx, cy));
        let actual = win
            .outer_position()
            .map(|a| (a.x, a.y))
            .ok();
        log_line(&format!(
            "dialog center: p={:?} size={:?} scale={} set=({:.1},{:.1}) res={:?} actual={:?}",
            p, size, scale, cx, cy, r.map(|_| "ok"), actual
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
    state: State<'_, AppState>,
    mode: String,
    no_remind: bool,
) -> Result<(), String> {
    log_line(&format!("dialog_confirm: mode={} no_remind={}", mode, no_remind));
    if no_remind && (mode == "exit" || mode == "minimize") {
        let mut s = settings_snapshot();
        s["close_default"] = serde_json::json!(mode);
        let dir = settings_path().parent().unwrap().to_path_buf();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(settings_path(), serde_json::to_string_pretty(&s).unwrap());
        log_line(&format!("dialog_confirm: saved default={}", mode));
    }
    if let Some(w) = app.get_window("confirm-dialog") {
        let _ = w.close();
    }
    if mode == "exit" {
        // 直接退出 = 程序退出 + 结束 dsh 服务（用户需求）
        let mut dsh = state.dsh.lock().unwrap();
        match dsh_process::stop(&mut dsh) {
            Ok(()) => log_line("dialog_confirm: dsh stopped (exit)"),
            Err(e) => log_line(&format!("dialog_confirm: dsh stop note={}", e)),
        }
        drop(dsh);
        log_line("dialog_confirm: app.exit(0)");
        app.exit(0);
    } else if let Some(main) = app.get_window("main") {
        // 隐藏到任务栏托盘：窗口隐藏（任务栏无按钮），托盘图标可点击恢复
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
    std::path::PathBuf::from(la).join("dsh-up").join("settings.json")
}

/// 调试日志（%LOCALAPPDATA%\dsh-up\log.txt）
fn log_line(msg: &str) {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let p = std::path::PathBuf::from(la).join("dsh-up").join("log.txt");
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "[{}] {}", ts, msg);
    }
}

fn settings_snapshot() -> serde_json::Value {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}))
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
    let dir = settings_path().parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(settings_path(), serde_json::to_string_pretty(&s).unwrap())
        .map_err(|e| e.to_string())
}

/// ===== 设置存储结束 =====

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|e| format!("打开链接失败: {}", e))?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("仅支持 Windows".into())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 系统托盘：常驻右下角，恢复窗口/退出入口
            let show_item = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            TrayIconBuilder::with_id("dsh-up-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("dsh-up Desktop")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
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
            win_close,
            start_drag,
            get_close_default,
            set_close_default,
            get_dsh_theme
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}