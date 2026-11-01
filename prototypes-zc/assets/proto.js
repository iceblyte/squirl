/* Squirl · prototype interactions (vanilla, event-delegated)
   用法：
   [data-scene="key"]          场景切换按钮（.pill）；.scene[data-key] 为场景容器
   [data-open="dialogId"]      打开浮层（.overlay#id）
   [data-close]                关闭所在浮层（含点击遮罩空白处）
   [data-tab="panelId"]        页内标签按钮；需带 data-tab-group，面板 .set-panel#panelId
   [data-collapse="#target"]   折叠目标元素（切换 .collapsed）
   [data-toggle]               纯视觉开关（.toggle）
   [data-dropzone]             拖拽区悬停高亮演示
*/
(function () {
  function hideOverlay(o) { o.classList.remove('open'); o.style.display = 'none'; }

  function activate(btn, attr, cls) {
    var group = btn.closest('[data-' + attr + '-group]') || btn.parentElement;
    group.querySelectorAll('[' + attr + ']').forEach(function (b) { b.classList.remove(cls); });
    btn.classList.add(cls);
  }

  document.addEventListener('click', function (e) {
    var el;

    // 场景切换
    el = e.target.closest('[data-scene]');
    if (el) {
      var key = el.getAttribute('data-scene');
      document.querySelectorAll('.scene').forEach(function (s) {
        s.classList.toggle('act', s.getAttribute('data-key') === key);
      });
      var bar = el.closest('.scene-bar');
      if (bar) bar.querySelectorAll('[data-scene]').forEach(function (b) { b.classList.toggle('act', b === el); });
      window.scrollTo({ top: 0, behavior: 'smooth' });
      return;
    }

    // 打开浮层
    el = e.target.closest('[data-open]');
    if (el) {
      var ov = document.getElementById(el.getAttribute('data-open'));
      if (ov) { ov.classList.add('open'); ov.style.display = 'flex'; }
      return;
    }

    // 关闭浮层（按钮或遮罩空白）
    if (e.target.classList && e.target.classList.contains('overlay')) { hideOverlay(e.target); return; }
    if (e.target.classList && e.target.classList.contains('win-overlay')) { hideOverlay(e.target); return; }
    el = e.target.closest('[data-close]');
    if (el) {
      var o2 = el.closest('.overlay') || el.closest('.win-overlay');
      if (o2) hideOverlay(o2);
      return;
    }

    // 页内标签
    el = e.target.closest('[data-tab]');
    if (el) {
      activate(el, 'data-tab', 'act');
      document.querySelectorAll('.set-panel').forEach(function (p) {
        p.classList.toggle('act', p.id === el.getAttribute('data-tab'));
      });
      return;
    }

    // 折叠
    el = e.target.closest('[data-collapse]');
    if (el) {
      var t = document.querySelector(el.getAttribute('data-collapse'));
      if (t) t.classList.toggle('collapsed');
      return;
    }

    // 视觉开关
    el = e.target.closest('[data-toggle]');
    if (el) { el.classList.toggle('on'); return; }
  });

  // 拖拽区演示
  document.querySelectorAll('[data-dropzone]').forEach(function (z) {
    ['dragenter', 'dragover'].forEach(function (ev) {
      z.addEventListener(ev, function (e) { e.preventDefault(); z.classList.add('hot'); });
    });
    ['dragleave', 'drop'].forEach(function (ev) {
      z.addEventListener(ev, function (e) { e.preventDefault(); z.classList.remove('hot'); });
    });
  });

  // Esc 关闭全部浮层
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape') {
      document.querySelectorAll('.overlay.open, .win-overlay.open').forEach(hideOverlay);
    }
  });
})();
