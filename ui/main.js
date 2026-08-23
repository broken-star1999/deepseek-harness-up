// dsh-up Desktop 前端逻辑（Tauri v2, withGlobalTauri）
const invoke = window.__TAURI__.core.invoke;

const UI = {
  page: "console",
  embedActive: false,
  _resizeTimer: null,

  init() {
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
    window.addEventListener("resize", () => {
      clearTimeout(this._resizeTimer);
      this._resizeTimer = setTimeout(() => {
        if (this.page === "console" && this.embedActive) this.showEmbed();
      }, 150);
    });
    this.refresh();
    setInterval(() => {
      if (this.page === "console") this.refreshStatus(true);
    }, 10000);
  },

  async refresh() {
    await this.refreshStatus();
    if (this.page === "console") this.syncEmbed();
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
        "版本 " + (s.dshVersion || "未安装");
      const ub = document.getElementById("update-badge");
      if (s.updateAvailable) { ub.classList.remove("hidden"); ub.textContent = "可更新 → " + s.latestVersion; }
      else ub.classList.add("hidden");
      if (this.page === "console") {
        const st = document.getElementById("console-state");
        st.textContent = s.detail || "";
        document.getElementById("btn-start").disabled = s.running || !s.installed;
        document.getElementById("btn-stop").disabled = !s.running;
      }
    } catch (e) { /* 静默 */ }
  },

  startDsh() {
    this.logConsole("启动 dsh…");
  },

  async stopDsh() {
    if (!confirm("确定停止 dsh 吗？（DSH 服务将关闭）")) return;
    try {
      await invoke("stop_dsh");
      document.getElementById("console-state").textContent = "已停止";
      await this.hideEmbed();
      this.refresh();
    } catch (e) {
      alert("停止失败: " + e);
    }
  },

  /** 内嵌视图同步：按环境决定 显示/隐藏/错误态 */
  async syncEmbed() {
    if (this.page !== "console") { await this.hideEmbed(); return; }
    try {
      const s = await invoke("get_status");
      if (!s.running) {
        this.setEmbedStatus("error", "dsh 未运行 —— 点击上方「▶ 启动 dsh」", "");
        this.hideEmbed();
        return;
      }
      this.showEmbed();
    } catch (e) { /* ignore */ }
  },

  async showEmbed() {
    const slot = document.getElementById("embed-slot");
    if (!slot) return;
    const rect = slot.getBoundingClientRect();
    try {
      await invoke("show_embed", {
        x: rect.x, y: rect.y, width: rect.width, height: rect.height,
      });
      slot.classList.add("embedded");
      this.embedActive = true;
      this.setEmbedStatus("loading", "", "");
    } catch (e) {
      this.setEmbedStatus("error", "无法显示 DSH 界面", String(e));
      this.embedActive = false;
    }
  },

  async hideEmbed() {
    this.embedActive = false;
    const slot = document.getElementById("embed-slot");
    if (slot) slot.classList.remove("embedded");
    try { await invoke("hide_embed"); } catch (e) { /* ignore */ }
  },

  async refreshEmbed() {
    await this.hideEmbed();
    this.setEmbedStatus("loading", "", "");
    this.showEmbed();
  },

  setEmbedStatus(kind, title, error) {
    const holder = document.getElementById("embed-status");
    holder.className = kind === "error" ? "error-holder" : "";
    const errEl = document.getElementById("embed-error");
    const titleP = holder.querySelector("p.muted:not(.small)");
    if (titleP) titleP.textContent = title;
    errEl.textContent = error || "";
  },

  logConsole(text) {
    const st = document.getElementById("console-state");
    if (st) st.textContent = text;
  },

  loadPage() {
    if (this.page === "update") this.loadUpdate();
    else if (this.page === "checks") this.loadChecks();
    else if (this.page === "uninstall") this.loadUninstall();
    else if (this.page === "about") this.loadAbout();
    else if (this.page === "console") this.syncEmbed();
  },

  async loadUpdate() {
    const box = document.getElementById("update-box");
    box.textContent = "正在检查更新…";
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
    const log = document.getElementById("update-log");
    log.textContent = "npm i -g @deepseek-ai/dsh@latest …";
    try {
      const r = await invoke("update_dsh");
      log.innerHTML = (r.ok ? "" : "⚠ ") + (r.message || "更新完成");
      this.refreshStatus(true);
    } catch (e) {
      log.innerHTML = "<span class='err'>更新失败: " + e + "</span>";
    }
  },

  async loadChecks() {
    const box = document.getElementById("checks-box");
    box.textContent = "体检中…";
    try {
      const items = await invoke("env_check");
      box.innerHTML = items.map((it) =>
        '<div class="chk-item"><span class="dot ' + (it.ok ? "ok" : it.warn ? "warn" : "bad") + '"></span>' +
        "<b>" + it.name + "</b>&nbsp;" + (it.detail || "") +
        (it.action ? ' <button class="btn small-btn" onclick="UI.doAction(' + JSON.stringify(it.action).replace(/"/g, "&quot;") + ')">' + it.actionLabel + "</button>" : "") +
        "</div>"
      ).join("");
    } catch (e) { box.textContent = "体检失败: " + e; }
  },

  async doAction(action) {
    if (!action) return;
    try {
      const r = await invoke("env_action", { action });
      alert(r.message || "已执行");
      this.loadChecks();
      await this.refreshStatus(true);
    } catch (e) { alert("执行失败: " + e); }
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
    const log = document.getElementById("uninstall-log");
    log.textContent = "卸载中…";
    try {
      const r = await invoke("uninstall", { clearConfig: cfg, clearNpx: npx });
      log.innerHTML = (r.ok ? "" : "⚠ ") + (r.message || "完成");
      this.refreshStatus(true);
    } catch (e) {
      log.innerHTML = "<span class='err'>卸载失败: " + e + "</span>";
    }
  },

  async loadAbout() {
    try {
      const s = await invoke("get_status");
      document.getElementById("about-version").textContent =
        "dsh-up Desktop v" + s.appVersion + " · Tauri 2 · 独立外部工具";
    } catch (e) { /* ignore */ }
  },
};

window.UI = UI;
window.addEventListener("DOMContentLoaded", () => UI.init());
