/* Squirl · 内联 SVG 图标集（无图标库依赖） */
(function () {
  var P = {
    logo: '<path d="M12 21.2c-2.9-1.7-4.5-4.4-4.5-7.4 0-2 .6-3.9 1.5-5.3h6c.9 1.4 1.5 3.3 1.5 5.3 0 3-1.6 5.7-4.5 7.4Z" stroke="currentColor" stroke-width="1.4"/><path d="M8 8.6c1.4-.9 2.6-1.3 4-1.3s2.6.4 4 1.3" stroke="currentColor" stroke-width="1.3"/><path d="M12 7.3V5.2M12 5.2h2" stroke="currentColor" stroke-width="1.3"/>',
    ledger: '<path d="M4.5 6h15M4.5 12h15M4.5 18h9" stroke="currentColor" stroke-width="1.5"/><circle cx="18.5" cy="18" r="2" stroke="currentColor" stroke-width="1.4"/>',
    terminal: '<rect x="3" y="4.5" width="18" height="15" rx="2.6" stroke="currentColor" stroke-width="1.4"/><path d="m7 9.6 2.4 2.4L7 14.4M12.6 15h4.2" stroke="currentColor" stroke-width="1.4"/>',
    settings: '<circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.4"/><path d="M12 3.6v2.1M12 18.3v2.1M4.6 12h2.1M17.3 12h2.1M6.5 6.5l1.5 1.5M16 16l1.5 1.5M17.5 6.5 16 8M8 16l-1.5 1.5" stroke="currentColor" stroke-width="1.4"/>',
    search: '<circle cx="11" cy="11" r="6.2" stroke="currentColor" stroke-width="1.5"/><path d="m15.6 15.6 4.4 4.4" stroke="currentColor" stroke-width="1.5"/>',
    plus: '<path d="M12 5.5v13M5.5 12h13" stroke="currentColor" stroke-width="1.6"/>',
    folder: '<path d="M3.5 7.4h6l2 2h9v8.1a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1Zm0 0V5.6a1 1 0 0 1 1-1h5.1" stroke="currentColor" stroke-width="1.4"/>',
    folderOpen: '<path d="M3.5 8h6l2 2h9.2v6.6a1 1 0 0 1-1 1H4.5a1 1 0 0 1-1-1Zm0 0V6a1 1 0 0 1 1-1h5" stroke="currentColor" stroke-width="1.4"/>',
    refresh: '<path d="M19.5 11.2a7.6 7.6 0 1 0-2.1 5.4" stroke="currentColor" stroke-width="1.5"/><path d="M19.6 5.6v5.6h-5.6" stroke="currentColor" stroke-width="1.5"/>',
    play: '<path d="M8.2 5.6v12.8l10.6-6.4Z" stroke="currentColor" stroke-width="1.5"/>',
    stop: '<rect x="6.8" y="6.8" width="10.4" height="10.4" rx="1.6" stroke="currentColor" stroke-width="1.5"/>',
    edit: '<path d="m4.8 19.2 1-4.2 9.9-9.9 3.2 3.2-9.9 9.9-4.2 1Z" stroke="currentColor" stroke-width="1.4"/><path d="m12.8 7.2 3.2 3.2" stroke="currentColor" stroke-width="1.4"/>',
    trash: '<path d="M4.6 7h14.8M9.4 7V4.8h5.2V7M6.6 7l.9 12.2h10.2L18.6 7" stroke="currentColor" stroke-width="1.4"/><path d="M10 10.6v5.4M14 10.6v5.4" stroke="currentColor" stroke-width="1.3"/>',
    chevronDown: '<path d="m6.5 9.5 5.5 5.5 5.5-5.5" stroke="currentColor" stroke-width="1.5"/>',
    chevronLeft: '<path d="m14.5 6.5-5.5 5.5 5.5 5.5" stroke="currentColor" stroke-width="1.5"/>',
    chevronRight: '<path d="m9.5 6.5 5.5 5.5-5.5 5.5" stroke="currentColor" stroke-width="1.5"/>',
    arrowRight: '<path d="M4.5 12h14M13.5 6.5 19 12l-5.5 5.5" stroke="currentColor" stroke-width="1.5"/>',
    external: '<path d="M13.5 4.5H19.5v6M19 5l-7.2 7.2" stroke="currentColor" stroke-width="1.4"/><path d="M17.4 13.4v5.1a1 1 0 0 1-1 1H5.4a1 1 0 0 1-1-1V7.5a1 1 0 0 1 1-1h5.1" stroke="currentColor" stroke-width="1.4"/>',
    x: '<path d="m6.5 6.5 11 11M17.5 6.5 6.5 17.5" stroke="currentColor" stroke-width="1.5"/>',
    check: '<path d="m5 12.5 4.6 4.6L19 7.3" stroke="currentColor" stroke-width="1.6"/>',
    alert: '<path d="M12 4.3 20.4 19H3.6L12 4.3Z" stroke="currentColor" stroke-width="1.4"/><path d="M12 9.6v4.6M12 16.6v.1" stroke="currentColor" stroke-width="1.5"/>',
    dots: '<circle cx="6" cy="12" r="1.2" fill="currentColor"/><circle cx="12" cy="12" r="1.2" fill="currentColor"/><circle cx="18" cy="12" r="1.2" fill="currentColor"/>',
    grid: '<rect x="4" y="4" width="6.6" height="6.6" rx="1.2" stroke="currentColor" stroke-width="1.4"/><rect x="13.4" y="4" width="6.6" height="6.6" rx="1.2" stroke="currentColor" stroke-width="1.4"/><rect x="4" y="13.4" width="6.6" height="6.6" rx="1.2" stroke="currentColor" stroke-width="1.4"/><rect x="13.4" y="13.4" width="6.6" height="6.6" rx="1.2" stroke="currentColor" stroke-width="1.4"/>',
    list: '<path d="M4.5 7h15M4.5 12h15M4.5 17h15" stroke="currentColor" stroke-width="1.5"/>',
    sun: '<circle cx="12" cy="12" r="4" stroke="currentColor" stroke-width="1.4"/><path d="M12 3.6v2M12 18.4v2M4.8 12h-1.2M20.4 12h-1.2M6.9 6.9 6 6M18 18l-.9-.9M17.1 6.9 18 6M6 18l.9-.9" stroke="currentColor" stroke-width="1.4"/>',
    moon: '<path d="M15.4 4.8a7.6 7.6 0 1 0 4.8 10.6 6.4 6.4 0 0 1-4.8-10.6Z" stroke="currentColor" stroke-width="1.4"/>',
    clock: '<circle cx="12" cy="12" r="7.4" stroke="currentColor" stroke-width="1.4"/><path d="M12 8v4.4l3 1.8" stroke="currentColor" stroke-width="1.4"/>',
    filter: '<path d="M4.5 6.5h15L14 13v5.4l-4 2v-7.4Z" stroke="currentColor" stroke-width="1.4"/>',
    panel: '<rect x="3.6" y="4.6" width="16.8" height="14.8" rx="2.2" stroke="currentColor" stroke-width="1.4"/><path d="M9.2 4.6v14.8" stroke="currentColor" stroke-width="1.4"/>',
    tray: '<rect x="3.6" y="12.6" width="16.8" height="7" rx="1.6" stroke="currentColor" stroke-width="1.4"/><path d="M12 4v7M8.8 7.6 12 10.8l3.2-3.2" stroke="currentColor" stroke-width="1.4"/>',
    layers: '<path d="m12 4.4 8.2 4.2-8.2 4.2-8.2-4.2Z" stroke="currentColor" stroke-width="1.4"/><path d="m4.6 13.2 7.4 3.8 7.4-3.8M4.6 17l7.4 3.8 7.4-3.8" stroke="currentColor" stroke-width="1.3"/>',
    split: '<rect x="3.6" y="4.6" width="16.8" height="14.8" rx="2.2" stroke="currentColor" stroke-width="1.4"/><path d="M12 4.6v14.8" stroke="currentColor" stroke-width="1.4"/>',
    radar: '<circle cx="12" cy="12" r="7.6" stroke="currentColor" stroke-width="1.4"/><circle cx="12" cy="12" r="3.6" stroke="currentColor" stroke-width="1.3"/><path d="M12 12V4.4" stroke="currentColor" stroke-width="1.3"/>',
    shield: '<path d="M12 4.2 19 6.6v5.2c0 4.2-2.8 7-7 8.4-4.2-1.4-7-4.2-7-8.4v-5.2Z" stroke="currentColor" stroke-width="1.4"/><path d="m9 11.7 2 2 4-4" stroke="currentColor" stroke-width="1.3"/>',
    code: '<path d="m8.6 7.6-4 4.4 4 4.4M15.4 7.6l4 4.4-4 4.4M13 5.6l-2 12.8" stroke="currentColor" stroke-width="1.4"/>',
    git: '<circle cx="6.6" cy="6.4" r="2.2" stroke="currentColor" stroke-width="1.4"/><circle cx="17.4" cy="8.4" r="2.2" stroke="currentColor" stroke-width="1.4"/><circle cx="6.6" cy="17.6" r="2.2" stroke="currentColor" stroke-width="1.4"/><path d="M6.6 8.6v6.8M8.8 6.9h5.2a1.4 1.4 0 0 1 1.4 1.4v.1" stroke="currentColor" stroke-width="1.3"/>',
    database: '<ellipse cx="12" cy="6.2" rx="6.6" ry="2.6" stroke="currentColor" stroke-width="1.4"/><path d="M5.4 6.2v11.6c0 1.4 3 2.6 6.6 2.6s6.6-1.2 6.6-2.6V6.2" stroke="currentColor" stroke-width="1.4"/><path d="M5.4 12c0 1.4 3 2.6 6.6 2.6s6.6-1.2 6.6-2.6" stroke="currentColor" stroke-width="1.3"/>',
    keyboard: '<rect x="2.8" y="6.6" width="18.4" height="10.8" rx="2" stroke="currentColor" stroke-width="1.4"/><path d="M7 10.4h.1M10.2 10.4h.1M13.4 10.4h.1M16.6 10.4h.1M8 14h8" stroke="currentColor" stroke-width="1.5"/>',
    info: '<circle cx="12" cy="12" r="7.6" stroke="currentColor" stroke-width="1.4"/><path d="M12 8.2v.1M12 11v5" stroke="currentColor" stroke-width="1.4"/>',
    copy: '<rect x="8.4" y="8.4" width="11" height="11" rx="2" stroke="currentColor" stroke-width="1.4"/><path d="M15.6 8.4V6.4a1 1 0 0 0-1-1H5.6a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h2.8" stroke="currentColor" stroke-width="1.4"/>',
    link: '<path d="M9.6 14.4 14.4 9.6" stroke="currentColor" stroke-width="1.4"/><path d="M12.6 7.4 14 6a3.4 3.4 0 0 1 4.8 4.8l-1.4 1.4M11.4 16.6 10 18a3.4 3.4 0 0 1-4.8-4.8L6.6 11.8" stroke="currentColor" stroke-width="1.4"/>',
    eye: '<path d="M2.8 12s3.4-6 9.2-6 9.2 6 9.2 6-3.4 6-9.2 6S2.8 12 2.8 12Z" stroke="currentColor" stroke-width="1.4"/><circle cx="12" cy="12" r="2.6" stroke="currentColor" stroke-width="1.4"/>',
    tag: '<path d="M4.4 11.6V5.4a1 1 0 0 1 1-1h6.2l7.6 7.6a1.4 1.4 0 0 1 0 2l-4.8 4.8a1.4 1.4 0 0 1-2 0Z" stroke="currentColor" stroke-width="1.4"/><circle cx="8.4" cy="8.4" r="1.3" stroke="currentColor" stroke-width="1.3"/>',
    wf: '<path d="M4 12h16M4 6h16M4 18h10" stroke="currentColor" stroke-width="1.4"/>'
  };

  window.icon = function (name, cls) {
    var inner = P[name] || P.info;
    return '<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"' + (cls ? ' class="' + cls + '"' : '') + '>' + inner + '</svg>';
  };
  window.ICONS = P;
})();
