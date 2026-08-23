mod dsh_locator;
mod dsh_process;
mod env_check;
mod uninstaller;
mod updater;

use serde::Serialize;
use std::sync::Mutex;
use tauri::{LogicalPosition, LogicalSize, Rect, State, Window};

pub struct AppState {
    pub dsh: Mutex<dsh_process::DshState>,
    pub embed: Mutex<Option<tauri::webview::Webview>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            dsh: Mutex::new(dsh_process::DshState::default()),
            embed: Mutex::new(None),
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
            update_embed_bounds
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}