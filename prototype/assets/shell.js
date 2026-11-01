/* Squirl · 统一外壳（侧栏 + 顶栏 + 主题 + 搜索快捷键 + 移动端抽屉） */
(function () {
  var M = window.MOCK;
  var H = window.SqShell = {};
  var STORE_KEY = 'sq.theme';

  var NAV = [
    { id: 'ledger', href: '01-home.html', icon: 'ledger', label: '项目台账', count: function () { return M.projects.length; } },
    { id: 'project', href: '02-project.html', icon: 'folder', label: '项目详情' },
    { id: 'runs', href: '03-run-states.html', icon: 'terminal', label: '运行状态', count: function () { return activeRuns().length; } },
    { id: 'settings', href: '04-settings.html', icon: 'settings', label: '设置' },
    { id: 'flows', href: '05-flows.html', icon: 'wf', label: '关键流程' },
    { id: 'extended', href: '06-extended.html', icon: 'layers', label: '扩展能力' },
    { id: 'index', href: '00-index.html', icon: 'tray', label: '原型导览' }
  ];

  function activeRuns() {
    return M.runs.filter(function (r) { return r.state === 'running' || r.state === 'starting'; });
  }
  function esc(s) {
    return String(s == null ? '' : s).replace(/[&<>"]/g, function (c) {
      return ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c];
    });
  }

  /* ---------- 主题 ---------- */
  function currentTheme() {
    try {
      var v = localStorage.getItem(STORE_KEY);
      if (v === 'light' || v === 'dark') return v;
    } catch (e) { /* file:// 下 localStorage 可能被禁用 */ }
    return (M.settings && M.settings.theme) || 'light';
  }
  function applyTheme(t, persist) {
    document.documentElement.setAttribute('data-theme', t);
    if (M.settings) M.settings.theme = t;
    if (persist !== false) {
      try { localStorage.setItem(STORE_KEY, t); } catch (e) { }
    }
    var tgl = document.getElementById('themeTgl');
    if (tgl) {
      tgl.classList.toggle('on', t === 'dark');
      tgl.setAttribute('aria-checked', t === 'dark' ? 'true' : 'false');
    }
    var lbl = document.getElementById('themeLabel');
    if (lbl) lbl.textContent = t === 'dark' ? '浅色模式' : '深色模式';
    var ico = document.getElementById('themeIco');
    if (ico) ico.innerHTML = icon(t === 'dark' ? 'sun' : 'moon');
    var btn = document.getElementById('themeBtn');
    if (btn) {
      btn.classList.toggle('on', t === 'dark');
      btn.textContent = t === 'dark' ? '深色' : '浅色';
    }
  }
  H.theme = function () { return currentTheme(); };
  H.setTheme = function (t) { applyTheme(t, true); };

  // 首帧前生效，避免明暗闪烁
  document.documentElement.setAttribute('data-theme', currentTheme());

  /* ---------- 侧栏 ---------- */
  function sidebarHtml(opts) {
    var nav = NAV.map(function (n) {
      var on = n.id === opts.active ? ' on' : '';
      var count = n.count ? n.count() : '';
      return '<a class="' + on.trim() + '" href="' + n.href + '"' + (n.id === opts.active ? ' aria-current="page"' : '') + '>' +
        icon(n.icon) + '<span>' + n.label + '</span>' +
        (count ? '<span class="n">' + count + '</span>' : '') + '</a>';
    }).join('');

    var roots = M.roots.map(function (r) {
      return '<div class="sb-root ' + (r.enabled ? '' : 'off') + '" data-goto="04-settings.html" title="' + esc(r.path) + '">' +
        '<span class="dot ' + (r.enabled ? 'live' : '') + '"></span>' +
        '<span class="rt">' + esc(r.path) + '</span>' +
        '<span class="ct">' + r.count + '</span></div>';
    }).join('');

    var sessions = activeRuns().map(function (r) {
      return '<div class="sb-root" data-goto="03-run-states.html" title="run ' + esc(r.rid) + '">' +
        '<span class="dot ' + (r.state === 'running' ? 'live' : '') + '"></span>' +
        '<span class="rt">' + esc(r.project) + ' · ' + esc(r.cmd) + '</span>' +
        '<span class="ct">' + esc(r.elapsed) + '</span></div>';
    }).join('');

    return '<div class="sb-brand">' +
        '<div class="sb-logo"><img src="assets/logo-128.png" alt="Squirl" width="29" height="29"></div>' +
        '<div><span class="sb-name">Squirl</span><span class="sb-sub">本地项目台账</span></div>' +
      '</div>' +
      '<nav class="nav" aria-label="主导航">' + nav + '</nav>' +
      '<div class="sb-sec">根目录<span class="sp"></span><span class="mini" data-goto="04-settings.html">管理</span></div>' +
      roots +
      '<div class="sb-sec">活动会话<span class="sp"></span><span class="mini" data-goto="03-run-states.html">全部</span></div>' +
      (sessions || '<div class="sb-root"><span class="dot"></span><span class="rt">暂无运行中的会话</span></div>') +
      '<div class="sb-foot">' +
        '<div class="sb-row" id="themeRow" role="button" tabindex="0">' +
          '<span class="sb-ico" id="themeIco">' + icon(currentTheme() === 'dark' ? 'sun' : 'moon') + '</span>' +
          '<span class="sb-lbl" id="themeLabel">' + (currentTheme() === 'dark' ? '浅色模式' : '深色模式') + '</span>' +
          '<span class="sp"></span><span class="tgl" id="themeTgl"></span>' +
        '</div>' +
        '<div class="sb-row" id="paletteRow">' + icon('keyboard') + '<span class="sb-lbl">搜索与命令面板</span><span class="sp">Ctrl K</span></div>' +
        '<div class="sb-row" data-goto="04-settings.html">' + icon('database') + '<span class="sb-lbl">数据目录</span><span class="sp">12.4 MB</span></div>' +
        '<div class="sb-ver">v0.1.0 · 原型 · 2026-11-01</div>' +
      '</div>';
  }

  /* ---------- 顶栏 ---------- */
  function topbarHtml(opts) {
    var crumb = (opts.crumb || []).map(function (c, i, arr) {
      if (i === arr.length - 1 && !c.href) return '<span class="cur">' + c.label + '</span>';
      return '<a href="' + (c.href || '01-home.html') + '">' + c.label + '</a>' + icon('chevronRight');
    }).join('');

    return '<button class="iconbtn tb-toggle" id="sbToggle" aria-label="打开导航">' + icon('list') + '</button>' +
      '<div class="tb-crumb">' + crumb + '</div>' +
      '<div class="tb-sp"></div>' +
      '<div class="search">' + icon('search') +
        '<input id="sqSearch" type="search" autocomplete="off" placeholder="' + esc(opts.search || '搜索项目、路径、备注、命令') + '" aria-label="搜索">' +
        '<span class="kbd">Ctrl K</span>' +
      '</div>' +
      '<div class="tb-actions">' + (opts.actions || '') + '</div>';
  }

  /* ---------- 挂载 ---------- */
  H.mount = function (opts) {
    opts = opts || {};
    var side = document.getElementById('sidebar');
    var top = document.getElementById('topbar');
    if (side) side.innerHTML = sidebarHtml(opts);
    if (top) top.innerHTML = topbarHtml(opts);

    // 主题开关
    var row = document.getElementById('themeRow');
    function flipTheme() { applyTheme(currentTheme() === 'dark' ? 'light' : 'dark', true); }
    if (row) {
      row.addEventListener('click', flipTheme);
      row.addEventListener('keydown', function (e) {
        if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); flipTheme(); }
      });
    }
    applyTheme(currentTheme(), false);

    // 跳转
    document.querySelectorAll('[data-goto]').forEach(function (el) {
      el.addEventListener('click', function () { location.href = el.dataset.goto; });
    });

    // 移动端抽屉
    var veil = document.createElement('div');
    veil.className = 'sb-veil';
    veil.id = 'sbVeil';
    document.body.appendChild(veil);
    var toggle = document.getElementById('sbToggle');
    function openDrawer(open) {
      if (!side) return;
      side.classList.toggle('open', open);
      veil.classList.toggle('on', open);
    }
    if (toggle) toggle.addEventListener('click', function () { openDrawer(!side.classList.contains('open')); });
    veil.addEventListener('click', function () { openDrawer(false); });
    if (side) side.querySelectorAll('.nav a').forEach(function (a) {
      a.addEventListener('click', function () { openDrawer(false); });
    });

    // 搜索
    var input = document.getElementById('sqSearch');
    if (input) {
      if (opts.query) input.value = opts.query;
      var run = Sq.debounce(function () {
        if (opts.onSearch) opts.onSearch(input.value.trim());
      }, 150);
      input.addEventListener('input', run);
      input.addEventListener('keydown', function (e) {
        if (e.key === 'Enter' && !opts.onSearch) {
          location.href = '01-home.html?q=' + encodeURIComponent(input.value.trim());
        }
        if (e.key === 'Escape') { input.value = ''; input.blur(); if (opts.onSearch) opts.onSearch(''); }
      });
      document.addEventListener('keydown', function (e) {
        var k = e.key.toLowerCase();
        if ((e.ctrlKey || e.metaKey) && k === 'k') { e.preventDefault(); input.focus(); input.select(); }
      });
      var pRow = document.getElementById('paletteRow');
      if (pRow) pRow.addEventListener('click', function () { input.focus(); input.select(); });
    }

    // 命令面板（示例入口：聚焦搜索并提示）
    var host = document.createElement('div');
    host.className = 'toast-host';
    host.id = 'toastHost';
    document.body.appendChild(host);

    return H;
  };

  /* 供页面查询当前搜索词 */
  H.query = function () {
    var i = document.getElementById('sqSearch');
    return i ? i.value.trim() : '';
  };
  H.activeRuns = activeRuns;
})();
