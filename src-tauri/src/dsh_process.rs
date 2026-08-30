// dsh 进程生命周期：隐藏 spawn / 三层状态检测 / 停止
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Stdio};
use std::time::Duration;

pub const DSH_PORT: u16 = 3080;

/// 核心运行日志路径
pub fn core_log_path() -> std::path::PathBuf {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    std::path::PathBuf::from(la).join("dsh-up").join("core.log")
}
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
    let out = crate::winutil::exe_hidden("netstat", &["-ano"])
        .output()
        .ok()?;
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

/// 聚合运行状态：只把自己的进程或已确认的 dsh 端口占用者视为运行。
pub fn running(state: &mut DshState) -> bool {
    if own_alive(state) {
        return true;
    }
    port_pid().map(is_dsh_pid).unwrap_or(false)
}

/// 隐藏窗口启动 dsh web（无 cmd 窗口、无泄漏窗口可误关）
pub fn start(state: &mut DshState, node: &str, bin_js: &str) -> Result<(), String> {
    if port_open() {
        return Err("端口 3080 已在使用（dsh 可能已在运行，请查看控制台状态）".into());
    }
    if own_alive(state) {
        return Err("dsh 进程已在运行".into());
    }

    // 【DSH-Launcher 移植】直接 spawn node(无 powershell 中转):
    // - windowsHide(true) = CREATE_NO_WINDOW(与社区 148-star 验证方案一致)
    // - --profile web --no-open(等价 web 子命令, 官方推荐形式)
    // - 无 powershell: 实测中转层会破坏隐藏链(13次黑窗元凶)
    let args = [bin_js, "--profile", "web", "--no-open"];
    let mut cmd = crate::winutil::exe_hidden(node, &args);
    // 核心输出重定向到日志（boot 过程/子命令全记录，可分析黑窗与慢启动来源）
    let log_path = core_log_path();
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(f) = std::fs::File::create(&log_path) {
        let stdout = f
            .try_clone()
            .map_err(|e| format!("复制核心日志句柄失败: {}", e))?;
        cmd.stdout(stdout).stderr(f).stdin(Stdio::null());
    } else {
        cmd.stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());
    }
    let child = cmd.spawn().map_err(|e| format!("启动 dsh 失败: {}", e))?;
    state.child = Some(child);
    Ok(())
}

fn normalize_command_line(value: &str) -> String {
    value.replace('\\', "/").to_ascii_lowercase()
}

/// 纯函数身份校验：命令行必须包含已定位的 dsh 入口绝对路径。
fn command_line_matches_dsh(command_line: &str, bin_js: &std::path::Path) -> bool {
    let command = normalize_command_line(command_line);
    let expected = normalize_command_line(&bin_js.to_string_lossy());
    !expected.is_empty() && command.contains(&expected)
}

/// 只读校验：PID 是否为 dsh 进程（匹配已定位入口，非侵入——只看不写）
pub fn is_dsh_pid(pid: u32) -> bool {
    let script = format!(
        r#"(Get-CimInstance Win32_Process -Filter "ProcessId={}").CommandLine"#,
        pid
    );
    let out = crate::winutil::cmd_hidden(&[
        "/C",
        "powershell",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        &script,
    ])
    .output()
    .ok();
    let Some(o) = out else { return false };
    let Some(bin_js) = crate::dsh_locator::locate().bin_js else {
        return false;
    };
    command_line_matches_dsh(&String::from_utf8_lossy(&o.stdout), &bin_js)
}

/// 停止：本工具启动的直接 kill；否则返回错误信息（由上层提示用户确认后按 PID 处理）
pub fn stop(state: &mut DshState) -> Result<(), String> {
    // 只有确认子进程仍存活时才优先处理它；已退出的句柄不能遮蔽外部 dsh。
    if own_alive(state) {
        if let Some(mut c) = state.child.take() {
            // 自己启动的 dsh 也结束整棵子进程树，避免子进程继续占用 3080。
            #[cfg(windows)]
            {
                let pid = c.id().to_string();
                let _ =
                    crate::winutil::exe_hidden("taskkill", &["/PID", &pid, "/T", "/F"]).output();
            }
            let _ = c.kill();
            let _ = c.wait();
            return Ok(());
        }
    } else {
        // 清掉已经退出的句柄，然后继续检查是否存在外部 dsh。
        let _ = state.child.take();
    }
    // 外部进程：先只读校验确实是 dsh（命令行特征），再按 PID 结束——防误杀无关程序
    if let Some(pid) = port_pid() {
        if !is_dsh_pid(pid) {
            return Err(format!(
                "端口 3080 被非 dsh 进程占用（PID {}），为安全起见不自动结束。请手动处理。",
                pid
            ));
        }
        let ok = crate::winutil::exe_hidden("taskkill", &["/PID", &pid.to_string(), "/T", "/F"])
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn command_line_identity_requires_exact_entry_path() {
        let expected =
            Path::new(r"C:\Users\u\AppData\Roaming\npm\node_modules\@deepseek-ai\dsh\lib\bin.js");
        assert!(command_line_matches_dsh(
            r#"node.exe C:\Users\u\AppData\Roaming\npm\node_modules\@deepseek-ai\dsh\lib\bin.js --profile web"#,
            expected
        ));
        assert!(!command_line_matches_dsh(
            r#"node.exe C:\other\node_modules\bin.js --profile web"#,
            expected
        ));
        assert!(!command_line_matches_dsh(
            r#"node.exe --label @deepseek-ai"#,
            expected
        ));
    }
}
