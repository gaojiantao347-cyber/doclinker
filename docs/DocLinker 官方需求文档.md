DocLinker 官方需求文档 (Optimized v1.1)
1. 项目愿景
DocLinker 是一款专为开发者设计的“非侵入式”工作流增强工具。它通过对用户指定的工作目录（Workspace）进行深度解析，将静态的文档转化为可交互、可执行的资产，消除从“阅读文档”到“执行操作”之间的切换成本。

2. 核心架构设计
2.1 数据索引引擎 (Rust Backend)
Scoped Scanning: 拒绝全盘扫描。仅对用户配置的 workspaces 列表进行递归索引。

混合存储架构:

SQLite FTS5: 存储 Markdown 的全文内容及文件名，支持秒级模糊搜索。

Memory Cache: 缓存高频访问的 .exe 和 .url 路径。

实时响应 (FileWatcher): 集成 notify 模块。当检测到 Create / Write / Rename / Remove 事件时，在 500ms 内完成增量索引更新。

2.2 搜索与权重逻辑
多维匹配: 搜索优先级为：Alias (别名) > Filename (文件名) > Markdown Header (文档标题) > Markdown Body (正文)。

动态权重: 引入 last_accessed_at 和 access_count 字段。每次打开文件或执行命令，该条目的权重提升。

3. 功能详细说明
3.1 工作空间管理 (Workspace)
路径挂载: 支持拖拽文件夹添加。

类型定义:

Project: 默认优先搜索 .git, package.json, solution.sln 等。

Doc: 重点扫描 .md, .pdf, .html。

Scripts: 扫描 .ps1, .bat, .py, .exe。

排除项: 默认全局排除 node_modules, .git, dist, target 等。

3.2 Markdown 交互增强 (Action Center)
这是本工具的核心竞争力，需满足：

Smart Code Blocks:

自动识别代码块语言。

一键复制: 按钮悬浮显示。

终端直达: 点击运行图标，后端通过 std::process 启动 powershell.exe -Command "{code}"。

变量注入 (Variable Injection):

正则识别 {variable_name}。

触发运行时，UI 弹出 Input 浮层，支持 Tab 切换多个变量输入，回车确认执行。

快捷提取 (Ctrl+C Hook):

在搜索列表焦点停留于某文档时，按 Ctrl+C 直接将第一个代码块提取到剪贴板，并伴有 Toast 提示。

4. 交互设计 (UI/UX)
4.1 窗口行为
沉浸式体验: 采用 Mica 或 Acrylic 透明材质，无标题栏。

唤醒逻辑: Alt + Space 快速呼出；窗口失去焦点（OnBlur）或按下 Esc 立即隐藏至后台。

4.2 布局分布
左侧/顶部: 极简搜索框。

中间: 搜索结果列表（带分类 Icon 和路径缩略）。

右侧: Markdown 预览侧边栏（宽度自适应，支持滚动）。

5. 非功能性需求 (Quality Attributes)
性能限制:

冷启动时间 < 1.5s。

搜索响应时间 < 100ms。

内存占用 (Idle) < 100MB。

安全性:

所有命令执行前需在 UI 界面有明显提示。

涉及系统敏感命令（如 rm, del, format）时，运行图标变红并强制二次确认。

本地化: 配置文件以明文 JSON 存储在 AppData/Roaming/DocLinker，方便用户手动迁移。

6. 异常处理
文件冲突: 扫描时遇到无法读取的文件（权限限制），静默跳过并在日志记录。

命令超时: 执行命令超过 30s 未响应，提供“强制结束进程”按钮。

索引重建: 提供一键“Rebuild Index”功能，应对数据库损坏或索引偏差。