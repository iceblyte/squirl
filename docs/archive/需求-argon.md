## 一、 想法可靠性验证与竞品调研

### 1. 痛点真实性与项目价值评估（是否值得做？）

**核心结论：非常值得做，且极具成为 GitHub Trending 爆款开源项目的潜质。**

当前开发者的日常工作流已经分裂为两种截然不同的模式：

- **创造者模式 (Creator Mode)**：深入编写代码，依赖 VSCode、IDEA、PyCharm 等重型 IDE。
- **探索者/使用者模式 (Explorer & Operator Mode)**：从 GitHub 克隆有趣的开源工具、AI Agent、脚本或自建微服务到本地，目的往往是“跑起来用它的功能”**或**“改几个配置体验一下”，而非阅读和重构整套源码。

现有工具几乎全部围绕“写代码”设计，导致以下三个高频痛点长期处于“野生状态”：

1. **上下文失忆 (Context Amnesia)**：项目散落在 `GitHub/`、`Projects/`、`Demos/` 等不同目录，过两周完全想不起某个英文名的仓库是干嘛的、当时为什么要克隆它。
2. **启动仪式感过重 (Ritual Fatigue)**：每次运行都要经历“找目录 ➔ 开对应终端 ➔ 激活 `.venv` ➔ 回忆是 `pnpm dev` 还是 `uv run` ➔ 纠结之前到底有没有跑过 `npm install`”。
3. **文档与配置割裂 (README-to-Config Friction)**：把开源项目跑起来的核心动作，90% 是对照着 `README.md` 里的说明去复制并填写 `.env` 或 `application.yml`，再加上散落在全局的 CLI 工具（如 `pi`、`dsh`、`codex`）容易被遗忘用法。

**为什么做成开源项目极具传播力？** 开发者生产力工具（DevTools）在 GitHub、Reddit（`r/rust`、`r/tauri`、`r/commandline`）、X 和 V2EX 拥有天然的高转发基因。只要做到“秒开、轻量、颜值在线、痛点精准”，极易获得开发者的自发 Star 与社区插件贡献。

### 2. 竞品调研矩阵

我们对目前市面上及 GitHub 社区中的相关工具进行了全面摸底：

| **竞品名称**                                   | **核心定位**                    | **优势**                                                     | **核心缺失（你的机会点）**                                   |
| ---------------------------------------------- | ------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------ |
| **JetBrains Toolbox / VSCode Project Manager** | IDE 工程启动器                  | 与编辑器深度绑定，打开源码极快                               | 只管“用 IDE 打开目录”，不管“怎么把项目跑起来”、无配置快填、无全局 CLI 管理 |
| **DevGo / DevDock**                            | 轻量级项目跳转器 (Tauri 2)      | 近期社区涌现的同类探索，支持快速跳编辑器或终端               | 仅停留在“路径书签跳转”，缺乏**状态感知命令（自动跳过 install/自动激活虚拟环境）**、**README 与配置同屏快填**及**插件体系** |
| **Pinokio**                                    | AI 开源项目一键浏览器           | 针对特定开源项目提供一键安装与运行脚本                       | 极其笨重（Electron + 独立全套环境动辄数十 GB），黑盒化严重，无法纳管开发者自己的常规项目 |
| **Warp (Launch Configs) / Pet**                | 终端工作流与命令片段工具        | 能保存常用命令组合与终端分屏布局                             | 要么强绑定特定商业终端（Warp），要么是纯命令行片段，无法与本地项目目录、技术栈指纹、配置文件联动 |
| **本项目 (暂定名: DevLaunch / Orbit)**         | **本地项目启动器与 CLI 军火库** | **目录自动发现 + 状态感知启动 + README/配置双栏快填 + 全局 CLI 库 + 插件生态** | **精准填补“文件管理器”与“重型 IDE”之间的生态空白**           |

## 二、 参考项目架构拆解与技术栈选型

### 1. 四大参考项目的核心设计汲取

我们对你提供的四个标杆项目进行了源码级与架构级拆解，提炼出可直接复用到本项目的架构基因：

- **参考 1：`yetone/magpie`（近期在 GitHub 38 天狂揽 2.4 万+ Star 的本地网关工具）**
  - **核心设计精髓**：
    1. **无损原子化配置重写 (Atomic Config Rewriting)**：修改各工具的配置文件时，精准改写目标键值，但**100% 保留原文件的注释、字段顺序和排版格式**。
    2. **多形态轻量驻留**：支持桌面主窗口 + 菜单栏托盘 (Menu Bar) 快速切换。
    3. **按需加载的轻量级插件**：插件基于 JS/Bun 运行，支持生命周期钩子，异常或超时自动熔断回退，绝不拖垮宿主主进程。
  - **本项目吸收点**：在实现 **“参照 `README.md` 快速填写 `application.yml` / `.env`”** 时，采用 `magpie` 同款的无损配置重写技术——用户在可视化表单里填空，底层保留原配置文件的所有注释与格式；同时提供系统托盘与全局快捷键极速唤起。
- **参考 2：`cyberpath-HQ/orbis`（Rust + Tauri + React 插件驱动桌面平台）**
  - **核心设计精髓**：
    1. **Schema-Driven UI（JSON Schema 驱动渲染）**：插件不需要自己打包笨重的 React 前端代码，而是直接输出声明式的 JSON Schema，由宿主的 `SchemaRenderer` 统一调用内置的 `shadcn/ui` 组件库渲染。
    2. **WASM 沙箱隔离 (`wasmtime`) + 页面级 Zustand 状态隔离**。
  - **本项目吸收点**：插件系统采用 **“JSON Schema 声明式 UI + 宿主统一渲染”** 模式。第三方开发者写一个“跨端同步插件”或“Docker 容器监控插件”时，只需声明配置表单和动作回调，即可获得与主程序完全一致的 Tailwind/shadcn 视觉体验。
- **参考 3：`thejacedev/Noteriv`（基于 Tauri 2 的开源 Markdown 编辑器）**
  - **核心设计精髓**：
    1. **Local-First 纯文本/无锁定理念**：数据完全归属本地，天然亲和 Git 与云盘同步。
    2. **File Watcher 实时文件监听**：后台实时监听目录变化（外部编辑器修改、Git 拉取），UI 状态毫秒级自动刷新。
    3. **GitHub 社区插件注册表机制 (`NoterivPlugins` / `NoterivThemes`)**：通过 `manifest.json` 实现一键安装社区插件与主题。
  - **本项目吸收点**：引入 Rust 层的 `notify` 文件监听器，当用户在外部执行了 `git pull` 或新增了子项目文件夹时，启动器自动感知并更新技术栈图标与状态；插件市场直接复用 GitHub Manifest 分发机制，零服务器成本。
- **参考 4：`Obsidian`（现代桌面插件架构标杆）**
  - **核心设计精髓**：**Microkernel（微内核）+ Core Plugins（官方核心插件）+ Community Plugins（社区插件）+ 全局 Command Palette（`Ctrl/Cmd + P` 命令面板）**。
  - **本项目吸收点**：保持底座绝对轻量，将“项目纳管、命令流水线、配置快填、全局 CLI”作为内置核心模块；将“Git/WebDAV/S3 跨设备同步”、“AI 自动解析 README 提取启动命令”作为可插拔的官方/社区插件。

### 2. 技术栈选型对比：`Tauri v2 + React` vs `Rust + GPUI`

| **评估维度**                   | **方案 A：Tauri v2 + Rust + React + TS + Tailwind CSS（强烈推荐 ★）** | **方案 B：Rust + GPUI（Zed 编辑器框架）**                    |
| ------------------------------ | ------------------------------------------------------------ | ------------------------------------------------------------ |
| **启动速度与内存**             | 极快（冷启动小于 150ms，隐藏至托盘释放 WebView 后内存仅 **15~25 MB**，安装包约 **6~10 MB**） | 极致（纯原生 GPU 渲染，无 WebView 开销）                     |
| **README 与配置表单渲染**      | **降维打击**：成熟的 GFM Markdown 渲染（支持表格、代码高亮、Mermaid 图）与动态配置表单组件生态 | **开发成本极高**：需要手搓复杂的富文本图文混排布局、表格渲染以及动态表单控件 |
| **三端兼容性 (Win/Mac/Linux)** | Tauri v2 三端极其成熟，且原生提供完善的 GitHub Actions 跨平台打包流水线 | GPUI 最初为 macOS 打造，Windows 与 Linux（尤其是中文输入法 IME 与多显示器 DPI 缩放）仍有较多底层坑 |
| **插件生态扩展性**             | 完美支持 JS/TS 插件与 `orbis` 风格的 JSON Schema 动态 UI 渲染 | 纯编译型 Rust 视图树，难以在不重新编译宿主的情况下让第三方插件动态注入自定义 UI |

> **选型建议**：坚定选择 **Tauri v2 + Rust (后端核心引擎) + React 19 + TypeScript + Tailwind CSS + shadcn/ui (前端视图)**。将所有耗时操作（目录扫描、技术栈指纹探测、AST 配置解析、进程与终端调度、SQLite 读写）100% 放在 **Rust 后端**并发执行，前端仅负责轻量交互与高精度渲染，兼顾开发效率、颜值与极致性能。

## 三、 为你补充的 6 个“高光亮点”设计思路

在完全保留你原有需求的基础上，结合独立开发者的实战经验，我为你构思了 6 个能让用户直呼“太懂我了”的杀手级细节：

1. **「感知依赖锁文件 Hash」的智能跳过机制（升级你的“跳过 install”想法）**
   - **你的原想法**：如果是 Node 项目，之前执行过 `pnpm install`，再次运行就跳过。
   - **进阶亮点**：如果仅仅记录一个布尔值“已执行”，当你某天 `git pull` 拉取了别人新增的依赖后，跳过 `install` 就会报错。我们可以让 Rust 后端在首次执行 `pnpm install` / `uv sync` 成功后，**自动记录当前 `pnpm-lock.yaml` / `package.json` / `uv.lock` 的文件 Hash 值（并检测 `node_modules` 或 `.venv` 目录是否存在）**。
   - **效果**：日常点击“一键启动”直接秒进 Run 阶段；一旦检测到你更新了代码导致依赖锁文件变化，或者你手动删了 `node_modules`，应用会自动高亮提示或自动补跑一次 `install`！
2. **「README × 配置文件」双栏联动与 `.env.example` 一键转正**
   - 很多 GitHub 项目克隆下来只有 `.env.example` 或 `config.example.yaml`。
   - 当进入项目的“配置快填”视图时，左侧展示 `README.md`，右侧自动扫描项目内的配置文件。如果发现只有 `.env.example` 而没有 `.env`，提供 **“一键生成 `.env` 并开始填写”** 按钮。
   - **智能高亮联动**：Rust 后端自动提取 `.env` 中的 Key（如 `OPENAI_API_KEY`、`REDIS_URL`），在左侧 `README.md` 中自动高亮标注这些变量名出现的位置，点击右侧表单项，左侧 README 自动滚动到对应的说明段落！
3. **「初心便签」+ GitHub 元数据自动回填**
   - 克隆下来的项目如果每次都要手动打字写“这个项目是干嘛的”，用户有时候会犯懒。
   - 当扫描到包含 `.git` 的项目时，Rust 自动读取 `.git/config` 中的 `remote.origin.url`，可一键拉取（或从本地 `README.md` 第一段提取）项目的原始描述与 GitHub Topics 标签作为“官方简介”**；同时留出最醒目的顶部横幅给你的**“个人备注（当初为什么克隆/开发它）”。
4. **「全局 CLI 军火库」的交互式参数槽位 (`{{param}}`)**
   - 针对你提到的 `pi`、`dsh`、`codex` 及带参数组合（如 `dsh deploy xxx`）：支持使用双花括号定义参数模板，例如：`dsh deploy {{env:dev|staging|prod}} --target {{app_name}}`。
   - 点击该全局命令时，直接弹出一个微型的键盘友好填空框（`env` 自动变成下拉菜单），填完回车立即在所选终端执行，彻底告别死记硬背命令参数。
5. **Oh-My-Posh 风格指纹 +「依赖黑洞清理器」**
   - 除了识别并展示技术栈图标（Rust、Python、Node、Spring Boot、Docker 等）及运行时版本（如读取 `.nvmrc`、`.python-version`），还能顺手计算 `node_modules`、`target`、`.venv` 的磁盘占用。
   - 对于那些“半年前克隆下来跑过一次、现在占了 2GB 空间但又舍不得删仓库”的项目，提供 **“一键休眠（仅清理依赖构建目录，保留源码与配置）”** 按钮。反正下次想跑的时候，我们的状态感知启动器会自动帮你重新安装依赖！
6. **「跨设备相对路径变量映射」同步机制（为后续同步插件打底）**
   - 跨设备同步最大的坑是：Windows 上项目在 `D:\GitHub\awesome-tool`，Mac 上在 `~/Code/github/awesome-tool`。
   - 我们在底层数据模型中不直接硬编码绝对路径，而是存储 **`WorkspaceRoot_ID + 相对路径`**（以及 `.git` Remote URL 作为唯一指纹）。这样未来启用 Git/云同步插件时，你在 Mac 和 Windows 上各自指定一次 `GitHub` 根目录，所有的项目分组、个人备注、启动流水线、全局命令就能实现**跨操作系统的无缝漫游**！

## 四、 《需求分析文档 (PRD v1.0)》

### 1. 产品概述

- **产品名称（暂定）**：**DevPort / RunDeck / MagLauncher**（可在确认阶段定夺）
- **一句话定位**：一款极速、低内存、支持插件生态的跨平台本地项目启动器与全局 CLI 军火库。
- **核心设计哲学**：
  - **Launcher, Not IDE（做启动器，不做重型编辑器）**：不展示复杂源码树，写代码一键移交 VSCode / JetBrains；专注于“找得到项目、记得住初心、改得了配置、一键跑得起来”。
  - **Bring Your Own Runtime（环境自治）**：应用本身不内置或接管 Node/Python/Java 的安装，而是聪明地感知系统已有环境与虚拟环境并调度终端。
  - **Local-First & Extensible（本地优先与插件化）**：所有元数据本地极速存储，通过插件机制扩展跨端同步与进阶能力。

### 2. 核心功能模块需求 (Functional Requirements)

#### 模块一：项目注册与智能发现中心 (Workspace & Discovery)

- **固定工作区扫描 (Watched Roots)**：
  - 支持配置多个根文件夹（例如 `D:\GitHub` 作为“外部克隆区”，`D:\MyProjects` 作为“自研项目区”）。
  - 支持设置递归扫描深度（如 1~3 层），根据特征文件（`.git`、`package.json`、`pyproject.toml`、`Cargo.toml`、`pom.xml`、`go.mod` 等）自动识别有效项目边界，避免把普通子目录误判为独立项目。
- **手动单项目导入**：支持拖拽文件夹或选择任意散落路径手动注册项目。
- **分组、标签与多维检索**：
  - 支持自定义分组（如“AI 工具”、“自研后端”、“前端组件库”、“待研究”）、多标签（Tags）及收藏置顶。
  - 全局模糊搜索（支持按项目名、个人备注内容、技术栈标签、甚至是克隆来源 URL 瞬间检索，支持拼音首字母匹配）。

#### 模块二：技术栈指纹与记忆卡片 (Tech Stack & Context Card)

- **Oh-My-Posh 风格技术栈徽章**：
  - Rust 后端极速解析项目根目录特征文件，自动渲染高辨识度 SVG 图标徽章：
    - 语言与框架：Python / FastAPI / Django、Node / React / Vue / Next.js、Rust / Tauri、Java / Spring Boot、Go、Docker 等。
    - 包管理器与构建工具：`uv`、`poetry`、`pip`、`pnpm`、`npm`、`yarn`、`bun`、`maven`、`gradle`、`cargo`。
- **项目记忆与备注系统 (Why I Cloned/Built This)**：
  - 每个项目提供显眼的 **“一句话初心备注”** 与 Markdown 详细笔记区。
  - 自动读取 `.git` 信息展示：远程 GitHub 地址（点击直达网页）、最后一次 Commit 时间、当前分支、本地是否有未提交修改。
- **外部 IDE 一键联动**：
  - 顶部快捷栏支持一键使用 VSCode (`code .`)、Cursor、IntelliJ IDEA、PyCharm、Zed 或系统文件资源管理器打开当前项目目录。

#### 模块三：状态感知命令流水线与多终端调度 (Stateful Runner & Terminal Dispatcher)

- **三段式智能命令编排**：
  1. **环境前置层 (Env Pre-hook)**：自动检测项目下的 `.venv` / `venv` 等虚拟环境目录。若为 Python 项目且勾选“自动激活虚拟环境”，在生成最终终端执行脚本时，自动根据所选终端类型注入对应激活指令（如 PowerShell 注入 `& ".\.venv\Scripts\Activate.ps1"`，Git Bash 注入 `source .venv/Scripts/activate`，macOS/Linux zsh 注入 `source .venv/bin/activate`）。
  2. **初始化层 (Setup / Install Steps)**：记录首次运行所需的构建或安装命令（如 `pnpm install`、`mvn clean install`）。记录执行状态与锁文件 Hash，**已成功执行且锁文件未变更时，后续启动默认自动跳过**；同时提供“强制重新执行初始化”的勾选框。
  3. **运行层 (Run Commands)**：自动从 `package.json` (`scripts`)、`pyproject.toml`、`Makefile`、`Cargo.toml` 提取可执行脚本供一键勾选，也支持用户手动编写多条常用启动命令（如“启动开发服”、“启动 Worker”）。
- **多终端自由选择与记忆**：
  - 自动探测当前操作系统已安装的终端：
    - **Windows**：PowerShell 7 (`pwsh`)、Windows PowerShell、CMD、Git Bash、Windows Terminal (`wt`)、WSL。
    - **macOS**：Terminal.app、iTerm2、Warp、Ghostty、Alacritty、Kitty。
    - **Linux**：GNOME Terminal、Konsole、Alacritty、Kitty、WezTerm。
  - 支持设置“全局默认终端”，同时支持**每个项目单独指定并记住其专属启动终端**。

#### 模块四：README 浏览与配置文件双栏快填台 (README & Config Twin-View)

- **高性能 README 预览器**：
  - 自动定位项目下的 `README.md`（优先匹配中文 `README.zh-CN.md` 或允许一键切换语言版本）。
  - 支持本地相对路径图片正常渲染、代码片段一键复制（或直接将 README 中的代码块一键添加为该项目的启动命令！）。
- **配置文件自动嗅探与快填**：
  - 自动扫描并列出项目中的核心配置文件：`.env`、`.env.local`、`.env.example`、`application.yml`、`application.properties`、`config.json`、`config.toml` 等（支持自定义配置文件匹配规则）。
  - **模板一键生成**：若存在 `.env.example` 但缺失 `.env`，提供一键复制创建功能。
  - **双栏对照与快速填写**：左侧滑动阅读 `README.md` 的配置说明，右侧通过结构化键值表单（针对 `.env` / `.yml` / `.toml`）或高亮文本编辑器快速修改并保存，底层采用无损重写保留原文件注释。

#### 模块五：全局 CLI 命令军火库 (Global CLI Arsenal)

- **全局工具登记簿**：
  - 独立面板用于管理 `pi`、`dsh`、`codex`、`ffmpeg`、`ngrok` 等全局命令行工具。
  - 记录信息包括：命令名称、一句话用途备忘（解决“忘了它是干嘛的”）、标签分类、关联官方文档链接。
- **常用参数组合模板 (Parameterized Presets)**：
  - 每个全局工具下可挂载多个“常用组合”（例如：`dsh deploy {{service}} --env {{env:dev|prod}}`）。
  - 支持一键复制命令到剪贴板，或填入参数后直接唤起终端执行（可选在默认目录或当前选中的某个项目目录下执行）。

#### 模块六：插件架构与跨端同步底座 (Plugin System & Sync)

- **核心扩展点 (Extension Points)**：
  - **Lifecycle Hooks**：`onProjectScanned`、`onBeforeProjectRun`、`onAfterProjectRun`、`onConfigSaved`、`onAppIdle`。
  - **UI Slots**：侧边栏导航槽位、项目详情页 Tab 槽位、状态栏徽章槽位、全局命令面板 (`Cmd/Ctrl + K`) 动作注入。
- **官方规划插件 1：跨设备同步插件 (Sync Plugin)**：
  - 支持通过私有 **Git 仓库**、**WebDAV（如坚果云/Nextcloud）** 或 **S3/Cloudflare R2** 同步应用的配置库（项目元数据、个人备注、命令流水线、全局 CLI 军火库、插件配置）。
  - 依靠“工作区根路径映射 (`${ROOT_GITHUB}`) + Git Remote URL 指纹”，实现在 Windows 台式机与 MacBook 之间同步备注与启动配置。

### 3. 非功能性需求与分发策略

- **性能指标**：
  - 主窗口冷启动时间小于 150ms；托盘唤醒时间小于 30ms。
  - 内存占用：空闲/托盘状态小于 30MB，活跃浏览状态小于 80MB。
  - 扫描性能：Rust 多线程异步扫描（基于 `ignore` crate 极速跳过 `node_modules` / `target` / `.git` 内部目录），扫描 200 个本地项目耗时小于 200ms。
- **数据存储**：
  - 采用本地 **SQLite**（通过 `rusqlite` 或 `sqlx`）存储项目索引与运行状态，同时支持一键导出/同步为结构清晰的 JSON/TOML 文件（方便 Git 同步时的 Diff 与合并）。
- **三端打包与包管理器分发 (CI/CD)**：
  - 基于 GitHub Actions 自动构建三端产物：
    - **Windows**：`.msi` / `-setup.exe` (NSIS)，并自动发布 Manifest 至 **Scoop Bucket** (`scoop install <app>`) 与 **Winget**。
    - **macOS**：Universal `.dmg` / `.app`（支持 Apple Silicon & Intel），自动更新 **Homebrew Tap** (`brew install --cask <app>`)。
    - **Linux**：`.deb`、`.AppImage`、`.rpm`，支持 **AUR** (Arch Linux)。