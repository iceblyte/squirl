# Squirl

**Squirl**（松鼠的变体拼法）——一只替你记住坚果埋在哪的松鼠。

一个**本地项目台账与一键启动中枢**：把散落在多个根目录下的项目登记成一张可搜索的清单，记住每个项目该怎么跑，点一下就启动——默认在你惯用的终端里执行，也可切换为应用内运行并实时查看日志；由应用内启动的进程，在退出时全部收回。

它**不是**：文件搜索器、环境管理器、IDE、终端替代品、重型 Git 客户端。

## 它解决什么

1. **找不到**：项目散落在 `GitHub/`、`Projects/`、临时目录——台账随时间增值，按最近使用排序，≤3 字符定位。
2. **上下文失忆**：备注 + 命令清单记录「这个项目是什么、怎么跑」。
3. **启动链路繁琐**：找目录 → 开终端 → cd → 回忆命令 ≈ 20~40 秒 → 这里一下点击。
4. **退出残留**：应用内启动的进程（含 `npm run dev` 派生的孙进程）退出时全部收回。

## 双模式一键启动

- **外部终端（默认）**：按「模拟器 × Shell」组合在命令的工作目录唤起（Windows Terminal / 控制台宿主 × pwsh7 / PowerShell 5.1 / cmd / Git Bash），命令跑完窗口保持打开，进程归终端所有。
- **应用内运行**：登录 shell 加载 profile，stdout/stderr 实时捕获（ANSI 保留、30ms 分批、环形缓冲 5000 行 / 2MB），停止 = 整棵进程树回收（Windows Job Object / Unix setsid）。

## 平台支持（诚实口径）

| 平台 | 状态 |
|---|---|
| Windows 10/11 | ✅ 实机验证（NSIS 安装包） |
| macOS（DMG） | ⚠️ 照发，**未经实机验证**（终端适配为代码级实现 + 单测） |
| Linux（AppImage/deb） | ⚠️ 照发，**未经实机验证** |

> 本项目不做「三端可用」宣传；上表即当前真实状态。

## 安装

从 [GitHub Releases](https://github.com/your-org/squirl/releases) 下载：

- Windows：`Squirl_*_x64-setup.exe`（NSIS）。**安装包未签名**，SmartScreen 可能提示——「更多信息 → 仍要运行」。
- 数据目录：`%APPDATA%\app.squirl.desktop`（卸载不清除）。

## 已知限制

诚实清单见 [docs/用户文档.md](docs/用户文档.md)「已知限制」段，要点：

- 应用内模式无 stdin（不支持交互式命令）；30s 无输出会提示疑似等待输入。
- 异常崩溃时的进程回收兜底三平台能力不同（Windows 内核保证 / Linux 仅直接子 / macOS 可能孤儿）。
- 命令明文存储（v1 不加密）；安装包未签名。
- cmd 的 `/k` 拦不住命令里的字面 `exit`（窗口会关闭）。

## 开发

技术栈：Tauri 2.12 + Rust（rusqlite/tokio/jwalk）+ React 19 + TypeScript 7 + Vite 8 + Tailwind 4 + shadcn/ui。

```bash
pnpm install
pnpm tauri dev        # 开发
pnpm tauri build      # 打包（NSIS）
pnpm test             # vitest
pnpm lint             # biome
cargo test            # src-tauri/（含 M0 Spike 实机冒烟：--ignored 用例）
```

- 架构与决策：[docs/方案设计文档.md](docs/方案设计文档.md)（DR-1~DR-18）
- 需求基线：[docs/需求分析文档-汇总版.md](docs/需求分析文档-汇总版.md)
- 阶段证据：[docs/进度与验证记录.md](docs/进度与验证记录.md)

## License

MIT
