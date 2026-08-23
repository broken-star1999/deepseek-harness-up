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

const invoke = window.__TAURI__.core.invoke;

const M = {
  async init() {
    document.getElementById('modal-ok').onclick = () => this.confirm();
    // 失焦/点透明区/Esc = 取消
    document.body.addEventListener('mousedown', (ev) => {
      if (ev.target === document.body) invoke('hide_dialog_window');
    });
    window.addEventListener('blur', () => { invoke('hide_dialog_window'); });
    window.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape') invoke('hide_dialog_window');
    });
  },

  async confirm() {
    const mode = document.querySelector('input[name="close-mode"]:checked').value;
    const noRemind = document.getElementById('modal-default').checked;
    // 单 IPC：一切交给 Rust（避免弹窗窗口销毁后后续命令发不出）
    try { await invoke('dialog_confirm', { mode, noRemind }); } catch (e) {
      console.error('dialog_confirm failed:', e);
    }
  },
};

window.addEventListener('DOMContentLoaded', () => M.init());