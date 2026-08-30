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
      const doBtn = document.getElementById('btn-upd-dsh-do');
      if (u.outdated) {
        log.textContent = '发现新版本！';
        if (doBtn) doBtn.classList.remove('hidden');
      } else {
        log.textContent = '✅ 已是最新版本';
        if (doBtn) doBtn.classList.add('hidden');
      }
    } catch (e) { info.textContent = '离线/无法检查'; log.textContent = ''; }
  },

  async doUpdate() {
    const log = document.getElementById('upd-dsh-log');
    log.textContent = 'npm install -g @deepseek-ai/dsh@latest …';
    try {
      const r = await invoke('update_dsh');
      log.textContent = (r.ok ? '✅ ' : '⚠ ') + (r.message || '更新完成');
    } catch (e) { log.textContent = '失败: ' + e; }
  },

  async checkAppUpdate() {
    const info = document.getElementById('upd-app-info');
    const dl = document.getElementById('btn-upd-app-dl');
    if (dl) dl.classList.add('hidden');
    try {
      // WebView fetch 自动走系统代理（与浏览器一致）
      const resp = await fetch('https://api.github.com/repos/broken-star1999/deepseek-harness-up/releases/latest', {
        headers: { 'Accept': 'application/vnd.github+json' }
      });
      if (!resp.ok) throw new Error('HTTP ' + resp.status);
      const j = await resp.json();
      const latest = (j.tag_name || '').replace(/^v/, '');
      const url = j.html_url || 'https://github.com/broken-star1999/deepseek-harness-up/releases/latest';
      let current = '';
      try { const s = await invoke('get_status'); current = s.appVersion || ''; } catch (e) {}
      if (latest && current && latest !== current) {
        info.textContent = '当前 v' + current + ' → 最新 v' + latest;
        if (dl) { this._appUrl = url; dl.classList.remove('hidden'); }
      } else if (latest) {
        info.textContent = '当前 v' + latest + '（已最新）';
      } else {
        info.textContent = '无法解析版本信息';
        if (dl) { this._appUrl = url; dl.classList.remove('hidden'); }
      }
    } catch (e) {
      info.textContent = '无法检查（离线或被拦截）';
      if (dl) { this._appUrl = 'https://github.com/broken-star1999/deepseek-harness-up/releases/latest'; dl.classList.remove('hidden'); }
    }
  },

  async downloadApp() {
    try { await invoke('open_external', { url: this._appUrl }); } catch (e) {}
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