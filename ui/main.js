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

// dsh-up Desktop 前端（两页制：启动器 / DSH 全窗口）
const invoke = window.__TAURI__.core.invoke;

const UI = {
  ctx: null,          // 环境状态缓存
  busy: false,        // 忙态（安装/启动中）
  embedActive: false,
  controlsActive: false,
  _resizeTimer: null,

  async init() {
    // 统一顶栏：启动即显示（logo/名称/拖拽/─□✕）
    try { await invoke('show_controls', this.ctrlRect()); } catch (e) {}
    try { invoke('fe_log', { msg: 'UI init: launcher ready' }); } catch (e) {}
    // Esc 返回页面 1 后的事件
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
    document.getElementById('btn-dsh').onclick = () => this.busyAction('installDsh');
    document.getElementById('btn-core').onclick = () => this.launch();
    document.getElementById('btn-open').onclick = () => this.openDsh();

    window.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape') {
        if (!document.getElementById('modal').classList.contains('hidden')) this.closeModal();
      }
    });

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
    const c = this.ctx;
    const hasNode = !!c.nodeVersion;
    const hasDsh = c.installed;
    // 版本信息
    document.getElementById('version-badge').textContent =
      c.dshVersion ? '版本 ' + c.dshVersion : (hasNode ? '环境就绪' : '');
    // ① 环境缺失：只显示对应安装按钮
    document.getElementById('btn-node').classList.toggle('hidden', hasNode);
    document.getElementById('btn-dsh').classList.toggle('hidden', hasDsh);
    // ② dsh 就绪：核心启动/打开界面/卸载
    const ready = hasNode && hasDsh;
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
    const openSub = document.getElementById('open-sub');
    if (!coreBtn) return;
    if (c.running) {
      // 核心已运行
      coreBtn.classList.remove('green', 'disabled');
      coreBtn.disabled = true;
      coreBtn.classList.add('green');
      coreLabel.textContent = 'dsh 核心已运行';
      openBtn.classList.remove('disabled');
      openBtn.classList.add('green');
      openBtn.disabled = false;
      openSub.textContent = '进入 127.0.0.1:3080 界面';
    } else if (c.booting) {
      coreBtn.classList.remove('green');
      coreBtn.disabled = true;
      coreLabel.textContent = '核心启动中…';
      openBtn.classList.add('disabled');
      openBtn.classList.remove('green');
      openBtn.disabled = true;
      openSub.textContent = '等待核心启动…';
    } else {
      // 未运行
      coreBtn.classList.remove('green', 'disabled');
      coreBtn.disabled = false;
      coreLabel.textContent = '启动 dsh 核心';
      openBtn.classList.add('disabled');
      openBtn.classList.remove('green');
      openBtn.disabled = true;
      openSub.textContent = '先启动核心…';
    }
  },

  async busyAction(kind) {
    if (this.busy) return;
    this.busy = true;
    document.getElementById('action-view').classList.add('hidden');
    document.getElementById('busy-view').classList.remove('hidden');
    const label = kind === 'installNode' ? '正在打开 Node.js 下载页…' : '正在安装 DeepSeek Harness…';
    document.getElementById('busy-text').textContent = label;
    try {
      if (kind === 'installNode') {
        await invoke('env_action', { action: 'node_download' });
      } else {
        await invoke('env_action', { action: 'install_dsh' });
      }
    } catch (e) {
      alert('操作失败: ' + e);
    } finally {
      this.busy = false;
      await this.checkEnv();
    }
  },

  /* ========== 更新 ========== */

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
    const btn = document.getElementById('btn-do-update');
    const log = document.getElementById('update-log');
    btn.disabled = true;
    btn.textContent = '更新中…';
    log.textContent = 'npm install -g @deepseek-ai/dsh@latest …';
    try {
      const r = await invoke('update_dsh');
      log.innerHTML = (r.ok ? '✅ ' : '⚠ ') + (r.message || '更新完成');
      btn.textContent = '完成';
      this.closeUpdateModal();
      await this.checkEnv();
    } catch (e) {
      log.innerHTML = '<span class="muted" style="color:var(--red)">失败: ' + e + '</span>';
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

  // 打开 dsh 界面（核心已运行才可点）
  async openDsh() {
    try { await invoke('fe_log', { msg: 'ACTION UI: open dsh clicked' }); } catch (e) {}
    if (this.busy) return;
    if (!(this.ctx && this.ctx.running)) {
      return;
    }
    this.busy = true;
    // 进入页面 2
    document.getElementById('launcher').classList.add('hidden');
    document.getElementById('dsh-page').classList.remove('hidden');
    this.busy = false;
    await this.layoutDshPage();
    try { await invoke('show_embed', this.embedRect()); } catch (e) {}
    this.embedActive = true;
    this.controlsActive = true;
    document.getElementById('dsh-loading').classList.add('hidden');
  },

  waitPort(seconds) {
    return new Promise((resolve) => {
      const t0 = Date.now();
      const timer = setInterval(async () => {
        try {
          const s = await invoke('get_status');
          if (s.running) { clearInterval(timer); resolve(true); return; }
        } catch (e) {}
        if (Date.now() - t0 > seconds * 1000) { clearInterval(timer); resolve(false); }
      }, 1000);
    });
  },

  async fail(msg) {
    this.busy = false;
    alert(msg);
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
      log.innerHTML = (r.ok ? '' : '⚠ ') + (r.message || '完成');
    } catch (e) { log.innerHTML = '<span style="color:var(--red)">失败: ' + e + '</span>'; }
    setTimeout(async () => {
      this.closeModal();
      document.getElementById('modal-log').textContent = '';
      await this.checkEnv();
    }, 1200);
  },
};

window.UI = UI;
window.addEventListener('DOMContentLoaded', () => UI.init());