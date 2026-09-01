// 版本检查（npm registry，npmmirror 优先 + 官方兜底）与 npm 更新
use std::path::{Path, PathBuf};
use std::time::Duration;
use url::Url;

const MAX_REGISTRY_LEN: usize = 2048;

/// 安装/更新日志路径（前端进度反馈）
fn install_log_path() -> Result<PathBuf, String> {
    crate::paths::install_log_path()
}

/// 流式执行 npm：stdout/stderr 边跑边追加 install.log（前端 1.5s 轮询可见实时日志）。
/// npm.cmd 是批处理文件，因此由 winutil 统一负责隐藏窗口和参数引用。
fn run_npm_stream(npm_path: &Path, args: &[String]) -> Result<String, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;

    let log_path = install_log_path()?;
    if let Some(d) = log_path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("创建日志目录失败: {}", e))?;
    }
    // 先创建日志文件，再启动 npm；避免日志失败时遗留孤儿 npm 进程。
    let file = std::fs::File::create(&log_path).map_err(|e| format!("创建日志失败: {}", e))?;
    let dup = file
        .try_clone()
        .map_err(|e| format!("复制日志句柄失败: {}", e))?;
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut cmd = crate::winutil::batch_hidden(npm_path, &arg_refs)?;
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("无法执行 npm: {}", e))?;
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
    let status = match child.wait() {
        Ok(status) => status,
        Err(e) => {
            let _ = child.kill();
            let _ = h1.join();
            let _ = h2.join();
            return Err(format!("等待 npm 结束失败: {}", e));
        }
    };
    let _ = h1.join();
    let _ = h2.join();
    let tail = std::fs::read_to_string(&log_path).unwrap_or_default();
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

/// 读取设置(镜像等)。设置路径异常时按未设置处理，执行 npm 时仍会再次校验。
fn setting(key: &str) -> Option<String> {
    let p = crate::paths::settings_path().ok()?;
    let text = std::fs::read_to_string(p).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get(key)?.as_str().map(|s| s.to_string())
}

fn contains_forbidden_registry_char(value: &str) -> bool {
    value.chars().any(|c| {
        c.is_control()
            || c.is_whitespace()
            || matches!(c, '&' | '|' | '<' | '>' | '^' | '%' | '!' | '"' | '\'')
    })
}

/// 校验并规范化 npm registry 基地址。
///
/// registry 参数最终会进入 Windows npm.cmd 调用，因此除了 URL 语义校验外，
/// 还拒绝 cmd 元字符、认证信息、query 和 fragment，避免 URL 被解释成命令。
pub fn validate_registry_url(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("自定义镜像地址不能为空".into());
    }
    if value.len() > MAX_REGISTRY_LEN {
        return Err("自定义镜像地址过长".into());
    }
    if contains_forbidden_registry_char(value) {
        return Err("自定义镜像包含空白、控制字符或不安全命令字符".into());
    }
    let parsed = Url::parse(value).map_err(|_| "自定义镜像不是有效 URL".to_string())?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("自定义镜像只允许 http 或 https".into());
    }
    if parsed.host_str().is_none() {
        return Err("自定义镜像缺少主机名".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("自定义镜像不允许携带用户名或密码".into());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("自定义镜像不允许携带 query 或 fragment".into());
    }
    let normalized = value.trim_end_matches('/');
    if normalized.is_empty() {
        return Err("自定义镜像地址无效".into());
    }
    Ok(normalized.to_string())
}

/// 规范化镜像设置：npmmirror / npmjs / custom:<url>。
pub fn normalize_mirror(mirror: Option<&str>) -> Result<String, String> {
    let value = mirror.unwrap_or("npmmirror").trim();
    match value {
        "npmmirror" | "npmjs" => Ok(value.to_string()),
        _ => {
            let custom = value
                .strip_prefix("custom:")
                .ok_or_else(|| "无效镜像，支持 npmmirror、npmjs 或 custom:<url>".to_string())?;
            Ok(format!("custom:{}", validate_registry_url(custom)?))
        }
    }
}

/// 返回适合写入日志的镜像标识，不记录自定义 URL 中可能存在的敏感信息。
pub fn mirror_log_value(mirror: &str) -> String {
    match normalize_mirror(Some(mirror)) {
        Ok(value) if value == "npmmirror" || value == "npmjs" => value,
        Ok(value) => {
            let raw = value.trim_start_matches("custom:");
            Url::parse(raw)
                .ok()
                .and_then(|u| u.host_str().map(|h| format!("custom host={}", h)))
                .unwrap_or_else(|| "custom host=<invalid>".into())
        }
        Err(_) => "invalid".into(),
    }
}

fn registry_chain() -> Result<Vec<String>, String> {
    let normalized = normalize_mirror(setting("mirror").as_deref())?;
    Ok(match normalized.as_str() {
        "npmjs" => vec!["https://registry.npmjs.org".to_string()],
        value if value.starts_with("custom:") => vec![
            value.trim_start_matches("custom:").to_string(),
            "https://registry.npmmirror.com".to_string(),
        ],
        _ => vec![
            "https://registry.npmmirror.com".to_string(),
            "https://registry.npmjs.org".to_string(),
        ],
    })
}

fn package_latest_url(registry: &str) -> String {
    format!("{}/@deepseek-ai/dsh/latest", registry.trim_end_matches('/'))
}

/// 构造 npm `--registry` 参数（与设置镜像一致）。
pub fn mirror_registry_arg(mirror: Option<&str>) -> Result<String, String> {
    let normalized = normalize_mirror(mirror)?;
    let registry = match normalized.as_str() {
        "npmjs" => "https://registry.npmjs.org".to_string(),
        "npmmirror" => "https://registry.npmmirror.com".to_string(),
        value => value.trim_start_matches("custom:").to_string(),
    };
    Ok(format!("--registry={registry}"))
}

/// 查询 registry 最新版本；全部失败返回 Err（上层静默降级为离线）。
pub fn registry_latest() -> Result<String, String> {
    for registry in registry_chain()? {
        let url = package_latest_url(&registry);
        match ureq::get(&url).timeout(Duration::from_secs(4)).call() {
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

/// semver 比较（prerelease 低于正式版，如 0.1.1-rc.2 < 0.1.1）。
pub fn is_outdated(local: &str, latest: &str) -> bool {
    match (
        semver::Version::parse(local),
        semver::Version::parse(latest),
    ) {
        (Ok(l), Ok(r)) => l < r,
        _ => local != latest,
    }
}

fn npm_path() -> Result<PathBuf, String> {
    crate::dsh_locator::npm_cmd_path().ok_or_else(|| "未找到 npm，请先安装 Node.js".to_string())
}

fn npm_args(action: &str, package: &str, mirror: Option<&str>) -> Result<Vec<String>, String> {
    let mirror_arg = mirror_registry_arg(mirror)?;
    Ok(vec![
        action.to_string(),
        "-g".into(),
        package.to_string(),
        mirror_arg,
    ])
}

/// 执行 npm 全局更新（走镜像设置一致的 registry，--registry 参数）。
pub fn update_dsh() -> Result<String, String> {
    let npm = npm_path()?;
    let args = npm_args(
        "install",
        "@deepseek-ai/dsh@latest",
        setting("mirror").as_deref(),
    )?;
    run_npm_stream(&npm, &args)
}

/// 安装 dsh（体检引导用）。
pub fn install_dsh() -> Result<String, String> {
    let npm = npm_path()?;
    let args = npm_args("install", "@deepseek-ai/dsh", setting("mirror").as_deref())?;
    run_npm_stream(&npm, &args)
}

/// 卸载 dsh 使用同一个 npm runner，保证镜像、路径和窗口行为一致。
pub fn uninstall_dsh() -> Result<String, String> {
    let npm = npm_path()?;
    let args = npm_args(
        "uninstall",
        "@deepseek-ai/dsh",
        setting("mirror").as_deref(),
    )?;
    run_npm_stream(&npm, &args)
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
        assert!(mirror_registry_arg(Some("npmmirror"))
            .unwrap()
            .contains("npmmirror"));
        assert!(mirror_registry_arg(Some("npmjs"))
            .unwrap()
            .contains("npmjs.org"));
        assert_eq!(
            mirror_registry_arg(Some("custom:https://reg.example.com/")).unwrap(),
            "--registry=https://reg.example.com"
        );
        assert!(mirror_registry_arg(None).unwrap().contains("npmmirror"));
    }

    #[test]
    fn registry_validation_rejects_shell_injection_and_credentials() {
        for value in [
            "https://reg.example.com&echo injected",
            "https://reg.example.com&&whoami",
            "https://reg.example.com|whoami",
            "https://reg.example.com%PATH%",
            "https://user:password@reg.example.com",
            "file:///tmp/registry",
            "https://reg.example.com?a=b",
            "https://reg.example.com#fragment",
        ] {
            assert!(validate_registry_url(value).is_err(), "accepted: {value}");
        }
    }

    #[test]
    fn registry_validation_accepts_normal_registry_bases() {
        assert_eq!(
            validate_registry_url("https://registry.example.com/npm/").unwrap(),
            "https://registry.example.com/npm"
        );
        assert_eq!(
            normalize_mirror(Some("custom:http://127.0.0.1:4873")).unwrap(),
            "custom:http://127.0.0.1:4873"
        );
    }

    #[test]
    fn mirror_log_value_does_not_include_custom_path_or_credentials() {
        assert_eq!(
            mirror_log_value("custom:https://registry.example.com/npm"),
            "custom host=registry.example.com"
        );
        assert_eq!(mirror_log_value("npmmirror"), "npmmirror");
    }
}
