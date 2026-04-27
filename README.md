# DocLinker

DocLinker 是一个面向开发者的桌面文档启动器。它只索引用户配置的工作空间，把 Markdown、文本、快捷方式和可执行文件统一放进一个可搜索入口里，减少在文档、脚本和工具之间来回切换的成本。

当前实现基于 Tauri 桌面壳、React 前端和 Rust 后端：前端负责搜索框、结果列表和 Markdown 预览；后端负责配置、索引、SQLite FTS5 搜索、文件监听、托盘菜单和全局快捷键。

## 技术栈

| 层级 | 技术 | 用途 |
| --- | --- | --- |
| 桌面框架 | Tauri 2 | Windows 桌面应用壳、窗口生命周期、托盘、权限能力 |
| 前端 | React 19 + TypeScript 5.8 | 搜索 UI、结果列表、预览面板、交互逻辑 |
| 构建 | Vite 7 | 前端开发服务器和生产构建 |
| 样式 | Tailwind CSS 4 + CSS | 透明玻璃卡片、滚动条、Markdown 样式、代码块样式 |
| Markdown | react-markdown + remark-gfm | Markdown/GFM 渲染 |
| 代码高亮 | PrismJS | Markdown 代码块语法高亮 |
| 后端 | Rust 2021 + Tauri commands | 搜索、预览、工作空间配置、索引调度 |
| 存储 | rusqlite + SQLite FTS5 | 本地索引库、全文搜索 |
| 文件扫描 | walkdir | 递归扫描工作空间 |
| 文件监听 | notify | 监听 Create / Write / Rename / Remove 并增量更新索引 |
| 系统集成 | tauri-plugin-opener、global-shortcut、autostart | 打开文件/URL、Alt + F 唤醒、开机自启动 |

## 已支持功能

| 功能 | 说明 |
| --- | --- |
| 工作空间索引 | 只扫描 `config.json` 中启用的 workspace，不做全盘扫描 |
| 多类型文件识别 | 支持 Markdown、文本文件、`.exe`、`.url` |
| Markdown 元数据 | 支持从 frontmatter 的 `alias` / `aliases` 提取别名，从第一个标题提取标题 |
| 全文搜索 | 使用 SQLite FTS5 搜索 `alias`、文件名、标题和正文 |
| 搜索权重 | 当前权重为 `alias > file_name > title > content`，同分时按修改时间倒序 |
| 实时索引 | 使用 `notify` 监听工作空间，500ms 防抖后增量更新索引 |
| Markdown 预览 | 支持 GFM、表格、任务列表、代码块高亮 |
| 文本预览 | 支持 `.txt`、`.json`、`.yaml`、`.log` 等文本类文件预览 |
| 代码块复制 | Markdown 代码块顶部提供复制按钮 |
| 快捷提取 | 搜索列表聚焦 Markdown 时，`Ctrl + C` 复制第一个代码块 |
| 文件/URL 打开 | `.url` 调用系统默认浏览器，其他非预览文件调用系统默认打开方式 |
| 快捷键唤醒 | `Alt + F` 显示/隐藏窗口 |
| 自动隐藏 | `Esc` 或窗口失焦时隐藏到后台 |
| 系统托盘 | 托盘菜单支持显示/隐藏、开机自动启动、退出 |
| 透明窗口 | 无标题栏、透明背景、独立玻璃卡片式搜索/列表/预览区域 |

## 支持的文件类型

| 类型 | 扩展名 | 行为 |
| --- | --- | --- |
| Markdown | `.md` | 索引文件名、标题、别名、正文；支持预览和代码块复制 |
| 文本 | `.txt`、`.html`、`.htm`、`.json`、`.xml`、`.yaml`、`.yml`、`.csv`、`.log` | 索引文件名和正文；支持纯文本预览 |
| 可执行文件 | `.exe` | 索引文件名；命中后可直接打开 |
| URL 快捷方式 | `.url` | 读取 `URL=` 目标地址；命中后用默认浏览器打开 |

默认排除目录：`node_modules`、`.git`、`dist`、`target`。可以在运行时配置文件中调整。

## 配置位置

### 运行时配置

应用首次启动会在用户数据目录下创建配置和索引文件。

| 文件 | Windows 默认位置 | 说明 |
| --- | --- | --- |
| `config.json` | `%APPDATA%\DocLinker\config.json` | 工作空间、排除项、开机自启动状态 |
| `doclinker.db` | `%APPDATA%\DocLinker\doclinker.db` | SQLite 索引库，不建议手工编辑 |

`config.json` 示例：

```json
{
  "version": 1,
  "workspaces": [
    {
      "id": "ws-docs",
      "name": "Docs",
      "path": "D:\\Users\\your-name\\Documents\\notes",
      "kind": "doc",
      "enabled": true
    }
  ],
  "excludePatterns": ["node_modules", ".git", "dist", "target"],
  "launchOnStartup": false
}
```

`kind` 可选值：

| 值 | 用途 |
| --- | --- |
| `project` | 项目目录 |
| `doc` | 文档目录 |
| `scripts` | 脚本或工具目录 |

手工修改 `config.json` 后建议重启应用，让后端重新加载配置并重建对应工作空间索引。

### 开发配置

| 文件 | 作用 |
| --- | --- |
| `package.json` | npm 脚本、前端依赖、Tauri CLI 依赖 |
| `vite.config.ts` | Vite + React + Tailwind 配置；Tauri 开发端口固定为 `1420`，HMR 端口为 `1421` |
| `tsconfig.json` | 前端 TypeScript 严格编译配置 |
| `tsconfig.node.json` | Node/Vite 配置文件的 TypeScript 配置 |
| `src-tauri/Cargo.toml` | Rust crate 元信息、Tauri 插件、SQLite、文件监听等依赖 |
| `src-tauri/tauri.conf.json` | Tauri 产品名、窗口参数、构建命令、打包图标 |
| `src-tauri/capabilities/default.json` | Tauri 权限能力：opener、global-shortcut、autostart |
| `index.html` | 前端 HTML 入口 |

## 开发命令

| 命令 | 说明 |
| --- | --- |
| `npm install` | 安装前端和 Tauri CLI 依赖 |
| `npm run dev` | 只启动 Vite 前端开发服务器 |
| `npm run build` | 执行 TypeScript 检查并构建前端产物到 `dist/` |
| `npm run preview` | 预览前端构建产物 |
| `npm run tauri -- dev` | 启动 Tauri 桌面开发模式 |
| `npm run tauri -- build` | 构建桌面应用安装包/可执行产物 |

注意：`npm run tauri -- dev` 会根据 `src-tauri/tauri.conf.json` 自动执行 `npm run dev` 作为前端开发服务。

## 项目结构

```text
DocLinker/
├─ public/                       # Vite 静态资源，构建时原样复制
├─ src/                          # React 前端源码
│  ├─ App.tsx                    # 搜索、结果列表、预览、快捷键等主界面逻辑
│  ├─ App.css                    # Tailwind 引入、玻璃态 UI、Markdown 和代码块样式
│  ├─ main.tsx                   # React 入口
│  ├─ vite-env.d.ts              # Vite 类型声明
│  └─ assets/                    # 前端资源文件
├─ src-tauri/                    # Tauri/Rust 后端工程
│  ├─ Cargo.toml                 # Rust 依赖和 crate 配置
│  ├─ Cargo.lock                 # Rust 依赖锁定文件
│  ├─ build.rs                   # Tauri 构建脚本入口
│  ├─ tauri.conf.json            # Tauri 应用、窗口和打包配置
│  ├─ capabilities/              # Tauri 权限声明
│  │  └─ default.json
│  ├─ icons/                     # 应用图标资源
│  ├─ gen/                       # Tauri 生成文件
│  ├─ src/                       # Rust 后端源码
│  │  ├─ main.rs                 # 桌面程序入口，调用 doclinker_lib::run()
│  │  ├─ lib.rs                  # Tauri builder、托盘、快捷键、窗口事件、应用初始化
│  │  ├─ commands.rs             # 暴露给前端的 Tauri commands
│  │  ├─ config.rs               # AppData 路径、config.json 读写、workspace 追加
│  │  ├─ db.rs                   # SQLite 初始化、FTS5 表、连接参数
│  │  ├─ indexer.rs              # 全量/增量索引、搜索、预览读取
│  │  ├─ scanner.rs              # 工作空间扫描、文件类型识别、Markdown 元数据提取
│  │  ├─ watcher.rs              # 文件系统监听和 500ms 防抖增量索引
│  │  ├─ models.rs               # 配置、DTO、文件类型等数据结构
│  │  └─ error.rs                # 统一错误类型
│  └─ target/                    # Rust 编译产物，生成目录
├─ dist/                         # 前端构建产物，生成目录
├─ node_modules/                 # npm 依赖，生成目录
├─ package.json                  # npm 脚本和前端依赖
├─ package-lock.json             # npm 依赖锁定文件
├─ tsconfig.json                 # 前端 TypeScript 配置
├─ tsconfig.node.json            # Vite/Node TypeScript 配置
├─ vite.config.ts                # Vite 配置
└─ README.md                     # 项目说明
```

## 后端模块说明

| 模块 | 职责 |
| --- | --- |
| `lib.rs` | 组装 Tauri 应用：插件、全局快捷键、托盘、窗口隐藏策略、索引初始化、watcher 启动 |
| `commands.rs` | 提供 `list_workspaces`、`add_workspace`、`search`、`read_preview` 给前端调用 |
| `config.rs` | 管理 `%APPDATA%\DocLinker`、配置文件读写、workspace 路径规范化 |
| `db.rs` | 创建 `files` 表和 `files_fts` FTS5 表，启用 WAL、foreign keys、NORMAL synchronous |
| `scanner.rs` | 扫描 workspace，过滤排除目录，解析 Markdown alias/title 和 `.url` 目标地址 |
| `indexer.rs` | 重建 workspace 索引、增量更新/删除/重命名、FTS 搜索、读取预览 |
| `watcher.rs` | 监听启用的 workspace，对文件创建/修改做防抖索引，对删除/重命名同步更新索引 |
| `models.rs` | 定义配置结构、工作空间类型、文件类型、搜索结果 DTO 和预览 DTO |
| `error.rs` | 将 IO、SQLite、notify、Tauri、JSON 错误统一为 `AppError` |

## 当前限制与规划中能力

| 状态 | 说明 |
| --- | --- |
| 当前 UI 还没有工作空间管理入口 | 后端已有 `add_workspace` / `list_workspaces` command，当前主要通过 `config.json` 配置 workspace |
| 命令执行中心尚未完成 | 需求文档中的代码块运行、变量注入、敏感命令二次确认仍属于后续能力 |
| 索引重建按钮尚未完成 | 当前启动时会重建已配置 workspace；独立的 Rebuild Index UI 尚未实现 |
| 搜索热度权重尚未完成 | 需求中的 `last_accessed_at` / `access_count` 动态权重字段当前未落库 |

## 使用建议

1. 先启动一次应用，让它生成 `%APPDATA%\DocLinker\config.json`。
2. 关闭应用后编辑 `config.json`，加入需要索引的 workspace。
3. 重新启动应用，后端会重建索引并启动文件监听。
4. 使用 `Alt + F` 唤醒搜索窗口，输入关键词搜索文档、脚本或工具。
5. 对 Markdown 或文本结果按 `Enter` 预览；对 `.exe` 或 `.url` 结果按 `Enter` 打开。
