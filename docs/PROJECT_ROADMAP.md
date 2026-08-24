# PlotApp 当前执行路线图

- 更新日期：`2026-08-24`
- 总体状态：`M3-M7.5_AUTOMATION_COMPLETE / PUBLIC_RELEASE_PENDING_AUTHORIZATION / REAL_SAMPLES_PENDING`
- Model Handoff：`paused`，文件保留；本路线图和 `STATUS.md` 负责普通协作期间的进度记录。
- 当前自动化基线：本机当前 Python `279 passed`；CI run `32681539816` 中 Python 3.10–3.14 各
  `279 passed`，原生 Ubuntu/macOS 各 `279 passed`，Windows `278 passed, 1 skipped`。三系统入口强制
  自举 uv 和托管 CPython 3.14.7，`pip check`、Qt、I/O、绘图、
  导出和 67 SVG 验证通过。M2 真实仪器文件验证仍延期到发布前。

## 总目标

在不制作独立 App 安装包的前提下，把 PlotApp 逐步整理成数据正确、撤销省内存、GUI 不易卡顿、
依赖体积可解释，并可在 Windows、macOS、Linux 一键建立隔离环境、验证和启动的 Python 桌面程序。

## 执行与记录规则

1. 同一时间只激活一个阶段；后续阶段是路线图，不是自动实施授权。
2. 状态只使用 `PLANNED`、`IN_PROGRESS`、`PENDING_VALIDATION`、`BLOCKED`、`COMPLETE`。
3. 每阶段先增加能失败的测试或基准，再修改生产代码；不得用现有绿测代替缺陷反例。
4. 每阶段完成时更新本文件和 `STATUS.md`，并在对应 `reports/M*-*.md` 记录：修改范围、行为变化、
   命令、退出码、测试数、耗时/内存/体积、残余风险和清理结果。
5. `STATUS.md` 始终只保留一个明确的 Next action。未完成事项必须转入后续阶段或风险清单。
6. 保留工作树中既有用户/M1/M2/M3 修改；不重置、不批量格式化、不顺手修改阶段外代码。
7. 阶段验收失败时先判断根因类别；同一根不变量二次失败必须扩大矩阵，不继续逐例补丁。

## 阶段 A — M3.2 共享输入转换闭环 — COMPLETE

目标：完成无 Qt 五类处理核心，确保任何输入只能形成独立的一维 real float 副本，或返回稳定
`ProcessingError`，不得泄漏异常/warning或静默丢失复数虚部。

工作：

1. 建立 0D、1D、2D、ragged，real/object/nullable/complex，以及五类 API 所有 x/y 位置的失败矩阵。
2. 把 array 构造、shape 判断、Pandas coercion、complex 拒绝和 float cast 纳入一个共享错误边界。
3. 复跑 correction 1 的参数、NaN/Inf、极值、有限 legacy、GUI 包装和八处 history owner 门禁。

验收：新增反例无裸异常或 warning；合法 real 结果和输入不变；本机/clean 3.12 完整回归、编译、wheel、
`pip check`、offscreen 启动和差异检查通过。完成后标记 `M3.2 COMPLETE`，再规划阶段 B。

完成记录：失败优先矩阵为 `16 failed, 70 passed`；共享边界统一处理 array 构造、ragged、nested object、
complex、Pandas coercion 和 float cast。最终处理 86 项、M3.2 定向 93 项、本机完整 181 项通过；clean
CPython 3.12.13 installed-wheel 为 `181 passed, 6 warnings in 50.09s`，`pip check`、三个模块内容和
offscreen 启动通过。八处 legacy history owner 未变，构建/环境/缓存已清理。

## 阶段 B — M3.3 差量历史纯核心 — COMPLETE

目标：用可测试的差量命令代替全量深拷贝，为 undo/redo 建立无 Qt 核心。

工作：实现列修改、删除行、删除文件和组合命令；验证重复路径、索引/dtype、十步 undo/redo、历史淘汰、
undo 后新操作和发布失败回滚。本阶段不接 GUI。

验收：所有状态往返精确相等；redo 不重新运行算法；命令提供 `payload_bytes`；纯核心无 Qt/Matplotlib。

完成记录：失败优先收集因核心模块缺失退出；新增 sidecar entry identity 和四类差量命令。本机核心
15 项、定向 108 项、完整 196 项通过；clean CPython 3.12.14 installed-wheel 为
`196 passed, 6 warnings in 58.74s`，`pip check`、wheel 四模块和 offscreen 启动通过。原八处 GUI
快照 owner 未变；构建产物、缓存和临时环境已清理。

## 阶段 C — M3.4 历史 GUI 集成 — COMPLETE

目标：迁移当前八处 `deepcopy(self.loaded_files)`，增加 redo，并让取消、失败和 no-op 不占历史槽。

工作：按一次用户操作生成一个组合命令；成功后才发布；失败中途完整回滚；保留下拉框、重绘、对话框、
部分成功摘要和 `loaded_files` 公共结构。

验收：八处全量快照归零；单点/框选/删线/五类处理均可 undo/redo；取消和全失败不改变历史。

完成记录：GUI 失败优先 5 项全部红；迁移后八处快照归零，新增 redo、统一刷新、会话 reset、按位置
删行及成功后原子发布。重复索引、重复路径、部分成功、取消、全失败、no-op 和 undo/redo 通过。

## 阶段 D — M3.5 内存与 M3 验收 — COMPLETE

目标：证明新历史正确且显著省内存，正式关闭 M3。

验收：沿用 4 文件 × 250,000 行 × 8 列 × 10 步基准，新历史 payload 不超过旧
`640,005,280 bytes` 的 35%；连续十步 undo/redo 精确恢复；clean 3.12 installed-wheel 全绿。

完成记录：新载荷 `160,010,560 bytes`，比率 `25.0014%`；十步 undo/redo 精确往返，耗时
`28.587972 s`，tracemalloc peak `306,337,085 bytes`。本机完整 `203 passed in 2.38s`；clean
CPython 3.12.14 installed-wheel `203 passed, 6 warnings in 51.14s`，wheel 四模块/67 SVG、`pip check`、
编译及 offscreen 启动通过。

## 阶段 E — M4 GUI、流畅性与可维护性 — COMPLETE

按以下顺序拆分，避免同时改变线程和界面结构：

1. M4.1（COMPLETE）：已记录启动、大文件导入、处理、拟合、平移/框选和重绘基准；清理 6 个 Qt
   弃用警告。正式 100k 行中位数见 `reports/M4-gui-performance.md`。
2. M4.2（COMPLETE）：导出列/冲突对话框和处理文件选择公共构建已移到 `instplot_dialogs.py`；主窗口
   保留兼容薄适配，布局和回调不变。完整 warnings-as-error 为 208 passed。
3. M4.3（COMPLETE）：任务控制器、异步导入、统一拟合核心及不少于 250,000 点的后台拟合已完成；
   Qt 心跳、主线程发布、取消丢弃、失败和队列收尾通过。
4. M4.4（COMPLETE）：16ms 交互绘图合并使 100k 平移事件主线程阻塞从 36.027ms 降至 0.167ms；
   完整重绘仍为 62.355ms。矩形扫描仅 0.612ms，按证据不增加数值缓存失效复杂度。
5. M4.5（COMPLETE）：跨系统轮转日志、不可写目录回退、未捕获异常记录、可复制错误详情及关闭诊断
   已完成；最终本机/clean installed-wheel 均为 238 passed。

验收：固定大数据集上无明显性能回退；耗时任务不长期阻塞主线程；关闭窗口能安全取消任务；完整功能
和 GUI 冒烟测试通过。每项必须给出前后耗时、峰值内存或帧延迟，不能只写“更流畅”。

## 阶段 F — M5 依赖和安装体积 — COMPLETE

目标：在不改变功能的情况下减少不必要依赖和安装资源。

工作：macOS 已完成 `PySide6-Essentials` 迁移、通用锁更新、依赖用途、wheel 资源、环境落盘和启动内存
验证；环境从 1,487,064 降至 607,412 KiB（-59.15%），本机/Essentials wheel 均为 238 passed。
Windows/Linux Essentials 功能验证已随 M6 原生矩阵通过；M7 原生 CI 又完成同版本 Essentials/full
PySide6 隔离体积比较：Linux、macOS、Windows 分别节省 `40.54%`、`57.97%`、`44.25%`。不得为了体积
擅自替换 Pandas/SciPy/Matplotlib。

验收：三系统完整功能通过并给出前后体积；若 Essentials 不兼容，记录证据后保留完整 PySide6。

## 阶段 G — M6 三系统一键安装 — COMPLETE

目标：不制作 `.app`、MSI、DMG、AppImage 或独立 exe；用户运行系统对应脚本即可安装和启动。

工作：实现标准库共享安装核心及 Windows `.bat`、macOS `.command`、Linux `.sh`；创建项目内 `.venv`；
支持已有 Python、受控 Python/uv、dry-run、幂等、repair、代理/离线提示、日志和启动器。

完成结果：上述核心和三个入口均已实现，后续日志碰撞回归也已进入矩阵。当前 GitHub 原生矩阵全绿：
Ubuntu/macOS 各 254 项，Windows 253 项加一个 POSIX 可执行位跳过。三系统均完成首次安装、健康重复、缺包识别、
无 repair 非零失败、显式 repair、哈希锁、`pip check` 和安装后 I/O/绘图/资源冒烟。Linux CI 明确安装
宿主 `libegl1`；项目安装器保持不提权、不静默安装系统软件。

验收：三系统干净环境首次/重复/修复安装；中文和空格路径；失败非零退出码；安装后 TXT/XLSX 导入、
绘图及 PNG/CSV/XLSX 导出冒烟测试。不得默认请求管理员权限或修改系统 Python。

## 阶段 H — M7 发布验收与真实样本 — PENDING_USER_VALIDATION

目标：形成可交付版本、CI、用户说明和风险清单。

工作：重建锁文件；普通用户安装复核；补充真实仪器 TXT/DAT/VSM/XLS/XLSX 样本验证；修订 README
中的平台前置条件、磁盘、错误排查、版本和联系方式；补充 Windows/Linux 对照体积。

当前结果：已增加 `docs/M7_RELEASE_PLAN.md`、`reports/M7-release.md` 和标准库发布验证器；README
占位链接、平台、磁盘、Linux EGL、故障排查及格式边界已修订；固定 `uv 0.12.5` 本地字节级重建锁通过。
固定 `uv 0.12.5` 字节级锁重建及最新三系统 CI run `32678530521` 已通过；Ubuntu/macOS 各 260 项，Windows
259 项加一个 POSIX-only 跳过。三系统同版本体积证据已记录并关闭 M5。macOS 正式启动器已在原生
Cocoa 桌面会话启动，并修复由此发现的 Qt 6 弃用警告和缺失字体重复日志。真实仪器文件仍保持待验证，
不能宣称 M7 完全完成。

验收：三系统 CI 和安装矩阵全绿；真实样本列名/列数/值正确；锁文件可重建；限制与磁盘需求准确。
若发布前仍没有真实仪器文件，则状态必须保留 `PENDING_USER_VALIDATION`，不能宣称完全发布验收。

## 当前风险与决策点

- M2 自动化 I/O 已接受，但真实仪器文件尚未验证。
- 三系统 CI 已通过，macOS 原生桌面启动已复核；Linux 入口会预检 `libEGL.so.1` 并按发行版提示系统包。
- M7.4 已把支持范围扩为 CPython 3.10–3.14，并让三系统入口安全自举项目内 uv 和托管 Python；原生
  三系统与五版本 CI 已通过。
- `.xls` 支持保留；Linux 当前不生成 `.desktop`。
- 当前历史内存基线很高，但必须先完成 M3.2 数值边界，再修改 history，避免两个根不变量混改。

## 唯一下一步

用户确认是否创建公开 GitHub Release v1.0.0，并上传三套已验证安装包。
