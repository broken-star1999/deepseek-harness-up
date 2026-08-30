// 卸载：全局包 + 可选清除用户配置(~/.dsh) 与 npx 缓存
use std::path::PathBuf;

/// dsh 配置根：$DSH_HOME 优先，否则 ~/.dsh
/// 防御：拒绝盘符根目录/用户主目录/空路径等危险删除目标
pub fn dsh_home() -> PathBuf {
    let user = std::env::var("USERPROFILE").unwrap_or_default();
    let default = PathBuf::from(&user).join(".dsh");
    if let Ok(h) = std::env::var("DSH_HOME") {
        let t = h.trim();
        if !t.is_empty() && dsh_home_safe(t, &user) {
            return PathBuf::from(t);
        }
    }
    default
}

/// DSH_HOME 安全校验：绝对路径 + 非 UNC + 非盘符根 + 非用户目录及其祖先（纯字符串规范化，无文件系统依赖）
fn dsh_home_safe(t: &str, user: &str) -> bool {
    let norm = normalize_windows_path(t);
    if norm.is_empty() {
        return false;
    }
    // 绝对路径检查
    let drive_ok = norm.len() >= 2 && norm.as_bytes()[1] == b':';
    if !drive_ok {
        return false;
    }
    // UNC
    if norm.starts_with("//") || norm.starts_with("\\\\") {
        return false;
    }
    // 盘符根（c: 单独出现）
    if norm.len() <= 2 {
        return false;
    }
    // 用户目录本身及其祖先（大小写无关）
    let user_norm = normalize_windows_path(user);
    let user_low = user_norm.to_lowercase();
    let norm_low = norm.to_lowercase();
    if norm_low == user_low || user_low.starts_with(&(norm_low + "/")) {
        return false;
    }
    true
}

/// 纯字符串规范化：正斜杠统一 + 去 . 和 ..（不触文件系统）
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
        if first.len() >= 2 && first.as_bytes()[1] == b':' {
            // 保留盘符不加前缀
        } else {
            out = (if is_abs { "/" } else { "" }).to_string() + &out;
        }
    }
    out
}

/// npx 缓存目录
pub fn npx_cache() -> PathBuf {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(la).join("npm-cache").join("_npx")
}

pub fn run(clear_config: bool, clear_npx: bool) -> Result<String, String> {
    let mut log = String::new();

    // 1. 卸载全局包（npm 自动清理 shim）
    let settings_file = {
        let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
        std::path::PathBuf::from(la)
            .join("dsh-up")
            .join("settings.json")
    };
    let registry_arg = crate::updater::mirror_registry_arg(
        std::fs::read_to_string(settings_file)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| {
                v.get("mirror")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            })
            .as_deref(),
    );
    let out = crate::winutil::cmd_hidden(&[
        "/C",
        "npm",
        "uninstall",
        "-g",
        "@deepseek-ai/dsh",
        &registry_arg,
    ])
    .output()
    .map_err(|e| format!("无法执行 npm: {}", e))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.chars().take(300).collect::<String>();
        return Err(format!("卸载全局包失败: {}", err.trim()));
    }
    log.push_str("✅ 全局包 @deepseek-ai/dsh 已卸载\n");

    // 2. 清除用户配置
    if clear_config {
        let home = dsh_home();
        if home.exists() {
            std::fs::remove_dir_all(&home)
                .map_err(|e| format!("删除 {} 失败: {}", home.display(), e))?;
            log.push_str(&format!("✅ 用户配置已清除: {}\n", home.display()));
        } else {
            log.push_str("（用户配置目录不存在，跳过）\n");
        }
    }

    // 3. 清除 npx 缓存
    if clear_npx {
        let npx = npx_cache();
        if npx.exists() {
            std::fs::remove_dir_all(&npx)
                .map_err(|e| format!("删除 {} 失败: {}", npx.display(), e))?;
            log.push_str(&format!("✅ npx 缓存已清除: {}\n", npx.display()));
        } else {
            log.push_str("（npx 缓存不存在，跳过）\n");
        }
    }

    Ok(log)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsh_home_defends_dangerous_paths() {
        // 盘符根目录 → 回退默认
        std::env::set_var("DSH_HOME", "C:\\");
        let h = dsh_home();
        assert!(h.ends_with(".dsh"));
        // 用户主目录 → 回退默认
        std::env::set_var("DSH_HOME", std::env::var("USERPROFILE").unwrap());
        let h2 = dsh_home();
        assert!(h2.ends_with(".dsh"));
        // 合法路径 → 使用
        std::env::set_var("DSH_HOME", "C:\\custom\\dsh-conf");
        let h3 = dsh_home();
        assert_eq!(h3.to_string_lossy(), "C:\\custom\\dsh-conf");
        std::env::remove_var("DSH_HOME");
    }

    #[test]
    fn dsh_home_rejects_relative_unc_and_dotdot() {
        let user = std::env::var("USERPROFILE").unwrap_or_default();
        // 相对路径
        assert!(!dsh_home_safe("relative\\path", &user));
        // UNC
        assert!(!dsh_home_safe("\\\\server\\share", &user));
        // 用户目录祖先（..）
        assert!(!dsh_home_safe(
            &format!(r"{}\..", user.trim_end_matches('\\')),
            &user
        ));
        // 合法绝对路径
        assert!(dsh_home_safe("C:\\custom\\dsh-conf", &user));
    }
}
