// dsh 可执行文件定位：node / npm 全局 root / dsh 包目录（绝对路径探测，不依赖 PATH shim）
use std::path::PathBuf;

#[derive(Clone)]
#[allow(dead_code)] // 字段为诊断/调试保留
pub struct DshLocator {
    pub node: Option<String>,
    pub bin_js: Option<PathBuf>,
    pub pkg_dir: Option<PathBuf>,
    pub dsh_cmd: Option<String>,
    pub npm_root: Option<String>,
}

impl DshLocator {
    pub fn installed(&self) -> bool {
        self.bin_js.is_some() && self.node.is_some()
    }
    pub fn version(&self) -> Option<String> {
        self.pkg_dir.as_ref().and_then(|d| pkg_version(d))
    }
}

/// npm 全局 root（例: C:\Users\xxx\AppData\Roaming\npm）
pub fn npm_global_root() -> Option<String> {
    let out = crate::winutil::cmd_hidden(&["/C", "npm", "root", "-g"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if root.is_empty() { None } else { Some(root) }
}

/// 在 PATH 中查找第一个可执行文件（node / dsh）
pub fn where_first(name: &str) -> Option<String> {
    let out = crate::winutil::cmd_hidden(&["/C", "where", name]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        let t = line.trim();
        if !t.is_empty() && t.to_lowercase().ends_with(".exe") {
            return Some(t.to_string());
        }
    }
    None
}

/// 完整定位：node + dsh 包（npm root -g 优先，其次 where dsh 所在前缀）
pub fn locate() -> DshLocator {
    let node = where_first("node");
    let npm_root = npm_global_root();
    let mut pkg_dir = npm_root
        .as_ref()
        .map(|r| PathBuf::from(r).join("@deepseek-ai").join("dsh"));

    // 兜底：从 dsh.cmd shim 推导（shim 位于 <prefix>\dsh.cmd → node_modules 在其上级）
    if pkg_dir.as_ref().map(|d| !d.join("lib/bin.js").exists()).unwrap_or(true) {
        if let Some(shim) = where_first("dsh") {
            let sp = PathBuf::from(&shim);
            if let Some(parent) = sp.parent() {
                let cand = parent.join("node_modules").join("@deepseek-ai").join("dsh");
                if cand.join("lib/bin.js").exists() {
                    pkg_dir = Some(cand);
                }
            }
        }
    }

    let bin_js = pkg_dir.as_ref().and_then(|d| {
        let p = d.join("lib").join("bin.js");
        if p.exists() { Some(p) } else { None }
    });

    DshLocator {
        node,
        bin_js,
        pkg_dir,
        dsh_cmd: where_first("dsh"),
        npm_root,
    }
}

/// 读取已安装包的 package.json 版本
pub fn pkg_version(pkg_dir: &std::path::Path) -> Option<String> {
    let p = pkg_dir.join("package.json");
    let text = std::fs::read_to_string(p).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("version")?.as_str().map(|s| s.to_string())
}