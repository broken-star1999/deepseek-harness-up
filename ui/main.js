// dsh-up Desktop 前端逻辑（Tauri v2, withGlobalTauri）
const invoke = window.__TAURI__.core.invoke;

const UI = {
  page: "console",

  init() {
    // 侧栏切换
    document.querySelectorAll(".nav").forEach((btn) => {
      btn.onclick = () => {
        document.querySelectorAll(".nav").forEach((b) => b.classList.remove("active"));
        btn.classList.add("active");
        this.page = btn.dataset.page;
        document.querySelectorAll(".page").forEach((p) => p.classList.remove("active"));
        const target = document.getElementById("page-" + this.page);
        if (target) target.classList.add("active");
        this.loadPage();
      };
    });
    this.refresh();
    setInterval(() => {
      if (this.page === "console") this.refreshStatus(true);
    }, 10000);
  },

  async refresh() {
    await this.refreshStatus();
    if (this.page === "console") await this.loadConsole();
  },

  async refreshStatus(silent) {
    try {
      const s = await invoke("get_status");
      const badge = document.getElementById("dsh-status");
      if (s.running) {
        badge.textContent = "● dsh 运行中";
        badge.className = "status status-running";
      } else if (s.booting) {
        badge.textContent = "◌ dsh 启动中…";
        badge.className = "status status-booting";
      } else {
        badge.textContent = "○ dsh 未运行";
        badge.className = "status status-idle";
      }
      document.getElementById("version-badge").textContent =
        "版本 " + (s.dshVersion || "未安装") + (s.npmOnline === false ? " · 离线" : "");
      const ub = document.getElementById("update-badge");
      if (s.updateAvailable) { ub.classList.remove("hidden"); ub.textContent = "可更新 → " + s.latestVersion; }
      else ub.classList.add("hidden");
    } catch (e) { /* 静默 */ }
  },

  async loadConsole() {
    try {
      const s = await invoke("get_status");
      const big = document.getElementById("console-big-status");
      big.textContent = s.running ? "🟢 dsh 运行中" : s.booting ? "🟡 启动中…" : "⚪ dsh 未运行";
      document.getElementById("console-detail").textContent = s.detail || "";
      document.getElementById("btn-start").disabled = s.running || s.booting || !s.installed;
      document.getElementById("btn-stop").disabled = !s.running;
    } catch (e) {
      document.getElementById("console-detail").textContent = "读取状态失败: " + e;
    }
  },

  async startDsh() {
    this.log("console-log", "启动 dsh…");
    try {
      const r = await invoke("start_dsh");
      this.log("console-log", r.ok ? "✅ dsh 已启动" : "启动失败: " + (r.message || ""), r.ok);
      this.refresh();
    } catch (e) { this.log("console-log", "启动失败: " + e, false); }
  },

  async stopDsh() {
    // 前端确认：安全护栏（场景3B/5）
    if (!confirm("确定停止 dsh 吗？（DSH 服务将关闭）")) return;
    this.log("console-log", "停止 dsh…");
    try {
      const r = await invoke("stop_dsh");
      this.log("console-log", r.ok ? "✅ dsh 已停止" : "取消失败: " + (r.message || ""), r.ok);
      this.refresh();
    } catch (e) { this.log("console-log", "停止失败: " + e, false); }
  },

  async loadPage() {
    if (this.page === "update") await this.loadUpdate();
    else if (this.page === "checks") await this.loadChecks();
    else if (this.page === "uninstall") await this.loadUninstall();
    else if (this.page === "about") await this.loadAbout();
  },

  async loadUpdate() {
    const box = document.getElementById("update-box");
    box.textContent = "正在检查更新…";
    this.log("update-log", "");
    try {
      const u = await invoke("check_update");
      if (!u.online) { box.textContent = "离线状态，无法检查更新（不影响使用）"; return; }
      if (u.outdated) {
        box.innerHTML = "当前 <b>" + u.local + "</b> → 最新 <b>" + u.latest + "</b> " +
          '<button class="btn primary" onclick="UI.doUpdate()">更新 dsh</button>';
      } else {
        box.innerHTML = "你已是最新版本 <b>" + u.local + "</b> ✅";
      }
    } catch (e) { box.textContent = "检查失败: " + e; }
  },

  async doUpdate() {
    this.log("update-log", "npm i -g @deepseek-ai/dsh@latest …");
    try {
      // 后端以事件流推送日志（占位：先一次性返回）
      const r = await invoke("update_dsh");
      this.log("update-log", r.message || "更新完成", r.ok);
      if (r.ok) this.refreshStatus(true);
    } catch (e) { this.log("update-log", "更新失败: " + e, false); }
  },

  async loadChecks() {
    const box = document.getElementById("checks-box");
    box.textContent = "体检中…";
    try {
      const items = await invoke("env_check");
      box.innerHTML = items.map((it) =>
        '<div class="chk-item"><span class="dot ' + (it.ok ? "ok" : it.warn ? "warn" : "bad") + '"></span>' +
        "<b>" + it.name + "</b>&nbsp;" + (it.detail || "") +
        (it.action ? ' <button class="btn" onclick="UI.doAction(' + JSON.stringify(it.action).replace(/"/g, "&quot;") + ')">' + it.actionLabel + "</button>" : "") +
        "</div>"
      ).join("");
    } catch (e) { box.textContent = "体检失败: " + e; }
  },

  async doAction(action) {
    if (!action) return;
    try { const r = await invoke("env_action", { action }); alert(r.message || "已执行"); this.loadChecks(); }
    catch (e) { alert("执行失败: " + e); }
  },

  async loadUninstall() {
    document.getElementById("uninstall-desc").textContent =
      "将移除全局安装的 @deepseek-ai/dsh（含 dsh 命令 shim）。不勾选任何选项时，你的用户配置（~/.dsh）将完整保留。";
  },

  async uninstall() {
    const cfg = document.getElementById("chk-config").checked;
    const npx = document.getElementById("chk-npx").checked;
    let msg = "确定卸载 dsh？将移除全局包。";
    if (cfg) msg += "\n⚠ 将同时删除 ~/.dsh（profiles、会话记录等），不可恢复！";
    if (npx) msg += "\n⚠ 将同时清理 npx 缓存目录。";
    if (!confirm(msg)) return;
    this.log("uninstall-log", "卸载中…");
    try {
      const r = await invoke("uninstall", { clearConfig: cfg, clearNpx: npx });
      this.log("uninstall-log", r.message || "完成", r.ok);
      this.refreshStatus(true);
    } catch (e) { this.log("uninstall-log", "卸载失败: " + e, false); }
  },

  async loadAbout() {
    document.getElementById("about-version").textContent = "dsh-up Desktop v" +
      (await invoke("get_status")).appVersion + " · Tauri 2 · 独立外部工具";
  },

  log(id, text, ok) {
    const el = document.getElementById(id);
    if (!el) return;
    if (text === "") { el.textContent = ""; return; }
    const line = document.createElement("div");
    line.className = ok === undefined ? "" : ok ? "ok" : "err";
    line.textContent = text;
    el.appendChild(line);
    el.scrollTop = el.scrollHeight;
  },
};

window.UI = UI;
window.addEventListener("DOMContentLoaded", () => UI.init());
