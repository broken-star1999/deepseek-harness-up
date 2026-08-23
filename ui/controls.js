const invoke = window.__TAURI__.core.invoke;

const CTL = {
  defaultAction: null,

  async init() {
    try { this.defaultAction = (await invoke('get_close_default')) || null; } catch (e) { this.defaultAction = null; }
    this.applyTheme();

    document.getElementById('btn-min').onclick = () => invoke('win_minimize');
    document.getElementById('btn-max').onclick = () => invoke('win_toggle_maximize');
    document.getElementById('btn-close').onclick = () => this.requestClose();

    const dz = document.getElementById('drag-zone');
    dz.addEventListener('mousedown', (ev) => { if (ev.button === 0) invoke('start_drag'); });

    window.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape') {
        if (this.defaultAction) this.doDefault();
        else invoke('back_to_launcher');
      }
    });
  },

  async applyTheme() {
    try {
      const hex = await invoke('get_dsh_theme');
      if (!hex || !/^#[0-9a-fA-F]{6}$/.test(hex)) return;
      const r = parseInt(hex.slice(1, 3), 16);
      const g = parseInt(hex.slice(3, 5), 16);
      const b = parseInt(hex.slice(5, 7), 16);
      const lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
      const dark = lum < 0.5;
      document.getElementById('bar').style.setProperty('--bar-top', hex);
      document.getElementById('bar').style.setProperty('--bar-bottom', this.shade(hex, dark ? -18 : 10));
      document.getElementById('bar').style.setProperty('--bar-fg', dark ? '#dbe4f5' : '#1c2434');
      document.getElementById('bar').style.setProperty('--bar-hover', dark ? 'rgba(255,255,255,0.10)' : 'rgba(0,0,0,0.08)');
      document.getElementById('bar').style.setProperty('--bar-border', dark ? 'rgba(120,140,200,0.12)' : 'rgba(0,0,0,0.15)');
    } catch (e) { }
  },

  shade(hex, amt) {
    const n = (c) => Math.max(0, Math.min(255, c + amt));
    const r = n(parseInt(hex.slice(1, 3), 16));
    const g = n(parseInt(hex.slice(3, 5), 16));
    const b = n(parseInt(hex.slice(5, 7), 16));
    return '#' + [r, g, b].map((x) => x.toString(16).padStart(2, '0')).join('');
  },

  async requestClose() {
    // 刷新一次记忆设置（弹窗中勾选过的"不再提醒"立即生效）
    try { this.defaultAction = (await invoke('get_close_default')) || null; } catch (e) { this.defaultAction = null; }
    if (this.defaultAction) { this.doDefault(); return; }
    try { await invoke('show_dialog_window'); } catch (e) {}
  },

  doDefault() {
    if (this.defaultAction === 'exit') invoke('win_close');
    else if (this.defaultAction === 'minimize') invoke('win_minimize');
    else invoke('back_to_launcher');
  },
};

window.CTL = CTL;
window.addEventListener('DOMContentLoaded', () => CTL.init());