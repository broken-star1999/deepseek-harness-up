// 环境体检：Node / npm / dsh / WebView2 / 端口 五项
use serde::Serialize;
use std::path::Path;
use std::process::Command;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckItem {
    pub name: String,
    pub ok: bool,
    pub warn: bool,
    pub detail: String,
    pub action: Option<String>,
    pub action_label: Option<String>,
}

fn run_version(cmd: &str, arg: &str) -> Option<String> {
    // 通过 cmd /C 执行(node.exe 无窗口; npm.cmd 批处理必须经 cmd, 隐藏窗口)
    let out = crate::winutil::cmd_hidden(&["/C", cmd, arg]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn item(name: &str, ok: bool, warn: bool, detail: String, action: Option<&str>, action_label: Option<&str>) -> CheckItem {
    CheckItem {
        name: name.into(),
        ok,
        warn,
        detail,
        action: action.map(|s| s.into()),
        action_label: action_label.map(|s| s.into()),
    }
}

pub fn run() -> Vec<CheckItem> {
    let mut items = Vec::new();

    // 1. Node.js
    let node = run_version("node", "--version");
    items.push(item(
        "Node.js",
        node.is_some(),
        false,
        node.clone().unwrap_or_else(|| "未安装（dsh 运行必需）".into()),
        if node.is_none() { Some("node_download") } else { None },
        if node.is_none() { Some("下载并安装 Node.js") } else { None },
    ));

    // 2. npm
    let npm = run_version("npm", "--version");
    items.push(item(
        "npm",
        npm.is_some(),
        false,
        npm.clone().unwrap_or_else(|| "未安装".into()),
        if npm.is_none() { Some("node_download") } else { None },
        if npm.is_none() { Some("安装 Node.js（内含 npm）") } else { None },
    ));

    // 3. dsh 全局包
    let loc = crate::dsh_locator::locate();
    items.push(item(
        "dsh 全局包",
        loc.installed(),
        false,
        match loc.version() {
            Some(v) => format!("已安装 v{}", v),
            None => "未安装（npm i -g @deepseek-ai/dsh）".into(),
        },
        if loc.installed() { None } else { Some("install_dsh") },
        if loc.installed() { None } else { Some("一键安装 dsh") },
    ));

    // 4. WebView2 Runtime
    let wv = Path::new(r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application").exists()
        || Path::new(r"C:\Program Files\Microsoft\EdgeWebView\Application").exists();
    items.push(item(
        "WebView2 Runtime",
        wv,
        false,
        if wv { "已安装（Win11 自带或 Edge 已装）".into() } else { "未检测到".into() },
        if wv { None } else { Some("webview2_download") },
        if wv { None } else { Some("安装 WebView2") },
    ));

    // 5. 端口 3080
    let port = crate::dsh_process::port_open();
    items.push(item(
        "端口 3080",
        !port,
        false,
        if port {
            format!(
                "已被占用（PID {:?}）",
                crate::dsh_process::port_pid()
            )
        } else {
            "空闲".into()
        },
        if port { Some("stop_port_owner") } else { None },
        if port { Some("结束占用进程") } else { None },
    ));

    items
}