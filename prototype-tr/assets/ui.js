/* Squirl · 复用组件（纯字符串渲染 + 轻量交互工具） */
(function () {
  var M = window.MOCK;
  var Sq = window.Sq = {};

  function esc(s) {
    return String(s == null ? '' : s).replace(/[&<>"]/g, function (c) {
      return ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c];
    });
  }
  Sq.esc = esc;

  var STATE = {
    running: { c: 'run', t: '运行中' },
    starting: { c: 'starting', t: '启动中' },
    exited: { c: 'ok', t: '已退出' },
    failed: { c: 'bad', t: '失败' },
    killed: { c: 'warn', t: '已停止' },
    idle: { c: '', t: '未运行' },
    missing: { c: 'bad', t: '路径失效' }
  };

  Sq.state = function (s) { return STATE[s] || STATE.idle; };
  Sq.statusPill = function (s) {
    var st = Sq.state(s);
    return '<span class="pill ' + st.c + '"><i></i>' + st.t + '</span>';
  };

  /* 项目卡片 */
  Sq.projectCard = function (p) {
    var live = p.state === 'running' || p.state === 'starting';
    var cls = 'pcard' + (live ? ' running' : '') + (p.missing ? ' missing' : '');
    var cmds = p.commands.slice(0, 2).map(function (c) {
      return '<span class="chip" data-cmd="' + c.id + '">' + icon('play') + '<span>' + esc(c.name) + '</span></span>';
    }).join('');
    if (p.commands.length > 2) cmds += '<span class="tag">+' + (p.commands.length - 2) + '</span>';
    if (!p.commands.length) cmds = '<span class="chip add" data-cmd="new">' + icon('plus') + '<span>加一条命令</span></span>';

    return '<article class="' + cls + '" data-id="' + p.id + '" tabindex="0" role="button">' +
      '<div class="ph">' +
        '<div class="avatar">' + esc(p.badge) + '</div>' +
        '<div style="min-width:0;flex:1">' +
          '<div class="nm">' + esc(p.name) + '</div>' +
          '<div class="pth" title="' + esc(p.path) + '">' + esc(p.path) + '</div>' +
        '</div>' +
        '<div class="kt">' + Sq.statusPill(p.missing ? 'missing' : p.state) + '</div>' +
      '</div>' +
      '<div class="note' + (p.note ? '' : ' nil') + '">' + (p.note ? esc(p.note) : '还没有备注。记下它为什么存在，下个月你会感谢自己。') + '</div>' +
      '<div class="cf">' + cmds +
        '<div class="rt"><span>' + (p.lastRun || '未运行') + '</span><span>·</span><span>' + p.runs + ' 次</span></div>' +
      '</div>' +
    '</article>';
  };

  /* 列表行 */
  Sq.listRow = function (p) {
    var cmds = p.commands.slice(0, 2).map(function (c) {
      return '<span class="chip" data-cmd="' + c.id + '">' + icon('play') + '<span>' + esc(c.name) + '</span></span>';
    }).join('');
    if (p.commands.length > 2) cmds += '<span class="tag">+' + (p.commands.length - 2) + '</span>';
    return '<div class="lrow" data-id="' + p.id + '" tabindex="0" role="button">' +
      '<div class="avatar">' + esc(p.badge) + '</div>' +
      '<div class="nm">' + esc(p.name) + '</div>' +
      '<div class="pth" title="' + esc(p.path) + '">' + esc(p.path) + '</div>' +
      '<div class="cmds">' + (cmds || '<span class="tag">无命令</span>') + '</div>' +
      '<div class="st">' + Sq.statusPill(p.missing ? 'missing' : p.state) + '</div>' +
    '</div>';
  };

  /* 本次发现行 */
  Sq.discoveryRow = function (d) {
    var tags = d.flags.length
      ? d.flags.map(function (f) { return '<span class="tag">' + esc(f) + '</span>'; }).join('')
      : '<span class="tag">无标志文件</span>';
    return '<div class="drow" data-id="' + d.id + '">' +
      '<span class="rel" title="' + esc(d.root + '/' + d.name) + '">' + esc(d.rel) + '</span>' +
      '<div class="tags">' + tags + '</div>' +
      '<button class="btn btn-sm reg" data-reg="' + d.id + '">登记</button>' +
    '</div>';
  };

  /* 日志行 */
  Sq.logLines = function (lines, startAt) {
    var n = startAt || 1;
    return lines.map(function (l) {
      return '<div class="lp-line"><span class="sn">' + (n++) + '</span><span class="tx ' + (l.c || '') + '">' + esc(l.t) + '</span></div>';
    }).join('');
  };

  /* Toast */
  Sq.toast = function (msg, kind, sub) {
    var host = document.getElementById('toastHost');
    if (!host) {
      host = document.createElement('div');
      host.className = 'toast-host';
      host.id = 'toastHost';
      document.body.appendChild(host);
    }
    var ic = kind === 'bad' ? 'alert' : kind === 'ok' ? 'check' : 'info';
    var el = document.createElement('div');
    el.className = 'toast ' + (kind || 'info');
    el.setAttribute('role', 'status');
    el.innerHTML = icon(ic) + '<div class="tm">' + msg + (sub ? '<small>' + sub + '</small>' : '') + '</div>';
    host.appendChild(el);
    setTimeout(function () {
      el.style.transition = 'opacity .22s, transform .22s';
      el.style.opacity = '0';
      el.style.transform = 'translateY(4px)';
      setTimeout(function () { el.remove(); }, 240);
    }, 2800);
    return el;
  };

  /* 对话框 */
  var escHandler = null;
  Sq.closeDialog = function () {
    var s = document.getElementById('sqScrim');
    if (s) s.remove();
    if (escHandler) { document.removeEventListener('keydown', escHandler); escHandler = null; }
  };
  Sq.dialog = function (o) {
    o = o || {};
    Sq.closeDialog();
    var scrim = document.createElement('div');
    scrim.className = 'scrim';
    scrim.id = 'sqScrim';
    scrim.innerHTML = '<div class="dialog" role="dialog" aria-modal="true" aria-label="' + esc(o.title || '对话框') + '">' +
      '<div class="dialog-head"><h3>' + (o.title || '确认') + '</h3>' + (o.desc ? '<p>' + o.desc + '</p>' : '') + '</div>' +
      (o.body ? '<div class="dialog-body">' + o.body + '</div>' : '') +
      '<div class="dialog-foot">' +
        (o.left ? '<div class="left">' + o.left + '</div>' : '') +
        '<button class="btn" data-close>' + (o.cancel || '取消') + '</button>' +
        (o.ok === false ? '' : '<button class="btn ' + (o.danger ? 'btn-danger' : 'btn-primary') + '" data-ok>' + (o.ok || '确定') + '</button>') +
      '</div>' +
    '</div>';
    document.body.appendChild(scrim);

    scrim.addEventListener('click', function (e) {
      if (e.target === scrim || e.target.closest('[data-close]')) Sq.closeDialog();
    });
    var okBtn = scrim.querySelector('[data-ok]');
    if (okBtn) okBtn.addEventListener('click', function () {
      if (o.onOk) o.onOk(scrim);
      Sq.closeDialog();
    });
    escHandler = function (e) { if (e.key === 'Escape') Sq.closeDialog(); };
    document.addEventListener('keydown', escHandler);
    setTimeout(function () { (okBtn || scrim.querySelector('[data-close]')).focus(); }, 40);
    return scrim;
  };
  Sq.confirm = function (o) { return Sq.dialog(o); };

  /* 交互工具
     注意：调用方常把 .tgl / .seg 元素本身作为 root 传进来（querySelectorAll 不含自身），
     所以这里要把 root 自身也纳入匹配，否则控件会静默失去响应。 */
  function collect(scope, sel) {
    var list = Array.prototype.slice.call(scope.querySelectorAll(sel));
    if (scope.classList && scope.classList.contains(sel.slice(1))) list.unshift(scope);
    return list;
  }

  Sq.bindTgls = function (root) {
    collect(root || document, '.tgl').forEach(function (t) {
      if (t.dataset.bound || t.dataset.manual) return;
      t.dataset.bound = '1';
      t.setAttribute('role', 'switch');
      t.setAttribute('tabindex', '0');
      t.setAttribute('aria-checked', t.classList.contains('on') ? 'true' : 'false');
      function flip() {
        t.classList.toggle('on');
        t.setAttribute('aria-checked', t.classList.contains('on') ? 'true' : 'false');
      }
      t.addEventListener('click', flip);
      t.addEventListener('keydown', function (e) {
        if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); flip(); }
      });
    });
  };
  Sq.bindSegs = function (root) {
    collect(root || document, '.seg').forEach(function (g) {
      if (g.dataset.bound) return;
      g.dataset.bound = '1';
      g.querySelectorAll('button').forEach(function (b) {
        b.addEventListener('click', function () {
          g.querySelectorAll('button').forEach(function (x) { x.classList.remove('on'); });
          b.classList.add('on');
          g.dispatchEvent(new CustomEvent('segchange', { detail: { value: b.dataset.value || b.textContent.trim() } }));
        });
      });
    });
  };
  Sq.q = function (sel, root) { return (root || document).querySelector(sel); };
  Sq.qa = function (sel, root) { return Array.prototype.slice.call((root || document).querySelectorAll(sel)); };
  Sq.debounce = function (fn, ms) {
    var t;
    return function () {
      var a = arguments, self = this;
      clearTimeout(t);
      t = setTimeout(function () { fn.apply(self, a); }, ms || 150);
    };
  };
  Sq.bind = function (el, ev, fn) { if (el) el.addEventListener(ev, fn); };

  /* 页面间跳转 */
  Sq.goProject = function (id) { location.href = '02-project.html?id=' + encodeURIComponent(id || 'p1'); };
})();
