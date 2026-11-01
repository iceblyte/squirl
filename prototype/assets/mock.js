/* Squirl · 原型假数据（仅会话内存在，不落库） */
(function () {
  var db = {
    settings: {
      theme: 'light',
      defaultMode: 'external',
      emulator: 'Windows Terminal',
      emulatorPath: 'C:\\Users\\ICE\\AppData\\Local\\Microsoft\\WindowsApps\\wt.exe',
      shell: 'pwsh 7+',
      shellPath: 'C:\\Program Files\\PowerShell\\7\\pwsh.exe',
      profile: true,
      scanDepth: 1,
      autoScan: false,
      dataDir: '%APPDATA%\\squirl',
      lastScanAt: '2 分钟前',
      lastScanCount: 37,
      lastScanMs: 412
    },

    roots: [
      { id: 'r1', path: 'D:\\GitHub', enabled: true, count: 9 },
      { id: 'r2', path: 'D:\\Projects', enabled: true, count: 5 },
      { id: 'r3', path: 'C:\\Users\\ICE\\Documents', enabled: false, count: 0 }
    ],

    projects: [
      {
        id: 'p1', name: 'squirl', path: 'D:\\Code\\AICreate\\ZCode\\squirl', rootId: 'r2',
        note: '本地项目台账，Tauri 2 + React。M0 先跑通外部终端转义与进程树回收，再动 UI。',
        mode: 'builtin', state: 'running', lastRun: '2 分钟前', runs: 84,
        createdAt: '2026-10-12',
        commands: [
          { id: 'c1', name: 'dev', line: 'pnpm dev', cwd: null, lastRun: '2 分钟前', exit: null, state: 'running' },
          { id: 'c2', name: 'tauri:dev', line: 'pnpm tauri dev', cwd: null, lastRun: '昨天 18:40', exit: 0, state: 'exited' },
          { id: 'c3', name: 'build', line: 'pnpm build && pnpm tauri build', cwd: null, lastRun: '3 天前', exit: null, state: 'idle' }
        ]
      },
      {
        id: 'p2', name: 'owl-runner', path: 'D:\\Projects\\owl-runner', rootId: 'r2',
        note: '内部调度服务。跑之前要先激活 .venv，uv 会自动处理，别手动 pip install。',
        mode: null, state: 'starting', lastRun: '刚刚', runs: 41,
        createdAt: '2026-09-28',
        commands: [
          { id: 'c4', name: 'api', line: 'uv run uvicorn app.main:app --reload --port 8010', cwd: null, lastRun: '刚刚', exit: null, state: 'starting' },
          { id: 'c5', name: 'test', line: 'pytest -q', cwd: null, lastRun: '昨天 09:12', exit: 0, state: 'exited' }
        ]
      },
      {
        id: 'p3', name: 'magpie-fork', path: 'D:\\GitHub\\magpie', rootId: 'r1',
        note: '读源码用的克隆：重点看菜单栏面板的行结构与模型选择器。',
        mode: null, state: 'idle', lastRun: '3 天前', runs: 6,
        createdAt: '2026-10-21',
        commands: [
          { id: 'c6', name: 'site', line: 'make site', cwd: null, lastRun: '3 天前', exit: 0, state: 'exited' },
          { id: 'c7', name: 'tui', line: 'go run . --tui', cwd: null, lastRun: '5 天前', exit: 0, state: 'exited' }
        ]
      },
      {
        id: 'p4', name: 'elegant-clipboard', path: 'D:\\GitHub\\ElegantClipboard', rootId: 'r1',
        note: '剪贴板工具的视觉参考。本机缺 C++ Build Tools，构建必失败，只跑 dev。',
        mode: 'builtin', state: 'failed', lastRun: '昨天 21:04', runs: 12,
        createdAt: '2026-10-19',
        commands: [
          { id: 'c8', name: 'dev', line: 'pnpm tauri dev', cwd: null, lastRun: '昨天 21:04', exit: 1, state: 'failed' }
        ]
      },
      {
        id: 'p5', name: 'api-gateway', path: 'E:\\old-projects\\api-gateway', rootId: null,
        note: 'Spring Boot 网关。目录被移动过，需要重新定位后命令才可运行。',
        mode: null, state: 'missing', lastRun: '半年前', runs: 23,
        createdAt: '2026-03-02',
        commands: [
          { id: 'c9', name: 'run', line: 'mvn spring-boot:run -Dspring-boot.run.profiles=local', cwd: null, lastRun: '半年前', exit: 0, state: 'idle' }
        ]
      },
      {
        id: 'p6', name: 'studio-site', path: 'D:\\Projects\\studio-site', rootId: 'r2',
        note: '工作室官网。内容在 content/ 下，改完跑 dev 看效果。',
        mode: null, state: 'idle', lastRun: '4 天前', runs: 18,
        createdAt: '2026-08-15',
        commands: [
          { id: 'c10', name: 'dev', line: 'pnpm dev', cwd: null, lastRun: '4 天前', exit: 0, state: 'exited' },
          { id: 'c11', name: 'build', line: 'pnpm build', cwd: null, lastRun: '4 天前', exit: 0, state: 'exited' },
          { id: 'c12', name: 'preview', line: 'pnpm preview --port 4173', cwd: null, lastRun: '一周前', exit: null, state: 'idle' }
        ]
      },
      {
        id: 'p7', name: 'nx-toolkit', path: 'D:\\GitHub\\nx-toolkit', rootId: 'r1',
        note: '刚克隆下来还没跑过。先看看 README 里怎么起。',
        mode: null, state: 'idle', lastRun: null, runs: 0,
        createdAt: '2026-10-30',
        commands: []
      },
      {
        id: 'p8', name: 'notes-vault', path: 'C:\\Users\\ICE\\Documents\\notes', rootId: null,
        note: '手动注册：不在任何根目录下，因此扫描不会刷新它，也不会被自动删除。',
        mode: 'builtin', state: 'exited', lastRun: '昨天 15:20', runs: 31,
        createdAt: '2026-06-11',
        commands: [
          { id: 'c13', name: 'serve', line: 'mkdocs serve -a 127.0.0.1:8123', cwd: null, lastRun: '昨天 15:20', exit: 0, state: 'exited' }
        ]
      }
    ],

    discovery: [
      { id: 'd1', rel: 'GitHub/awesome-go', flags: ['go.mod', '.git'], root: 'D:\\GitHub' },
      { id: 'd2', rel: 'GitHub/rust-by-example', flags: ['Cargo.toml', '.git'], root: 'D:\\GitHub' },
      { id: 'd3', rel: 'GitHub/tauri-samples', flags: ['package.json', '.git'], root: 'D:\\GitHub' },
      { id: 'd4', rel: 'Projects/blog-engine', flags: ['pyproject.toml', '.git'], root: 'D:\\Projects' },
      { id: 'd5', rel: 'Projects/design-tokens', flags: ['package.json'], root: 'D:\\Projects' },
      { id: 'd6', rel: 'GitHub/tinygo-playground', flags: ['go.mod'], root: 'D:\\GitHub' },
      { id: 'd7', rel: 'Projects/old-scraper', flags: [], root: 'D:\\Projects' },
      { id: 'd8', rel: 'GitHub/electron-tray-demo', flags: ['package.json'], root: 'D:\\GitHub' },
      { id: 'd9', rel: 'GitHub/cmake-hello', flags: ['CMakeLists.txt'], root: 'D:\\GitHub' },
      { id: 'd10', rel: 'Projects/sandbox-2025', flags: [], root: 'D:\\Projects' }
    ],

    runs: [
      { rid: '7f3a91c2', project: 'squirl', cmd: 'dev', line: 'pnpm dev', state: 'running', elapsed: '2m 14s', since: '2 分钟前', out: 1284 },
      { rid: '2b91d4e7', project: 'owl-runner', cmd: 'api', line: 'uv run uvicorn app.main:app --reload', state: 'starting', elapsed: '0.4s', since: '刚刚', out: 6 },
      { rid: 'c5d2f8a1', project: 'studio-site', cmd: 'dev', line: 'pnpm dev', state: 'exited', elapsed: '3m 02s', since: '4 天前', out: 3410 },
      { rid: '91e4b0c6', project: 'elegant-clipboard', cmd: 'dev', line: 'pnpm tauri dev', state: 'failed', elapsed: '18s', since: '昨天 21:04', out: 214 },
      { rid: 'a83f2d59', project: 'squirl', cmd: 'tauri:dev', line: 'pnpm tauri dev', state: 'killed', elapsed: '6m 41s', since: '昨天 18:40', out: 5210 },
      { rid: 'e1c7a3b4', project: 'squirl', cmd: 'build', line: 'pnpm build && pnpm tauri build', state: 'idle', elapsed: '—', since: '3 天前', out: 0 }
    ],

    log: [
      { c: 'lp-profile', t: '▸ profile 输出（7 行，点击展开） · 已折叠' },
      { c: 'lp-rule', t: '──── squirl command output ────────────────' },
      { c: '', t: '> squirl@0.1.0 dev D:\\Code\\AICreate\\ZCode\\squirl' },
      { c: '', t: '> vite' },
      { c: '', t: '' },
      { c: 'lp-ok', t: '  VITE v8.3.2  ready in 312 ms' },
      { c: '', t: '' },
      { c: 'lp-ok', t: '  ➜  Local:   http://localhost:5173/' },
      { c: 'lp-ok', t: '  ➜  Network: use --host to expose' },
      { c: 'lp-dim', t: '  ➜  press h + enter to show help' },
      { c: '', t: '' },
      { c: 'lp-warn', t: '  [vite] warning: sourcemap is enabled in production build' },
      { c: 'lp-dim', t: '  transform: 128 modules · cache: 96 hits' },
      { c: 'lp-err', t: '  Error: listen EADDRINUSE: address already in use :::5173' },
      { c: 'lp-dim', t: '      at Server.setupListenHandle (node:net:1817:16)' },
      { c: 'lp-dim', t: '      at listenInCluster (node:net:1865:12)  …' },
      { c: '', t: '' },
      { c: 'lp-ok', t: '  ℹ 重新使用已存在的 5173 端口实例，热更新可用' },
      { c: 'lp-dim', t: '  hmr update /src/styles.css  (42 ms)' }
    ],

    newLogLines: [
      { c: 'lp-dim', t: '  hmr update /src/App.tsx  (18 ms)' },
      { c: '', t: '  hmr update /src/components/ProjectCard.tsx  (9 ms)' },
      { c: 'lp-dim', t: '  hmr update /src/styles/tokens.css  (31 ms)' },
      { c: '', t: '  [vite] page reload (full reload required)' }
    ]
  };

  function calc() {
    db.projects.forEach(function (p) {
      p.cmdCount = p.commands.length;
      p.primary = p.commands.filter(function (c) { return c.state === 'running' || c.state === 'starting'; })[0] ||
        p.commands.slice().sort(function (a, b) { return (b.lastRun ? 1 : 0) - (a.lastRun ? 1 : 0); })[0] || null;
      p.missing = p.state === 'missing';
      p.badge = p.name.slice(0, 1).toUpperCase();
    });
    db.discovery.forEach(function (d) { d.name = d.rel.split('/').pop(); });
  }
  calc();

  db.projectById = function (id) {
    return db.projects.filter(function (p) { return p.id === id; })[0] || db.projects[0];
  };
  db.clearAll = function () {
    db.projects = [];
    db.discovery = [];
    db.roots.forEach(function (r) { r.count = 0; });
  };

  window.MOCK = db;
})();
