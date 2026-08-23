// 悬浮控制条逻辑
const invoke = window.__TAURI__.core.invoke;

const CTL = {
  defaultAction: null, // 'exit' | 'minimize'

  async init() {
    try { this.defaultAction = (await invoke('get_close_default')) || null; } catch (e) { this.defaultAction = null; }

    document.getElementById('btn-min').onclick = () => invoke('win_minimize');
    document.getElementById('btn-max').onclick = () => invoke('win_toggle_maximize');
    document.getElementById('btn-close').onclick = () => this.requestClose();

    // 顶部拖拽区（无边框窗口拖动）
    const dz = document.getElementById('drag-zone');
    dz.addEventListener('mousedown', (ev) => {
      if (ev.button !== 0) return;
      invoke('start_drag');
    });

    // 弹窗按钮
    document.getElementById('modal-exit').onclick = () => this.choose('exit');
    document.getElementById('modal-min').onclick = () => this.choose('minimize');
    document.getElementById('modal-cancel').onclick = () => this.cancel();

    // Esc 返回页面 1
    window.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape') {
        if (!document.getElementById('modal').classList.contains('hidden')) this.cancel();
        else if (this.defaultAction) this.doDefault();
        else invoke('back_to_launcher');
      }
    });

    // 弹窗显示时占满全屏（这样弹窗可居中显示在 DSH 之上）
    this._modalShown = false;
  },

  async requestClose() {
    if (this.defaultAction) { this.doDefault(); return; }
    // 弹窗 → 请求主 UI 扩展本 webview 到全屏
    this._modalShown = true;
    try { await invoke('show_modal_overlay'); } catch (e) {}
    document.getElementById('modal').classList.remove('hidden');
  },

  async choose(action) {
    const asDefault = document.getElementById('modal-default').checked;
    if (asDefault) { try { await invoke('set_close_default', { value: action }); } catch (e) {} }
    await this.closeModal();
    if (action === 'exit') await invoke('win_close');
    else await invoke('win_minimize');
  },

  doDefault() {
    if (this.defaultAction === 'exit') invoke('win_close');
    else if (this.defaultAction === 'minimize') invoke('win_minimize');
    else invoke('back_to_launcher');
  },

  async cancel() {
    await this.closeModal();
  },

  async closeModal() {
    if (this._modalShown) {
      try { await invoke('hide_modal_overlay'); } catch (e) {}
      this._modalShown = false;
    }
    document.getElementById('modal').classList.add('hidden');
  },
};

window.CTL = CTL;
window.addEventListener('DOMContentLoaded', () => CTL.init());