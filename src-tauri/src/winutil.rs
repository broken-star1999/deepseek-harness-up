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
