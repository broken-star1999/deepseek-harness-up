// 版本检查（npm registry，npmmirror 优先 + 官方兜底）与 npm 更新
use std::time::Duration;

/// 安装/更新日志路径（前端进度反馈）
fn install_log_path() -> std::path::PathBuf {
    let la = std::env::var("LOCALAPPDATA").unwrap_or_default();
    std::path::PathBuf::from(la)
        .join("dsh-up")
        .join("install.log")
}

/// 流式执行 npm：stdout/stderr 边跑边追加 install.log（前端 1.5s 轮询可见实时日志）
fn run_npm_stream(args: &[&str]) -> Result<String, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;
    let mut cmd = crate::winutil::cmd_hidden(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("无法执行 npm: {}", e))?;
    if let Some(d) = install_log_path().parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let file =
        std::fs::File::create(install_log_path()).map_err(|e| format!("创建日志失败: {}", e))?;
    let dup = file
        .try_clone()
        .unwrap_or_else(|_| file.try_clone().ok().unwrap());
    let h1 = {
        let mut f = file;
        let so = child.stdout.take();
        std::thread::spawn(move || {
            if let Some(so) = so {
                let mut reader = BufReader::new(so);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => {
                            let _ = writeln!(f, "{}", line.trim_end());
                            let _ = f.flush();
                        }
                        Err(_) => break,
                    }
                }
            }
        })
    };
    let h2 = {
        let mut f = dup;
        let se = child.stderr.take();
        std::thread::spawn(move || {
            if let Some(se) = se {
                let mut reader = BufReader::new(se);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => {
                            let _ = writeln!(f, "{}", line.trim_end());
                            let _ = f.flush();
                        }
                        Err(_) => break,
                    }
                }
            }
        })
    };
    let status = child
        .wait()
        .map_err(|e| format!("等待 npm 结束失败: {}", e))?;
    let _ = h1.join();
    let _ = h2.join();
    let tail = std::fs::read_to_string(install_log_path()).unwrap_or_default();
    let tail = tail
        .chars()
        .rev()
        .take(800)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    if !status.success() {
        return Err(format!("npm 操作失败: {}", tail.trim()));
    }
    Ok(tail)
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
        Some("npmjs") => vec!["https://registry.npmjs.org/@deepseek-ai/dsh/latest".to_string()],
        Some(m) if m.starts_with("custom:") => {
            let url = m.trim_start_matches("custom:");
            let mut v = vec![format!(
                "{}/@deepseek-ai/dsh/latest",
                url.trim_end_matches('/')
            )];
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
            format!(
                "--registry={}",
                m.trim_start_matches("custom:").trim_end_matches('/')
            )
        }
        _ => "--registry=https://registry.npmmirror.com".to_string(),
    }
}

/// 查询 registry 最新版本；全部失败返回 Err（上层静默降级为离线）
pub fn registry_latest() -> Result<String, String> {
    for url in registry_chain() {
        match ureq::get(url.as_str())
            .timeout(Duration::from_secs(4))
            .call()
        {
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
    match (
        semver::Version::parse(local),
        semver::Version::parse(latest),
    ) {
        (Ok(l), Ok(r)) => l < r,
        _ => local != latest,
    }
}

/// 执行 npm 全局更新（走镜像设置一致的 registry，--registry 参数）
pub fn update_dsh() -> Result<String, String> {
    let mirror_arg = mirror_registry_arg(setting("mirror").as_deref());
    run_npm_stream(&[
        "/C",
        "npm",
        "install",
        "-g",
        "@deepseek-ai/dsh@latest",
        &mirror_arg,
    ])
}

/// 安装 dsh（体检引导用）
pub fn install_dsh() -> Result<String, String> {
    let mirror_arg = mirror_registry_arg(setting("mirror").as_deref());
    run_npm_stream(&[
        "/C",
        "npm",
        "install",
        "-g",
        "@deepseek-ai/dsh",
        &mirror_arg,
    ])
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outdated_semver_compare() {
        assert!(is_outdated("0.2.4", "0.2.5"));
        assert!(!is_outdated("0.2.5", "0.2.5"));
        assert!(!is_outdated("0.2.10", "0.2.9"));
        // 预发布：rc 低于正式版
        assert!(is_outdated("0.2.4", "0.2.5-rc.1"));
        // rc → 正式版：确实可更新
        assert!(is_outdated("0.2.5-rc.1", "0.2.5"));
    }

    #[test]
    fn mirror_arg_mapping() {
        assert!(mirror_registry_arg(Some("npmmirror")).contains("npmmirror"));
        assert!(mirror_registry_arg(Some("npmjs")).contains("npmjs.org"));
        // custom：去尾斜杠
        assert_eq!(
            mirror_registry_arg(Some("custom:https://reg.example.com/")),
            "--registry=https://reg.example.com"
        );
        // 未设置 → 默认 npmmirror
        assert!(mirror_registry_arg(None).contains("npmmirror"));
    }
}
