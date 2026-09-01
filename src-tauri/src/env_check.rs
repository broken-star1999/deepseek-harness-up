// 环境体检：Node / npm / dsh / WebView2 / 端口 五项
use serde::Serialize;
use std::path::Path;

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
    // 优先使用定位器返回的绝对路径，避免安装 Node 后当前进程 PATH 尚未刷新。
    let (exe, is_batch) = match cmd {
        "node" => (crate::dsh_locator::locate().node?, false),
        "npm" => (
            crate::dsh_locator::npm_cmd_path()?
                .to_string_lossy()
                .into_owned(),
            true,
        ),
        other => (other.to_string(), false),
    };
    let out = if is_batch {
        // npm.cmd 是批处理文件，统一使用参数引用和隐藏 cmd 调用。
        crate::winutil::batch_hidden(Path::new(&exe), &[arg])
            .ok()?
            .output()
            .ok()?
    } else {
        crate::winutil::exe_hidden(&exe, &[arg]).output().ok()?
    };
    if !out.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn webview2_installed() -> bool {
    let mut roots = vec![
        Path::new(r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application").to_path_buf(),
        Path::new(r"C:\Program Files\Microsoft\EdgeWebView\Application").to_path_buf(),
    ];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(
            Path::new(&local)
                .join("Microsoft")
                .join("EdgeWebView")
                .join("Application"),
        );
    }
    roots.into_iter().any(|root| root.is_dir())
}

fn item(
    name: &str,
    ok: bool,
    warn: bool,
    detail: String,
    action: Option<&str>,
    action_label: Option<&str>,
) -> CheckItem {
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
        node.clone()
            .unwrap_or_else(|| "未安装（dsh 运行必需）".into()),
        if node.is_none() {
            Some("node_download")
        } else {
            None
        },
        if node.is_none() {
            Some("下载并安装 Node.js")
        } else {
            None
        },
    ));

    // 2. npm
    let npm = run_version("npm", "--version");
    items.push(item(
        "npm",
        npm.is_some(),
        false,
        npm.clone().unwrap_or_else(|| "未安装".into()),
        if npm.is_none() {
            Some("node_download")
        } else {
            None
        },
        if npm.is_none() {
            Some("安装 Node.js（内含 npm）")
        } else {
            None
        },
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
        if loc.installed() {
            None
        } else {
            Some("install_dsh")
        },
        if loc.installed() {
            None
        } else {
            Some("一键安装 dsh")
        },
    ));

    // 4. WebView2 Runtime
    let wv = webview2_installed();
    items.push(item(
        "WebView2 Runtime",
        wv,
        false,
        if wv {
            "已安装（Win11 自带或 Edge 已装）".into()
        } else {
            "未检测到".into()
        },
        if wv { None } else { Some("webview2_download") },
        if wv { None } else { Some("安装 WebView2") },
    ));

    // 5. 端口 3080
    let port = crate::dsh_process::port_open();
    let owner = crate::dsh_process::port_pid();
    let dsh_owner = owner.map(crate::dsh_process::is_dsh_pid).unwrap_or(false);
    items.push(item(
        "端口 3080",
        !port || dsh_owner,
        port && !dsh_owner,
        if port {
            if dsh_owner {
                format!("dsh 正在使用（PID {:?}）", owner)
            } else {
                format!("被非 dsh 进程占用（PID {:?}）", owner)
            }
        } else {
            "空闲".into()
        },
        if dsh_owner {
            Some("stop_port_owner")
        } else {
            None
        },
        if dsh_owner { Some("停止 dsh") } else { None },
    ));

    items
}
