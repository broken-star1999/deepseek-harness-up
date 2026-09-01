// 卸载：全局包 + 可选清除用户配置(~/.dsh) 与 npx 缓存
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallTarget {
    pub path: String,
    pub exists: bool,
    pub is_symlink: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallPreview {
    pub dsh_home: Option<UninstallTarget>,
    pub npx_cache: Option<UninstallTarget>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallReport {
    pub ok: bool,
    pub message: String,
    pub package_removed: bool,
    pub config_requested: bool,
    pub config_removed: bool,
    pub npx_requested: bool,
    pub npx_removed: bool,
}

/// dsh 配置根：$DSH_HOME 优先，否则 ~/.dsh。
///
/// 正常运行不会读取 dsh 内部内容；只有用户明确选择完整卸载时，调用方才会
/// 使用该路径删除整棵用户数据目录。任何无法确认的路径都会 fail closed。
pub fn dsh_home() -> Option<PathBuf> {
    let user = std::env::var("USERPROFILE").ok()?;
    if user.trim().is_empty() || !Path::new(&user).is_absolute() {
        return None;
    }
    let default = PathBuf::from(&user).join(".dsh");
    if !dsh_home_safe(&default.to_string_lossy(), &user) {
        return None;
    }
    if let Ok(h) = std::env::var("DSH_HOME") {
        let t = h.trim();
        if t.is_empty() {
            return Some(default);
        }
        // DSH_HOME 已明确设置但不安全时不能悄悄回退到默认目录。
        return dsh_home_safe(t, &user).then(|| PathBuf::from(t));
    }
    Some(default)
}

/// DSH_HOME 安全校验：绝对路径 + 非 UNC + 非盘符根 + 非用户主目录及其祖先。
fn dsh_home_safe(t: &str, user: &str) -> bool {
    if t.chars().any(|c| c.is_control()) || user.chars().any(|c| c.is_control()) {
        return false;
    }
    if t.replace('\\', "/")
        .split('/')
        .any(|segment| segment == "..")
    {
        return false;
    }
    let norm = normalize_windows_path(t);
    if norm.is_empty() {
        return false;
    }
    // 绝对路径检查：C:/foo（盘符后必须是分隔符，拒绝 drive-relative C:foo）。
    let drive_ok = norm.len() >= 3 && norm.as_bytes()[1] == b':' && norm.as_bytes()[2] == b'/';
    if !drive_ok {
        return false;
    }
    // UNC 及盘符根目录。
    if norm.starts_with("//") || norm.starts_with(r"\\") {
        return false;
    }
    if norm.len() <= 3 || norm[3..].contains(':') {
        return false;
    }
    // 用户目录本身及其祖先（大小写无关）。
    let user_norm = normalize_windows_path(user);
    let user_low = user_norm.to_lowercase();
    let norm_low = norm.to_lowercase();
    if norm_low == user_low || user_low.starts_with(&(norm_low + "/")) {
        return false;
    }
    true
}

/// 纯字符串规范化：正斜杠统一 + 去 . 和 ..（不触文件系统）。
fn normalize_windows_path(p: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut is_abs = false;
    for seg in p.replace('\\', "/").split('/') {
        match seg {
            "" => {
                if parts.is_empty() && !is_abs {
                    is_abs = true;
                }
            }
            "." => {}
            ".." => {
                if parts.len() > 1 {
                    parts.pop();
                }
            }
            other => parts.push(other.to_string()),
        }
    }
    let mut out = parts.join("/");
    if let Some(first) = parts.first() {
        if first.len() < 2 || first.as_bytes()[1] != b':' {
            out = (if is_abs { "/" } else { "" }).to_string() + &out;
        }
    }
    out
}

/// npx 缓存目录。
pub fn npx_cache() -> Option<PathBuf> {
    crate::paths::npx_cache_path().ok()
}

fn target(path: Option<PathBuf>) -> Option<UninstallTarget> {
    let path = path?;
    let metadata = std::fs::symlink_metadata(&path).ok();
    Some(UninstallTarget {
        path: path.to_string_lossy().into_owned(),
        exists: metadata.is_some(),
        is_symlink: metadata
            .as_ref()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
    })
}

pub fn preview() -> UninstallPreview {
    UninstallPreview {
        dsh_home: target(dsh_home()),
        npx_cache: target(npx_cache()),
    }
}

fn preflight_delete(path: &Path, label: &str) -> Result<(), String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("读取 {} 失败: {}", path.display(), e)),
    };
    if meta.file_type().is_symlink() {
        return Err(format!(
            "{} 是链接，为安全起见拒绝删除: {}",
            label,
            path.display()
        ));
    }
    if !meta.is_dir() {
        return Err(format!(
            "{} 不是目录，为安全起见拒绝删除: {}",
            label,
            path.display()
        ));
    }
    Ok(())
}

fn delete_optional(path: &Path, label: &str) -> Result<bool, String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(format!("读取 {} 失败 ({}): {}", label, path.display(), e)),
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(format!(
            "{} 不是可安全删除的普通目录: {}",
            label,
            path.display()
        ));
    }
    std::fs::remove_dir_all(path)
        .map_err(|e| format!("删除 {} 失败 ({}): {}", label, path.display(), e))?;
    Ok(!path.exists())
}

/// 执行卸载。
///
/// 包卸载失败时不会继续删除用户数据；包已经卸载但数据删除失败时，返回
/// `ok=false` 的结构化部分完成结果，让前端可以准确提示残留。
pub fn run(clear_config: bool, clear_npx: bool) -> Result<UninstallReport, String> {
    let home = if clear_config {
        Some(dsh_home().ok_or("无法确定用户配置目录，已拒绝删除")?)
    } else {
        None
    };
    let npx = if clear_npx {
        Some(npx_cache().ok_or("无法确定 npm 缓存目录，已拒绝删除")?)
    } else {
        None
    };

    // 先预检查所有删除目标，避免 npm 卸载后才发现用户选择的删除范围不安全。
    if let Some(path) = home.as_deref() {
        preflight_delete(path, "用户配置目录")?;
    }
    if let Some(path) = npx.as_deref() {
        preflight_delete(path, "npx 缓存目录")?;
    }

    let mut message = String::new();
    crate::updater::uninstall_dsh()?;
    message.push_str("✅ 全局包 @deepseek-ai/dsh 已卸载\n");

    let mut errors = Vec::new();
    let config_removed = if let Some(path) = home.as_deref() {
        match delete_optional(path, "用户配置目录") {
            Ok(true) => {
                message.push_str(&format!("✅ dsh 用户数据已清除: {}\n", path.display()));
                true
            }
            Ok(false) => {
                errors.push(format!("用户配置目录仍然存在: {}", path.display()));
                false
            }
            Err(e) => {
                errors.push(e);
                false
            }
        }
    } else {
        false
    };

    let npx_removed = if let Some(path) = npx.as_deref() {
        match delete_optional(path, "npx 缓存目录") {
            Ok(true) => {
                message.push_str(&format!("✅ npx 缓存已清除: {}\n", path.display()));
                true
            }
            Ok(false) => {
                errors.push(format!("npx 缓存目录仍然存在: {}", path.display()));
                false
            }
            Err(e) => {
                errors.push(e);
                false
            }
        }
    } else {
        false
    };

    let ok = errors.is_empty();
    if !ok {
        message.push_str("⚠ 卸载已部分完成：\n");
        for error in &errors {
            message.push_str("- ");
            message.push_str(error);
            message.push('\n');
        }
    }

    Ok(UninstallReport {
        ok,
        message,
        package_removed: true,
        config_requested: clear_config,
        config_removed,
        npx_requested: clear_npx,
        npx_removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsh_home_safe_defends_dangerous_paths() {
        let user = r"C:\Users\u";
        assert!(!dsh_home_safe(r"C:\", user));
        assert!(!dsh_home_safe(user, user));
        assert!(!dsh_home_safe(r"C:\Users", user));
        assert!(!dsh_home_safe(r"C:\Users\u\..", user));
        assert!(!dsh_home_safe(r"\\server\share", user));
        assert!(!dsh_home_safe(r"C:relative", user));
        assert!(dsh_home_safe(r"C:\custom\dsh-conf", user));
        assert!(dsh_home_safe(r"c:/custom/dsh-conf", user));
    }

    #[test]
    fn normalize_windows_path_removes_dot_segments_without_fs_access() {
        assert_eq!(normalize_windows_path(r"C:\custom\.\dsh"), "C:/custom/dsh");
        assert_eq!(
            normalize_windows_path(r"C:\custom\dsh\..\conf"),
            "C:/custom/conf"
        );
    }

    #[test]
    fn isolated_regular_directory_is_deleted_completely() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "dsh-up-uninstaller-test-{}-{}",
            std::process::id(),
            nonce
        ));
        let nested = root.join("profiles").join("web");
        std::fs::create_dir_all(&nested).expect("test directory should be writable");
        std::fs::write(nested.join("session.json"), b"test").expect("test file should be writable");

        assert!(preflight_delete(&root, "test directory").is_ok());
        assert!(delete_optional(&root, "test directory").expect("test directory should delete"));
        assert!(!root.exists());
    }
}
