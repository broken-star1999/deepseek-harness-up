// dsh 进程生命周期：隐藏 spawn / 三层状态检测 / 停止
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub const DSH_PORT: u16 = 3080;
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Default)]
pub struct DshState {
    pub child: Option<Child>,
}

/// 第一层：自己 spawn 的进程是否存活
pub fn own_alive(state: &mut DshState) -> bool {
    match state.child.as_mut() {
        Some(c) => c.try_wait().ok().flatten().is_none(),
        None => false,
    }
}

/// 第二层：3080 端口是否可连接（服务是否在）
pub fn port_open() -> bool {
    let addr: SocketAddr = format!("127.0.0.1:{}", DSH_PORT).parse().unwrap();
    TcpStream::connect_timeout(&addr, Duration::from_millis(600)).is_ok()
}

/// 第三层：netstat 反查 3080 监听 PID（识别外部启动的 dsh）
pub fn port_pid() -> Option<u32> {
    let out = Command::new("netstat").arg("-ano").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if line.contains(&format!(":{}", DSH_PORT)) && line.contains("LISTENING") {
            if let Some(pid) = line.split_whitespace().last() {
                if let Ok(p) = pid.parse::<u32>() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// 聚合运行状态
pub fn running(state: &mut DshState) -> bool {
    own_alive(state) || port_open()
}

/// 隐藏窗口启动 dsh web（无 cmd 窗口、无泄漏窗口可误关）
pub fn start(state: &mut DshState, node: &str, bin_js: &str) -> Result<(), String> {
    if port_open() {
        return Err("端口 3080 已在使用（dsh 可能已在运行，请查看控制台状态）".into());
    }
    if own_alive(state) {
        return Err("dsh 进程已在运行".into());
    }

    // 用 powershell -WindowStyle Hidden 包一层：
    // 它创建一个【隐藏控制台】，node 与其所有子进程(npm/pnpm...)继承同一隐藏控制台
    // ，而不是各自新建可见控制台(黑窗)。--no-open 禁止自动弹浏览器。
    let mut cmd = Command::new("powershell.exe");
    cmd.arg("-NoProfile")
        .arg("-WindowStyle")
        .arg("Hidden")
        .arg("-Command")
        .arg(format!(
            "& '{}' '{}' web --no-open",
            node.replace("'", "''"),
            bin_js.replace("'", "''")
        ));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let child = cmd.spawn().map_err(|e| format!("启动 dsh 失败: {}", e))?;
    state.child = Some(child);
    Ok(())
}

/// 停止：本工具启动的直接 kill；否则返回错误信息（由上层提示用户确认后按 PID 处理）
pub fn stop(state: &mut DshState) -> Result<(), String> {
    if let Some(mut c) = state.child.take() {
        let _ = c.kill();
        let _ = c.wait();
        return Ok(());
    }
    // 外部进程：按 PID 结束（调用方需已确认）
    if let Some(pid) = port_pid() {
        let ok = crate::winutil::exe_hidden(
            "taskkill",
            &["/PID", &pid.to_string(), "/T", "/F"],
        )
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
        if ok {
            return Ok(());
        }
        return Err(format!("无法结束外部进程 PID {}", pid));
    }
    Err("没有正在运行的 dsh 进程".into())
}