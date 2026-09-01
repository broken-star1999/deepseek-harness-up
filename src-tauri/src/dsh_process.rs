// dsh 进程生命周期：隐藏 spawn / 三层状态检测 / 停止
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

pub const DSH_PORT: u16 = 3080;
const BOOT_TIMEOUT: Duration = Duration::from_secs(40);

/// 核心运行日志路径
pub fn core_log_path() -> Result<std::path::PathBuf, String> {
    crate::paths::core_log_path()
}

#[derive(Default)]
pub struct DshState {
    pub child: Option<Child>,
    /// 已 spawn 但 3080 尚未就绪的启动阶段。
    pub booting: bool,
    pub started_at: Option<Instant>,
}

/// 读取自己 spawn 的进程状态；发现进程已退出时顺手清理句柄和阶段。
pub fn own_alive(state: &mut DshState) -> bool {
    match state.child.as_mut() {
        Some(c) => match c.try_wait() {
            Ok(Some(_)) => {
                state.child = None;
                state.booting = false;
                state.started_at = None;
                false
            }
            Ok(None) => true,
            // 无法确认时按仍存活处理，避免更新/卸载与未知进程竞争。
            Err(_) => true,
        },
        None => false,
    }
}

fn kill_owned_child(state: &mut DshState) {
    if let Some(mut child) = state.child.take() {
        // 自己启动的 dsh 也结束整棵子进程树，避免子进程继续占用 3080。
        #[cfg(windows)]
        {
            let pid = child.id().to_string();
            let _ = crate::winutil::exe_hidden("taskkill", &["/PID", &pid, "/T", "/F"]).output();
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    state.booting = false;
    state.started_at = None;
}

/// 刷新自己启动的 dsh 生命周期。
///
/// 启动超时会清理自己持有的进程树；这只作用于本工具自己 spawn 的 child，
/// 不会对外部 PID 做猜测性终止。
pub fn refresh(state: &mut DshState) {
    if !own_alive(state) {
        return;
    }
    if !state.booting {
        return;
    }
    if port_open() {
        state.booting = false;
        state.started_at = None;
    } else if state
        .started_at
        .map(|t| t.elapsed() >= BOOT_TIMEOUT)
        .unwrap_or(false)
    {
        kill_owned_child(state);
    }
}

/// 返回自己启动的 dsh 是否仍在启动阶段。
pub fn booting(state: &mut DshState) -> bool {
    refresh(state);
    state.booting && own_alive(state)
}

/// 第一层/第二层/第三层聚合运行状态。
/// 启动阶段只有 booting=true，服务端口就绪后才返回 running=true。
pub fn running(state: &mut DshState) -> bool {
    refresh(state);
    if own_alive(state) {
        return !state.booting;
    }
    port_pid().map(is_dsh_pid).unwrap_or(false)
}

/// 第二层：3080 端口是否可连接（服务是否在）。
pub fn port_open() -> bool {
    let addr: SocketAddr = format!("127.0.0.1:{}", DSH_PORT).parse().unwrap();
    TcpStream::connect_timeout(&addr, Duration::from_millis(600)).is_ok()
}

/// 等待 3080 释放，给 Windows 进程树和 socket 关闭留出短暂时间。
pub fn wait_port_closed(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while port_open() {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    true
}

/// 第三层：netstat 反查 3080 监听 PID（识别外部启动的 dsh）。
pub fn port_pid() -> Option<u32> {
    let out = crate::winutil::exe_hidden("netstat", &["-ano"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    parse_netstat_pid(&text)
}

fn parse_netstat_pid(text: &str) -> Option<u32> {
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        let exact_port = fields[1]
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse::<u16>().ok())
            == Some(DSH_PORT);
        if exact_port && fields.contains(&"LISTENING") {
            if let Some(pid) = fields.last().and_then(|value| value.parse::<u32>().ok()) {
                return Some(pid);
            }
        }
    }
    None
}

/// 隐藏窗口启动 dsh web（无 cmd 窗口、无泄漏窗口可误关）。
pub fn start(state: &mut DshState, node: &str, bin_js: &str) -> Result<(), String> {
    refresh(state);
    if port_open() {
        return Err("端口 3080 已在使用（dsh 可能已在运行，请查看控制台状态）".into());
    }
    if own_alive(state) {
        return Err("dsh 进程已在运行".into());
    }

    // 【DSH-Launcher 移植】直接 spawn node(无 powershell 中转):
    // - CREATE_NO_WINDOW(与社区方案一致)
    // - --profile web --no-open(等价 web 子命令, 官方推荐形式)
    // - 无 powershell: 中转层会破坏隐藏链
    let args = [bin_js, "--profile", "web", "--no-open"];
    let mut cmd = crate::winutil::exe_hidden(node, &args);
    // 核心输出重定向到日志（boot 过程/子命令全记录，可分析黑窗与慢启动来源）。
    let log_path = core_log_path()?;
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
    state.booting = true;
    state.started_at = Some(Instant::now());
    Ok(())
}

fn normalize_command_line(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace('\\', "/")
        .to_ascii_lowercase()
}

/// 解析 Windows 常见命令行引号规则，足以区分路径参数和普通文本参数。
fn split_windows_command_line(value: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut backslashes = 0usize;

    let flush_backslashes = |current: &mut String, count: &mut usize| {
        current.extend(std::iter::repeat_n('\\', *count));
        *count = 0;
    };

    for ch in value.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                current.extend(std::iter::repeat_n('\\', backslashes / 2));
                if backslashes % 2 == 1 {
                    current.push('"');
                } else {
                    in_quotes = !in_quotes;
                }
                backslashes = 0;
            }
            c if c.is_whitespace() && !in_quotes => {
                flush_backslashes(&mut current, &mut backslashes);
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            other => {
                flush_backslashes(&mut current, &mut backslashes);
                current.push(other);
            }
        }
    }
    flush_backslashes(&mut current, &mut backslashes);
    if !current.is_empty() {
        args.push(current);
    }
    args
}

/// 纯函数身份校验：第二个命令行参数必须是已定位的 dsh 入口绝对路径。
fn command_line_matches_dsh(command_line: &str, bin_js: &std::path::Path) -> bool {
    let args = split_windows_command_line(command_line);
    let Some(actual) = args.get(1) else {
        return false;
    };
    let expected = normalize_command_line(&bin_js.to_string_lossy());
    !expected.is_empty() && normalize_command_line(actual) == expected
}

/// 只读校验：PID 是否为 dsh 进程（executable + 精确入口参数，非侵入）。
pub fn is_dsh_pid(pid: u32) -> bool {
    let script = format!(
        r#"(Get-CimInstance Win32_Process -Filter "ProcessId={}" | Select-Object ExecutablePath,CommandLine | ConvertTo-Json -Compress)"#,
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
    let Ok(info) = serde_json::from_slice::<serde_json::Value>(&o.stdout) else {
        return false;
    };
    let Some(bin_js) = crate::dsh_locator::locate().bin_js else {
        return false;
    };
    let Some(expected_node) = crate::dsh_locator::locate().node else {
        return false;
    };
    let executable = info
        .get("ExecutablePath")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let command_line = info
        .get("CommandLine")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    normalize_command_line(executable) == normalize_command_line(&expected_node)
        && command_line_matches_dsh(command_line, &bin_js)
}

/// 停止：本工具启动的直接 kill；否则只对确认身份的外部 dsh 按 PID 处理。
pub fn stop(state: &mut DshState) -> Result<(), String> {
    refresh(state);
    if own_alive(state) {
        kill_owned_child(state);
        return Ok(());
    }

    // 清掉已经退出的句柄，然后继续检查是否存在外部 dsh。
    let _ = state.child.take();
    state.booting = false;
    state.started_at = None;

    // 外部进程：先只读校验确实是 dsh（命令行特征），再按 PID 结束。
    if let Some(pid) = port_pid() {
        if !is_dsh_pid(pid) {
            return Err(format!(
                "端口 3080 被非 dsh 进程占用（PID {}），为安全起见不自动结束。请手动处理。",
                pid
            ));
        }
        let pid_arg = pid.to_string();
        let ok = crate::winutil::exe_hidden("taskkill", &["/PID", &pid_arg, "/T", "/F"])
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
    fn command_line_identity_requires_exact_entry_argument() {
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
            r#"node.exe --label C:\Users\u\AppData\Roaming\npm\node_modules\@deepseek-ai\dsh\lib\bin.js"#,
            expected
        ));
        assert!(command_line_matches_dsh(
            r#""C:\Program Files\nodejs\node.exe" "C:\Users\u\AppData\Roaming\npm\node_modules\@deepseek-ai\dsh\lib\bin.js" --profile web"#,
            expected
        ));
    }

    #[test]
    fn split_windows_command_line_preserves_quoted_paths() {
        let args = split_windows_command_line(
            r#""C:\Program Files\nodejs\node.exe" "C:\Program Files\dsh\lib\bin.js" --profile web"#,
        );
        assert_eq!(args[0], r"C:\Program Files\nodejs\node.exe");
        assert_eq!(args[1], r"C:\Program Files\dsh\lib\bin.js");
    }

    #[test]
    fn netstat_parser_requires_exact_listening_port() {
        let text = concat!(
            "  TCP    127.0.0.1:30800    0.0.0.0:0    LISTENING    11\n",
            "  TCP    127.0.0.1:3080     0.0.0.0:0    TIME_WAIT    12\n",
            "  TCP    [::]:3080         [::]:0         LISTENING    42\n",
        );
        assert_eq!(parse_netstat_pid(text), Some(42));
    }
}
