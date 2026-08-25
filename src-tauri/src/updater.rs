// 版本检查（npm registry，npmmirror 优先 + 官方兜底）与 npm 更新
use std::time::Duration;

/// 安装/更新日志路径（前端进度反馈）
fn install_log_path() -> std::path::PathBuf {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    std::path::PathBuf::from(la).join("dsh-up").join("install.log")
}

/// 记录 npm 输出到 install.log + 返回尾部文本
fn write_install_log(out: &std::process::Output) {
    let mut s = String::new();
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    if let Some(d) = install_log_path().parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(install_log_path(), s);
}

/// 读取设置(镜像等)
fn setting(key: &str) -> Option<String> {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let p = std::path::PathBuf::from(la)
        .join("dsh-up")
        .join("settings.json");
    let text = std::fs::read_to_string(p).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get(key)?.as_str().map(|s| s.to_string())
}

/// 注册表选择（settings["mirror"]: npmmirror / npmjs / custom:<url>）
fn registry_chain() -> Vec<String> {
    match setting("mirror").as_deref() {
        Some("npmjs") => vec![
            "https://registry.npmjs.org/@deepseek-ai/dsh/latest".to_string(),
        ],
        Some(m) if m.starts_with("custom:") => {
            let url = m.trim_start_matches("custom:");
            let mut v = vec![format!("{}/@deepseek-ai/dsh/latest", url.trim_end_matches('/'))];
            v.push("https://registry.npmmirror.com/@deepseek-ai/dsh/latest".to_string());
            v
        }
        _ => vec![
            "https://registry.npmmirror.com/@deepseek-ai/dsh/latest".to_string(),
            "https://registry.npmjs.org/@deepseek-ai/dsh/latest".to_string(),
        ],
    }
}

/// 构造 npm `--registry` 参数（与设置镜像一致：npmjs/npmmirror/custom:url，custom 去尾斜杠）
pub fn mirror_registry_arg(mirror: Option<&str>) -> String {
    match mirror {
        Some("npmjs") => "--registry=https://registry.npmjs.org".to_string(),
        Some(m) if m.starts_with("custom:") => {
            format!("--registry={}", m.trim_start_matches("custom:").trim_end_matches('/'))
        }
        _ => "--registry=https://registry.npmmirror.com".to_string(),
    }
}

/// 查询 registry 最新版本；全部失败返回 Err（上层静默降级为离线）
pub fn registry_latest() -> Result<String, String> {
    for url in registry_chain() {
        match ureq::get(url.as_str()).timeout(Duration::from_secs(4)).call() {
            Ok(resp) => {
                if let Ok(body) = resp.into_string() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(vs) = v.get("version").and_then(|x| x.as_str()) {
                            return Ok(vs.to_string());
                        }
                    }
                }
            }
            Err(_) => continue,
        }
    }
    Err("无法连接 npm 注册表".into())
}

/// semver 比较（prerelease 低于正式版，如 0.1.1-rc.2 < 0.1.1）
pub fn is_outdated(local: &str, latest: &str) -> bool {
    match (semver::Version::parse(local), semver::Version::parse(latest)) {
        (Ok(l), Ok(r)) => l < r,
        _ => local != latest,
    }
}

/// 执行 npm 全局更新（走镜像设置一致的 registry，--registry 参数）
pub fn update_dsh() -> Result<String, String> {
    let mirror_arg = mirror_registry_arg(setting("mirror").as_deref());
    let out = crate::winutil::cmd_hidden(&[
        "/C",
        "npm",
        "install",
        "-g",
        "@deepseek-ai/dsh@latest",
        &mirror_arg,
    ])
        .output()
        .map_err(|e| format!("无法执行 npm: {}", e))?;
    write_install_log(&out);
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.chars().take(300).collect::<String>();
        return Err(format!("npm 更新失败: {}", err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).chars().take(500).collect())
}

/// 安装 dsh（体检引导用）
pub fn install_dsh() -> Result<String, String> {
    let mirror_arg = mirror_registry_arg(setting("mirror").as_deref());
    let out = crate::winutil::cmd_hidden(&[
        "/C",
        "npm",
        "install",
        "-g",
        "@deepseek-ai/dsh",
        &mirror_arg,
    ])
        .output()
        .map_err(|e| format!("无法执行 npm: {}", e))?;
    write_install_log(&out);
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.chars().take(300).collect::<String>();
        return Err(format!("npm 安装失败: {}", err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).chars().take(300).collect())
}