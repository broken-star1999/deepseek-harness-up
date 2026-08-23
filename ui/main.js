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
    document.getElementById('btn-launch').onclick = () => this.launch();

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
    // 按钮栈：只显示当下需要的
    document.getElementById('btn-node').classList.toggle('hidden', hasNode);
    document.getElementById('btn-dsh').classList.toggle('hidden', hasDsh);
    document.getElementById('btn-launch').classList.toggle('hidden', !(hasNode && hasDsh));
    const sub = document.getElementById('launch-sub');
    if (sub) sub.textContent = c.running ? '继续会话' : '进入 127.0.0.1:3080 界面';
    document.getElementById('checking-view').classList.add('hidden');
    document.getElementById('action-view').classList.remove('hidden');
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

  /* ========== 启动 → 页面 2 ========== */

  async launch() {
    if (this.busy) return;
    this.busy = true;
    this.showChecking();
    // 确保 dsh 运行
    const c = this.ctx || {};
    if (!c.running) {
      try { await invoke('start_dsh'); } catch (e) {
        await this.fail('dsh 启动失败: ' + e);
        return;
      }
    }
    // 等 3080 就绪
    const ok = await this.waitPort(30);
    if (!ok) { await this.fail('等待 dsh 服务超时（30s）'); return; }
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
      }, 500);
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