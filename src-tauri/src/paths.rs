use std::path::{Component, PathBuf};

/// 返回本工具专用的本地应用数据目录。
///
/// 这里不允许环境变量缺失、为空或退化成相对路径。配置、日志和缓存都必须
/// 落在明确的绝对路径下，避免异常环境下把数据写进当前工作目录。
pub fn app_data_dir() -> Result<PathBuf, String> {
    let raw = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "LOCALAPPDATA 未设置，无法确定应用数据目录".to_string())?;
    let path = PathBuf::from(raw);
    let display = path.to_string_lossy();
    if display.trim().is_empty() {
        return Err("LOCALAPPDATA 为空，无法确定应用数据目录".into());
    }
    if display.chars().any(|c| c.is_control()) {
        return Err("LOCALAPPDATA 包含非法控制字符".into());
    }
    if !path.is_absolute() {
        return Err(format!(
            "LOCALAPPDATA 不是绝对路径，已拒绝使用: {}",
            path.display()
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!(
            "LOCALAPPDATA 包含父目录跳转，已拒绝使用: {}",
            path.display()
        ));
    }
    if path.parent().is_none() || path.file_name().is_none() {
        return Err(format!(
            "LOCALAPPDATA 不是有效目录，已拒绝使用: {}",
            path.display()
        ));
    }
    Ok(path)
}

pub fn dsh_up_dir() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("dsh-up"))
}

pub fn settings_path() -> Result<PathBuf, String> {
    Ok(dsh_up_dir()?.join("settings.json"))
}

pub fn log_path() -> Result<PathBuf, String> {
    Ok(dsh_up_dir()?.join("log.txt"))
}

pub fn install_log_path() -> Result<PathBuf, String> {
    Ok(dsh_up_dir()?.join("install.log"))
}

pub fn core_log_path() -> Result<PathBuf, String> {
    Ok(dsh_up_dir()?.join("core.log"))
}

pub fn npx_cache_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("npm-cache").join("_npx"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_dir_is_absolute_on_current_platform() {
        assert!(app_data_dir().is_ok());
    }
}
