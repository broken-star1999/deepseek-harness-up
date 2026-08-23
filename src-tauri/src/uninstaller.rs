// 卸载：全局包 + 可选清除用户配置(~/.dsh) 与 npx 缓存
use std::path::PathBuf;

/// dsh 配置根：$DSH_HOME 优先，否则 ~/.dsh
pub fn dsh_home() -> PathBuf {
    if let Ok(h) = std::env::var("DSH_HOME") {
        if !h.trim().is_empty() {
            return PathBuf::from(h.trim());
        }
    }
    let user = std::env::var("USERPROFILE").unwrap_or_default();
    PathBuf::from(user).join(".dsh")
}

/// npx 缓存目录
pub fn npx_cache() -> PathBuf {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(la).join("npm-cache").join("_npx")
}

pub fn run(clear_config: bool, clear_npx: bool) -> Result<String, String> {
    let mut log = String::new();

    // 1. 卸载全局包（npm 自动清理 shim）
    let mirror_arg = {
        let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let p = std::path::PathBuf::from(la).join("dsh-up").join("settings.json");
        std::fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("mirror").and_then(|x| x.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| "npmmirror".to_string())
    };
    let registry_arg = match mirror_arg.as_str() {
        "npmjs" => "--registry=https://registry.npmjs.org".to_string(),
        m if m.starts_with("custom:") => {
            format!("--registry={}", m.trim_start_matches("custom:").trim_end_matches('/'))
        }
        _ => "--registry=https://registry.npmmirror.com".to_string(),
    };
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