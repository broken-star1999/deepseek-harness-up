// 版本检查（npm registry，npmmirror 优先 + 官方兜底）与 npm 更新
use std::process::Command;
use std::time::Duration;

const REGISTRIES: [&str; 2] = [
    "https://registry.npmmirror.com/@deepseek-ai/dsh/latest",
    "https://registry.npmjs.org/@deepseek-ai/dsh/latest",
];

/// 查询 registry 最新版本；全部失败返回 Err（上层静默降级为离线）
pub fn registry_latest() -> Result<String, String> {
    for url in REGISTRIES {
        match ureq::get(url).timeout(Duration::from_secs(4)).call() {
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

/// 执行 npm 全局更新（走用户已配置的 registry，如 npmmirror）
pub fn update_dsh() -> Result<String, String> {
    let out = Command::new("cmd")
        .args(["/C", "npm", "install", "-g", "@deepseek-ai/dsh@latest"])
        .output()
        .map_err(|e| format!("无法执行 npm: {}", e))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.chars().take(300).collect::<String>();
        return Err(format!("npm 更新失败: {}", err.trim()));
    }
    let out = String::from_utf8_lossy(&out.stdout);
    Ok(out.chars().take(500).collect())
}

/// 安装 dsh（体检引导用）
pub fn install_dsh() -> Result<String, String> {
    let out = Command::new("cmd")
        .args(["/C", "npm", "install", "-g", "@deepseek-ai/dsh"])
        .output()
        .map_err(|e| format!("无法执行 npm: {}", e))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.chars().take(300).collect::<String>();
        return Err(format!("npm 安装失败: {}", err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).chars().take(300).collect())
}
