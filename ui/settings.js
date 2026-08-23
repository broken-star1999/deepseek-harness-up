const invoke = window.__TAURI__.core.invoke;

const SK = {
  init() {
    // 回填 ✕行为/镜像 + about
    invoke('get_close_default').then((v) => {
      const val = v || 'minimize';
      document.querySelectorAll('input[name="close-mode"]').forEach((r) => { r.checked = r.value === val; });
    }).catch(() => {});
    invoke('get_mirror').then((m) => {
      document.querySelectorAll('input[name="mirror"]').forEach((r) => {
        if (m && m.startsWith('custom:')) { r.checked = r.value === 'custom'; document.getElementById('mirror-custom').value = m.slice(7); }
        else r.checked = r.value === (m || 'npmmirror');
      });
    }).catch(() => {});
    invoke('get_status').then((s) => { document.getElementById('about-info').textContent = 'DeepSeek Harness Up v' + s.appVersion + ' · dsh ' + (s.dshVersion || '未安装'); }).catch(() => {});
    // 壁纸状态
    invoke('get_bg_data').then((d) => { document.getElementById('bg-status').textContent = d ? '自定义壁纸' : '默认壁纸'; }).catch(() => {});
    // Esc 关闭
    window.addEventListener('keydown', (ev) => { if (ev.key === 'Escape') this.close(); });
  },

  switchTab(name) {
    document.querySelectorAll('.tab').forEach((t) => t.classList.toggle('active', t.dataset.tab === name));
    document.querySelectorAll('.tab-page').forEach((p) => p.classList.toggle('active', p.id === 'tab-' + name));
  },

  async checkDshUpdate() {
    const log = document.getElementById('upd-dsh-log');
    const info = document.getElementById('upd-dsh-info');
    log.textContent = '检查中…';
    try {
      const u = await invoke('check_update');
      info.textContent = '当前 ' + u.local + (u.outdated ? ' → 最新 ' + u.latest : '（已最新）');
      if (u.outdated) log.innerHTML = '发现新版本！<button class="btn small primary" onclick="SK.doUpdate()">立即更新</button>';
      else log.textContent = '✅ 已是最新版本';
    } catch (e) { info.textContent = '离线/无法检查'; log.textContent = ''; }
  },

  async doUpdate() {
    const log = document.getElementById('upd-dsh-log');
    log.textContent = 'npm install -g @deepseek-ai/dsh@latest …';
    try {
      const r = await invoke('update_dsh');
      log.innerHTML = (r.ok ? '✅ ' : '⚠ ') + (r.message || '更新完成');
    } catch (e) { log.innerHTML = '失败: ' + e; }
  },

  async checkAppUpdate() {
    const info = document.getElementById('upd-app-info');
    try {
      const u = await invoke('check_app_update');
      if (!u.configured) { info.textContent = '当前 v' + u.current + '（未配置更新源）'; return; }
      info.textContent = '当前 v' + u.current + (u.outdated ? ' → 最新 v' + u.latest : '（已最新）');
    } catch (e) { info.textContent = '离线/无法检查'; }
  },

  async pickBg() {
    try {
      const p = await invoke('pick_and_set_bg');
      if (p && p !== 'cancelled') { document.getElementById('bg-status').textContent = '自定义壁纸'; }
    } catch (e) {}
  },

  async resetBg() {
    try { await invoke('reset_bg'); document.getElementById('bg-status').textContent = '默认壁纸'; } catch (e) {}
  },

  openLogs() { invoke('open_logs').catch(() => {}); },

  async save() {
    const mode = document.querySelector('input[name="close-mode"]:checked');
    if (mode) { try { await invoke('set_close_default', { value: mode.value }); } catch (e) {} }
    const mirror = document.querySelector('input[name="mirror"]:checked');
    if (mirror) {
      let val = mirror.value;
      if (val === 'custom') { const url = document.getElementById('mirror-custom').value.trim(); if (url) val = 'custom:' + url; }
      if (val) { try { await invoke('set_mirror', { mirror: val }); } catch (e) {} }
    }
    try { await invoke('hide_settings_window'); } catch (e) {}
  },

  async close() {
    try { await invoke('hide_settings_window'); } catch (e) {}
  },
};

window.SK = SK;
window.addEventListener('DOMContentLoaded', () => SK.init());