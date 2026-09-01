use std::path::Path;

// Windows 进程工具：统一隐藏控制台窗口（防 cmd 闪烁）
#[allow(unused)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 创建隐藏窗口的 cmd 命令（所有 cmd /C 调用必须走这里，否则闪烁）
#[allow(unused)]
pub fn cmd_hidden(args: &[&str]) -> std::process::Command {
    let mut c = std::process::Command::new("cmd");
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// 隐藏窗口运行一个可执行文件（node/netstat 等 exe 直接跑无窗口，但保险统一）
#[allow(unused)]
pub fn exe_hidden(prog: &str, args: &[&str]) -> std::process::Command {
    let mut c = std::process::Command::new(prog);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// 通过隐藏 cmd 调用批处理文件。
///
/// Windows 的 CreateProcess 不能直接执行 .cmd，因此 npm.cmd 仍需要 cmd /C。
/// 与直接把参数拼到 `/C` 后面不同，这里会把程序路径和每个参数逐项引用，
/// 避免参数中的空格、`&`、`|` 等字符被当成新的 shell 语句。调用方仍应对
/// 外部输入做语义校验；该函数负责命令行层的参数边界。
pub fn batch_hidden(program: &Path, args: &[&str]) -> Result<std::process::Command, String> {
    validate_batch_arg(&program.to_string_lossy())?;
    for arg in args {
        validate_batch_arg(arg)?;
    }
    let mut inner = quote_windows_arg(&program.to_string_lossy());
    for arg in args {
        inner.push(' ');
        inner.push_str(&quote_windows_arg(arg));
    }
    // /S /C 会剥掉命令文本最外层的一对引号；再加一层才能保留程序
    // 路径和各参数自己的引号（典型形式：""C:\\Program Files\\npm.cmd" arg"）。
    let command_line = format!("\"{}\"", inner);
    let mut cmd = cmd_hidden(&["/D", "/S", "/C"]);
    // cmd.exe 的 /C 参数本身是一段命令文本，不能让 Rust 的普通 argv
    // 转义把内部双引号变成字面量反斜杠；Windows 上使用 raw_arg 保留
    // 经过本函数构造的命令文本。所有调用方仍须传入已校验的参数。
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(&command_line);
    }
    #[cfg(not(windows))]
    {
        cmd.arg(command_line);
    }
    Ok(cmd)
}

/// 批处理参数的最后一道 fail-closed 检查。
///
/// npm.cmd 会在批处理脚本内部展开 `%*`，因此 shell 元字符即使在调用端
/// 被引号包住，也不能保证在批处理展开后仍是原子参数。此处直接拒绝它们；
/// 业务层的 registry 校验还会提供 URL 语义校验。
fn validate_batch_arg(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Ok(());
    }
    if value.chars().any(|c| {
        c.is_control() || matches!(c, '&' | '|' | '<' | '>' | '^' | '%' | '!' | '"' | '\'')
    }) {
        return Err("批处理参数包含不安全的 shell 字符".into());
    }
    Ok(())
}

/// Windows 命令行参数引用（兼容包含空格和反斜杠的路径）。
fn quote_windows_arg(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for ch in value.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            other => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(other);
                backslashes = 0;
            }
        }
    }
    // 引号结尾前的反斜杠需要翻倍，否则会转义结束引号。
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_command_quotes_shell_metacharacters_as_one_argument() {
        let cmd = batch_hidden(
            Path::new(r"C:\Program Files\nodejs\npm.cmd"),
            &["--registry=https://registry.example.com/a&b", "plain"],
        )
        .unwrap_err();
        let debug = format!("{:?}", cmd);
        assert!(debug.contains("不安全") || debug.contains("shell"));
    }

    #[test]
    fn windows_arg_quotes_backslashes_before_quote() {
        assert_eq!(quote_windows_arg(r#"C:\path\"#), r#""C:\path\\""#);
    }

    #[cfg(windows)]
    #[test]
    fn batch_runner_executes_safe_argument_without_injection() {
        let script =
            std::env::temp_dir().join(format!("dsh-up-batch-test-{}.cmd", std::process::id()));
        std::fs::write(&script, b"@echo off\r\necho ARG=%~1\r\n")
            .expect("test batch file should be writable");
        let output = batch_hidden(Path::new(&script), &["SAFE value"])
            .expect("safe batch arguments should be accepted")
            .output()
            .expect("test batch file should start");
        let _ = std::fs::remove_file(&script);
        assert!(
            output.status.success(),
            "status={:?} stdout={:?} stderr={:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("ARG=SAFE value"), "stdout={text:?}");
        assert!(!text.lines().any(|line| line.trim() == "INJECTED"));
    }
}
