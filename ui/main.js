// 全局错误 → 运行日志
window.addEventListener("error", (ev) => {
  try { invoke("fe_log", { msg: "ERROR " + (ev.message || "") + " @ " + (ev.filename || "") + ":" + (ev.lineno || "") }); } catch (e) {}
});
window.addEventListener("unhandledrejection", (ev) => {
  try { invoke("fe_log", { msg: "REJECTION " + (ev.reason && ev.reason.message ? ev.reason.message : String(ev.reason || "")) }); } catch (e) {}
});
window.addEventListener("DOMContentLoaded", () => {
  try { invoke("fe_log", { msg: "PAGE LOADED " + location.pathname.split("/").pop() }); } catch (e) {}
});

// DeepSeek Harness Up 前端（两页制：启动器 / DSH 全窗口）
const invoke = window.__TAURI__.core.invoke;

const UI = {
  ctx: null,          // 环境状态缓存
  busy: false,        // 忙态（安装/启动中）
  embedActive: false,
  controlsActive: false,
  _resizeTimer: null,

  async init() {
    // 应用壁纸（自定义 or 默认）
    try { await this.applyBg(); } catch (e) {}
    // 统一顶栏：启动即显示（logo/名称/拖拽/─□✕）
    try { await invoke('show_controls', this.ctrlRect()); } catch (e) {}
    try { invoke('fe_log', { msg: 'UI init: launcher ready' }); } catch (e) {}
    // Esc 返回页面 1 后的事件
    try {
      window.__TAURI__.event.listen('open-settings', () => {
        try { invoke('fe_log', { msg: 'EVENT received: open-settings' }); } catch (e) {}
        this.openSettings();
      });
    } catch (e) {}
    try {
      window.__TAURI__.event.listen('back-to-launcher', async () => {
        this.embedActive = false;
        this.controlsActive = false;
        document.getElementById('dsh-page').classList.add('hidden');
        document.getElementById('launcher').classList.remove('hidden');
        await this.checkEnv();
      });
    } catch (e) {}

    // 窗口缩放 → 同步全窗口内嵌 + 右上角悬浮条
    window.addEventListener('resize', () => {
      clearTimeout(this._resizeTimer);
      this._resizeTimer = setTimeout(() => {
        invoke('update_controls_bounds', this.ctrlRect()).catch(() => {});
        if (this.embedActive) this.layoutDshPage();
      }, 120);
    });

    document.getElementById('btn-uninstall').onclick = () => this.openModal();
    document.getElementById('btn-node').onclick = () => this.busyAction('installNode');
    document.getElementById('btn-dsh').onclick = async () => {
    if (this.ctx && !this.ctx.nodeVersion) {
      this.toast('请先安装 Node.js', 'warn');
      return;
    }
    this.busyAction('installDsh');
  };
    document.getElementById('btn-core').onclick = () => {
      if (this.ctx && this.ctx.running) this.stopCore();
      else this.launch();
    };
    document.getElementById('btn-open').onclick = () => this.openDsh();

    window.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape') {
        if (!document.getElementById('modal').classList.contains('hidden')) this.closeModal();
      }
    });

    // 页面1 状态轮询（10s）：环境变化（装完Node/dsh）自动刷新按钮，不闪转圈
    setInterval(async () => {
      if (this.embedActive) return;
      try {
        const s = await invoke('get_status');
        const c = this.ctx || {};
        const changed = c.installed !== s.installed || c.running !== s.running ||
          c.nodeVersion !== s.nodeVersion || c.dshVersion !== s.dshVersion;
        if (changed) {
          this.ctx = s;
          this.renderActions();
        }
      } catch (e) {}
    }, 10000);

    // 页面2 心跳：核心崩溃检测（15s 间隔）
    setInterval(async () => {
      if (!this.embedActive) return;
      try {
        const s = await invoke('get_status');
        if (!s.running) {
          this.toast('dsh 核心已停止，已返回启动器', 'warn');
          try { await invoke('back_to_launcher'); } catch (e) {}
        }
      } catch (e) {}
    }, 15000);

    await this.checkEnv();
  },

  /* ========== 页面 1：检查 + 按钮栈 ========== */

  async checkEnv() {
    this.showChecking();
    let ctx;
    try {
      ctx = await invoke('get_status');
    } catch (e) {
      ctx = { installed: false, nodeVersion: null, dshVersion: null, running: false };
    }
    this.ctx = ctx;
    this.renderActions();
    // 后台异步检查更新（不阻塞界面）
    try { await this.checkUpdate(); } catch (e) {}
  },

  showChecking() {
    document.getElementById('checking-view').classList.remove('hidden');
    document.getElementById('action-view').classList.add('hidden');
    document.getElementById('busy-view').classList.add('hidden');
  },

  renderActions() {
    // 设置按钮：始终显示
    const setBtn = document.getElementById('btn-settings');
    if (setBtn) setBtn.classList.remove('hidden');
    const c = this.ctx;
    const hasNode = !!c.nodeVersion;
    const hasDsh = c.installed;
    // 版本信息
    document.getElementById('version-badge').textContent =
      c.dshVersion ? '版本 ' + c.dshVersion : (hasNode ? '环境就绪' : '');
    // ① 环境缺失：只显示对应安装按钮
    document.getElementById('btn-node').classList.toggle('hidden', hasNode);
    const btnDsh = document.getElementById('btn-dsh');
    btnDsh.classList.toggle('hidden', hasDsh);
    // 无 Node 时置灰提示（仍可点，点击弹提示引导）
    const noNode = !hasNode;
    btnDsh.classList.toggle('disabled', noNode);
    const subEl = btnDsh.querySelector('.sub');
    if (subEl) subEl.textContent = noNode ? '请先安装 Node.js' : 'npm 全局安装 @deepseek-ai/dsh';
    // ② dsh 就绪：核心启动/打开界面/卸载
    const ready = hasNode && hasDsh;
    // 体检未就绪时显示重检入口（装完 Node 后点一下即刷新，无需重启）
    const recheckBtn = document.getElementById('btn-recheck');
    if (recheckBtn) recheckBtn.classList.toggle('hidden', ready);
    document.getElementById('btn-core').classList.toggle('hidden', !ready);
    document.getElementById('btn-open').classList.toggle('hidden', !ready);
    document.getElementById('btn-uninstall').classList.toggle('hidden', !hasDsh);
    this.applyButtonStates(c);
    document.getElementById('checking-view').classList.add('hidden');
    document.getElementById('action-view').classList.remove('hidden');
  },

  // 按钮色态：核心未启动=打开灰；核心运行=打开绿
  applyButtonStates(c) {
    const coreBtn = document.getElementById('btn-core');
    const openBtn = document.getElementById('btn-open');
    const coreLabel = document.getElementById('core-label');
    if (!coreBtn) return;
    // 核心按钮：未运行=绿色"启动核心"，运行中=红色"停止核心"
    if (c.running) {
      coreBtn.classList.remove('disabled', 'green');
      coreBtn.classList.add('red');
      coreBtn.disabled = false;
      coreLabel.textContent = '停止核心';
      openBtn.classList.remove('disabled');
      openBtn.classList.add('green');
      openBtn.disabled = false;
    } else if (c.booting) {
      coreBtn.classList.remove('green', 'red');
      coreBtn.classList.add('disabled');
      coreBtn.disabled = true;
      coreLabel.textContent = '核心启动中…';
      openBtn.classList.add('disabled');
      openBtn.classList.remove('green');
      openBtn.disabled = true;
    } else {
      // 未运行：绿色启动核心
      coreBtn.classList.remove('disabled', 'red');
      coreBtn.classList.add('green');
      coreBtn.disabled = false;
      coreLabel.textContent = '启动核心';
      openBtn.classList.add('disabled');
      openBtn.classList.remove('green');
      openBtn.disabled = true;
    }
  },

  async stopCore() {
    if (this.busy) return;
    this.busy = true;
    try { await invoke('fe_log', { msg: 'ACTION UI: stop core clicked' }); } catch (e) {}
    try {
      await invoke('stop_dsh');
      document.getElementById('core-label').textContent = '正在停止…';
      this.ctx.running = false;
    } catch (e) {
      try { await invoke('fe_log', { msg: 'ERROR stop core: ' + e }); } catch (e2) {}
    }
    this.busy = false;
    await this.checkEnv();
  },

  async busyAction(kind) {
    if (this.busy) return;
    // 安装/更新 dsh 前：运行中的核心占用包文件，会失败（Windows）
    if (kind === 'installDsh' && this.ctx && this.ctx.running) {
      this.toast('请先停止核心再安装 dsh', 'warn');
      return;
    }
    this.busy = true;
    document.getElementById('action-view').classList.add('hidden');
    document.getElementById('busy-view').classList.remove('hidden');
    const label = kind === 'installNode' ? '正在打开 Node.js 下载页…' : '正在安装 DeepSeek Harness（约 1-2 分钟）…';
    document.getElementById('busy-text').textContent = label;
    document.getElementById('busy-log').textContent = '';
    // 安装期间滚动 npm 日志（1.5s 间隔）
    let logTimer = null;
    if (kind === 'installDsh') {
      logTimer = setInterval(async () => {
        try {
          const tail = await invoke('tail_install_log');
          const el = document.getElementById('busy-log');
          if (el && tail) { el.textContent = tail; el.scrollTop = el.scrollHeight; }
        } catch (e) {}
      }, 1500);
    }
    try {
      if (kind === 'installNode') {
        await invoke('env_action', { action: 'node_download' });
      } else {
        await invoke('env_action', { action: 'install_dsh' });
        try { await invoke('invalidate_locator_cache'); } catch (e) {}
        if (logTimer) { clearInterval(logTimer); await this.refreshInstallLog(); }
        this.toast('✅ DeepSeek Harness 安装成功', 'ok');
      }
    } catch (e) {
      if (logTimer) clearInterval(logTimer);
      this.toast('操作失败: ' + e, 'warn');
    } finally {
      this.busy = false;
      await this.checkEnv();
    }
  },

  async refreshInstallLog() {
    try {
      const tail = await invoke('tail_install_log');
      const el = document.getElementById('busy-log');
      if (el && tail) { el.textContent = tail; }
    } catch (e) {}
  },

  /* ========== 更新 ========== */

  // 手动检查更新（界面按钮）：有新版→弹窗，无/离线→toast
  async manualCheckUpdate() {
    const btn = document.getElementById('btn-check-update');
    if (btn.disabled) return;
    btn.disabled = true;
    btn.textContent = '检查中…';
    try {
      const u = await invoke('check_update');
      btn.disabled = false;
      btn.textContent = '检查更新';
      if (u && u.outdated) {
        // 有新版 → 弹更新确认窗
        this.openUpdateModal(u);
      } else if (u) {
        this.toast('已是最新版本 ' + u.local + ' ✅', 'ok');
      }
    } catch (e) {
      btn.disabled = false;
      btn.textContent = '检查更新';
      this.toast('网络不可用，无法检查更新', 'warn');
    }
  },

  toast(msg, kind) {
    const t = document.getElementById('toast');
    if (!t) return;
    t.textContent = msg;
    t.className = kind || '';
    t.classList.remove('hidden');
    clearTimeout(this._toastTimer);
    this._toastTimer = setTimeout(() => {
      t.classList.add('hidden');
    }, 2200);
  },

  async checkUpdate() {
    try {
      const u = await invoke('check_update');
      const badge = document.getElementById('update-badge');
      if (u && u.outdated) {
        badge.textContent = '⚠ 可更新 → ' + u.latest;
        badge.classList.remove('hidden');
        badge.onclick = () => this.openUpdateModal(u);
      } else {
        badge.classList.add('hidden');
      }
    } catch (e) {
      // 离线/未装 dsh：静默
      document.getElementById('update-badge').classList.add('hidden');
    }
  },

  openUpdateModal(u) {
    document.getElementById('update-info').textContent =
      '当前版本 ' + u.local + ' → 最新版本 ' + u.latest;
    document.getElementById('update-log').textContent = '';
    document.getElementById('btn-do-update').disabled = false;
    document.getElementById('btn-do-update').textContent = '立即更新';
    document.getElementById('modal-update').classList.remove('hidden');
  },

  closeUpdateModal() {
    document.getElementById('modal-update').classList.add('hidden');
  },

  async doUpdate() {
    if (this.ctx && this.ctx.running) {
      this.toast('请先停止核心再更新', 'warn');
      return;
    }
    const btn = document.getElementById('btn-do-update');
    const log = document.getElementById('update-log');
    btn.disabled = true;
    btn.textContent = '更新中…';
    log.textContent = 'npm install -g @deepseek-ai/dsh@latest …';
    try {
      const r = await invoke('update_dsh');
      log.textContent = (r.ok ? '✅ ' : '⚠ ') + (r.message || '更新完成');
      btn.textContent = '完成';
      this.toast('dsh 已更新，核心重启后生效', 'ok');
      this.closeUpdateModal();
      await this.checkEnv();
    } catch (e) {
      log.textContent = '失败: ' + e;
      btn.disabled = false;
      btn.textContent = '重试';
    }
  },

  /* ========== 启动 → 页面 2 ========== */

  // 启动 dsh 核心（后台 dsh web --no-open）
  async launch() {
    try { await invoke('fe_log', { msg: 'ACTION UI: start core clicked' }); } catch (e) {}
    if (this.busy) return;
    if (this.ctx && this.ctx.running) return; // 已运行则忽略
    this.busy = true;
    this.ctx = this.ctx || {};
    this.ctx.booting = true;
    this.applyButtonStates(this.ctx);
    try {
      try { await invoke('fe_log', { msg: 'UI: invoking start_dsh' }); } catch (e) {}
      await invoke('start_dsh');
    } catch (e) {
      this.ctx.booting = false;
      await this.fail('dsh 核心启动失败: ' + e);
      return;
    }
    // 提示用户核心启动需要几秒到十几秒（dsh boot 较慢）
    document.getElementById('core-label').textContent = '核心启动中（约 10 秒）…';
    const ok = await this.waitPort(30);
    this.busy = false;
    if (!ok) {
      this.ctx.booting = false;
      await this.fail('等待 dsh 服务超时（30s）');
      return;
    }
    try { await this.checkEnv(); } catch (e) {}
  },

  // 手动重新体检（装完 Node/WebView2 后点一下即可，无需重启工具）
  async recheck() {
    try { await invoke('fe_log', { msg: 'ACTION UI: recheck clicked' }); } catch (e) {}
    this.showChecking();
    await this.checkEnv();
  },

  // 打开 dsh 界面（核心已运行才可点）
  async openDsh() {
    try { await invoke('fe_log', { msg: 'ACTION UI: open dsh clicked' }); } catch (e) {}
    if (this.busy) return;
    if (!(this.ctx && this.ctx.running)) {
      return;
    }
    this.busy = true;
    // 等核心端口就绪（启动有 2-15 秒窗口，过快点击会黑屏）
    const portReady = await this.waitPort(15);
    if (!portReady) {
      this.busy = false;
      this.toast('核心还在启动中，请稍候再试', 'warn');
      return;
    }
    // 先显示 dsh 页（量取边界），嵌入成功后再隐藏启动器；失败恢复
    document.getElementById('dsh-page').classList.remove('hidden');
    await this.layoutDshPage();
    try {
      await invoke('show_embed', this.embedRect());
    } catch (e) {
      // 失败恢复：隐藏 dsh 页，回到启动器（防空白卡死）
      document.getElementById('dsh-page').classList.add('hidden');
      const l = document.getElementById('launcher'); if (l) l.classList.remove('hidden');
      this.busy = false;
      await this.fail('打开失败: ' + e);
      return;
    }
    this.busy = false;
    document.getElementById('launcher').classList.add('hidden');
    this.embedActive = true;
    this.controlsActive = true;
    // 给 WebView 留一点绘制时间再隐藏加载层（避免闪黑）
    setTimeout(() => { const l = document.getElementById('dsh-loading'); if (l) l.classList.add('hidden'); }, 450);
  },

  waitPort(seconds) {
    return new Promise((resolve) => {
      const t0 = Date.now();
      const timer = setInterval(async () => {
        try {
          const s = await invoke('get_status');
          if (s.portOpen) { clearInterval(timer); resolve(true); return; }
        } catch (e) {}
        if (Date.now() - t0 > seconds * 1000) { clearInterval(timer); resolve(false); }
      }, 1000);
    });
  },

  async fail(msg) {
    this.busy = false;
    this.toast(msg, 'warn');
    await this.checkEnv();
  },

  embedRect() {
    const slot = document.getElementById('dsh-slot');
    const r = slot.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  },

  ctrlRect() {
    const w = window.innerWidth;
    // 顶部通栏拖拽条（全宽，靠渐变半透明自动适配背景色）
    return { x: 0, y: 0, width: w, height: 38 };
  },

  async layoutDshPage() {
    try { await invoke('update_embed_bounds', this.embedRect()); } catch (e) {}
    try { await invoke('update_controls_bounds', this.ctrlRect()); } catch (e) {}
  },

  /* ========== 卸载弹窗 ========== */


  async openSettings() {
    try { await invoke('show_settings_window'); } catch (e) {}
  },


  // 应用壁纸（自定义文件 or 内置默认 bg.png）
  async applyBg() {
    const launcher = document.getElementById('launcher');
    if (!launcher) return;
    try {
      const bgData = await invoke('get_bg_data');
      if (bgData) {
        launcher.style.backgroundImage = "url('" + bgData + "')";
      } else {
        launcher.style.backgroundImage = "url('bg.png')";
      }
    } catch (e) {}
  },

  openModal() {
    document.getElementById('modal').classList.remove('hidden');
  },
  closeModal() {
    document.getElementById('modal').classList.add('hidden');
  },

  async doUninstall() {
    const cfg = document.getElementById('chk-config').checked;
    const npx = document.getElementById('chk-npx').checked;
    const log = document.getElementById('modal-log');
    log.textContent = '卸载中…';
    try {
      const r = await invoke('uninstall', { clearConfig: cfg, clearNpx: npx });
      log.textContent = (r.ok ? '' : '⚠ ') + (r.message || '完成');
    } catch (e) { log.textContent = '失败: ' + e; }
    setTimeout(async () => {
      this.closeModal();
      document.getElementById('modal-log').textContent = '';
      try { await invoke('invalidate_locator_cache'); } catch (e) {}
      await this.checkEnv();
    }, 1200);
  },
};

window.UI = UI;
window.addEventListener('DOMContentLoaded', () => UI.init());