# PlotApp 优化与跨平台安装实施计划

> [!NOTE]
> 这是旧 Python 版的历史计划，内容中的“不制作独立 App 安装包”不再是当前产品方向。
> 普通用户请查看 [InstPlot Lite 下载与使用说明](../README.md)，当前开发状态请查看
> [Lite 状态页](../instplot-lite/docs/STATUS.md)。

## Objective

在不制作独立 App 安装包的前提下，将 PlotApp 整理为结构清晰、运行稳定、交互流畅、
依赖体积可控的 Python 桌面程序，并为 Windows、macOS、Linux 提供可重复、可诊断的
一键安装与启动方式。

## Overall delivery goal

最终交付物是一个不要求管理员权限、不修改系统 Python、也不打包成独立 App 的 PlotApp：
Windows、macOS、Linux 用户分别运行对应的一键脚本，即可创建隔离环境、安装、验证并启动。
在此之前，项目必须先完成数据正确性、算法与撤销、GUI 流畅性、依赖体积和发布验证，避免把
现有运行问题带入安装流程。

完成标准：

- 正确：合法文件的列名、列数和数据值一致；算法正常输入结果兼容；错误不得静默吞掉。
- 稳定：核心模块可脱离 Qt 测试，完整回归在干净 Python 环境通过。
- 流畅：大文件读取、拟合和耗时处理不长期阻塞 GUI，交互绘制有可复现基准。
- 可控：依赖和安装体积有三系统实测数据，不携带非运行资源。
- 易安装：三系统各有一键入口，支持重复安装、空格/中文路径、失败日志和修复安装。
- 可交付：CI、版本、用户说明、故障排查和发布证据完整。

## Stage status recording rule

当前日常执行顺序和精简状态见 `docs/PROJECT_ROADMAP.md`；本文件继续保存完整审计、阶段合同和历史证据。
若两者冲突，以 `STATUS.md` 的当前阶段和 `PROJECT_ROADMAP.md` 的顺序为准，并在下一次计划更新中消除
冲突。

每个阶段开始前标记 `PLANNED` 或 `IN_PROGRESS`；只有验收证据齐全后才能标记 `COMPLETE`。
若受外部样本、系统或领域决定限制，标记 `BLOCKED` 或 `PENDING_USER_VALIDATION`，不得写成完成。

每次阶段结束必须同步记录：

1. 在 `reports/M*-*.md` 写明完成范围、修改文件、行为变化、已知限制和清理结果；
2. 记录实际验证命令、退出码、测试数量、耗时、内存或安装体积，不使用笼统的“测试通过”；
3. 更新 `STATUS.md` 的当前状态、已完成内容、约束和唯一下一动作；
4. 更新本计划对应里程碑状态；发生模型/角色切换或激活下一阶段时，再同步 `MODEL_HANDOFF.md`；
5. 未完成项进入下一阶段清单，不因阶段切换而丢失，也不得把后续路线图当作自动实施授权。

当前阶段看板：M0/M1 已完成；M2 已自动化接受、真实文件验证延期到 M7；M3.1 已接受，M3.2 正在
共享输入转换返工规划，M3.3～M7 未开始。下一实施阶段是 `PROJECT_ROADMAP.md` 的阶段 A。

## Execution and handoff rule

`.cursor/rules/` 中的普通项目规则适用。Model Handoff Protocol 当前暂停且文件保留；暂停期间
`STATUS.md` 是实时状态看板，`docs/PROJECT_ROADMAP.md` 是执行顺序。只有用户明确批准的里程碑可以
进入实施；不得把后续路线图视为自动授权。
实施时必须保留当前工作区中已有的 `InstPlot.py` 未提交修改，先确认基线再改动。

## Audit baseline

### Repository and runtime

- 主程序：[InstPlot.py](../InstPlot.py)，约 6,500 行。
- `PlotApp` 单个类约 6,100 行；最长方法 `open_publish_dialog()` 约 2,240 行。
- 当前没有自动化测试、打包元数据、依赖锁文件或跨系统 CI。
- README 当前只说明 `pip install -r requirements.txt` 和直接运行脚本，无法保证干净机器
  一次安装成功。
- 默认 Homebrew Python 3.13 未安装项目依赖，直接启动失败。
- 已安装依赖的 Conda Python 3.13 无界面启动成功；实测进程启动约 1.1～1.8 秒，
  最大常驻内存约 277 MB。该数据只作为本机基线，不代表三系统验收结果。

### Confirmed issues

#### P0 — 安装和运行可靠性

1. `requirements.txt` 没有声明代码实际使用的 `qtawesome`、`openpyxl` 和旧 `.xls`
   读取所需的 `xlrd`。
2. 依赖只有过宽的最低版本，无法复现安装结果，也可能在旧 Python 上解析出不兼容组合。
3. `denoise_data()` 在数据点过少或 `polyorder >= window_length` 时会抛出
   `ValueError`；已用 1 行数据和 3 行/3 阶参数复现。
4. 文件解析器仍有需要回归覆盖的边界：单行数值文件、无表头文件、表头前空行、注释、
   GBK/Big5、尾部分隔符、列数不一致、VSM 固定格式和导出后重新导入。
5. 源码编译出现 5 个 `\pi` 非法转义警告。
6. 约 123 处宽泛异常捕获和约 55 处直接 `print()`，部分失败可能被静默吞掉，用户无法
   获得可操作的错误信息。

#### P1 — 流畅性和内存

1. 文本导入、Excel 读取、拟合和多数数据处理都运行在 GUI 主线程，大文件会冻结界面。
2. 文本导入可能按多个编码重复完整读取文件，并同时保留原文、分行副本、标准化文本和
   DataFrame，峰值内存偏高。
3. 撤销历史通过 `copy.deepcopy(self.loaded_files)` 保存最多 10 份全量数据，大数据场景
   会成倍放大内存占用。
4. 鼠标平移和矩形框选择在移动事件中同步调用 `canvas.draw()`，高点数曲线容易掉帧。
5. 启动时立即创建并绘制 7 条示例曲线，增加首屏时间；科学计算和部分绘图模块也可以
   延迟导入。
6. 最近点选择每次点击都对所有曲线重新做数值转换和坐标变换，可缓存有效数值与显示坐标。

#### P1 — 可维护性

1. GUI、数据解析、算法、绘图、状态、资源和安装逻辑集中在一个文件中。
2. 多个超长对话框方法包含大量嵌套回调，难以独立测试和复用。
3. 主题、按钮、列选择、历史记录和错误展示存在重复逻辑。
4. 数据处理函数对 NaN、无穷值、非数值列、短数组和空选区的契约没有统一定义。
5. 没有结构化日志、崩溃日志位置和“复制错误详情”入口。

#### P2 — 安装体积和仓库体积

1. 当前完整 `PySide6` 会安装 Essentials 和 Addons；本机环境中 Addons 约 817 MiB，
   而项目目前只使用 Core、Gui、Widgets，初步可改用 `PySide6-Essentials`。
2. `ReadMe图片/` 约 7.6 MiB，只用于文档，不应复制进运行安装目录。
3. `Pillow` 已由 Matplotlib 间接依赖；确认无直接使用后可不再作为直接依赖维护。
4. 即使去除 Qt Addons，Qt、NumPy、Pandas、Matplotlib、SciPy 的独立环境预计仍需
   约 500～700 MiB。除非替换技术栈，否则不应承诺几十 MiB 的安装体积。

### Recommendations

- 先补回归测试，再拆分代码；避免把现有导入修复和重构混在一起后无法判断行为变化。
- 使用 `pyproject.toml` 作为依赖和命令入口的单一来源，为三个系统生成经过验证的锁文件。
- 目标 Python 版本先统一为 3.12；验证后再扩大到 3.13，不继续承诺未经测试的 Python 3.8。
- 以 `PySide6-Essentials` 代替完整 `PySide6`，但必须在三系统完成 Qt、Matplotlib、
  QtAwesome 和 SVG 图标回归后才能冻结。
- 把大文件读取和耗时算法放入 Qt 工作线程；所有窗口控件更新仍只在主线程执行。
- 撤销改为命令/差量记录：删除操作保存行和位置，列变换只保存受影响列，不复制所有文件。
- 平移、框选采用 `draw_idle()`、30～60 FPS 节流或 Matplotlib blitting；滚轮缩放合并事件。
- 错误处理分为用户可修复错误、数据格式错误和程序缺陷；状态栏展示摘要，日志记录完整堆栈。
- 共享安装核心沿用现有模型切换安装器的路径校验、dry-run、幂等和不覆盖设计。

## Target structure

```text
PlotApp/
├── pyproject.toml
├── requirements/                 # 平台锁文件或约束文件
├── src/instplot/
│   ├── __main__.py               # python -m instplot
│   ├── app.py                    # QApplication 与主窗口装配
│   ├── state.py                  # 应用状态和设置模型
│   ├── data_io.py                # TXT/CSV/DAT/Excel 导入导出
│   ├── processing.py             # 去噪、归一化、去背底等纯函数
│   ├── plotting.py               # 主图和出版绘图
│   ├── history.py                # 差量撤销命令
│   ├── workers.py                # 后台任务和取消机制
│   ├── dialogs/                  # 输入、拟合、出版、筛选等对话框
│   └── resources/                # 运行时图标和样式
├── scripts/
│   ├── install.py                # 共享安装核心
│   ├── install_windows.bat
│   ├── install_macos.command
│   ├── install_linux.sh
│   └── verify_install.py
└── tests/
    ├── fixtures/                 # 小型、匿名、跨编码样本
    ├── test_data_io.py
    ├── test_processing.py
    ├── test_export_roundtrip.py
    ├── test_installer.py
    └── test_smoke_gui.py
```

该结构是目标方向，不要求一次性大重写。每个里程碑应保持程序可启动并可回退。

## Installation design

### Shared installer

`scripts/install.py` 只使用 Python 标准库，并负责：

1. 检测 OS、CPU、Python 和项目路径。
2. 默认 dry-run；`--apply` 后才创建环境或启动安装。
3. 创建项目内 `.venv`，不修改系统 Python 和全局 site-packages。
4. 安装与当前系统匹配的锁定依赖。
5. 校验导入、资源路径、Qt 后端和无界面窗口初始化。
6. 重复运行时报告 `identical / healthy / repair-needed / conflict`。
7. 不覆盖用户修改；升级和修复必须显式选择。
8. 将日志写入项目内可清理目录，并在失败时输出下一步命令。

### OS entry points

- Windows：双击 `install_windows.bat`；优先使用 `py` 启动器，缺少合适 Python 时引导
  安装受控运行时。生成 `run_instplot.bat`。
- macOS：双击 `install_macos.command`；处理 Gatekeeper/执行权限说明，生成
  `run_instplot.command`。
- Linux：运行或双击 `install_linux.sh`；启动前检查 Qt 所需系统库和显示服务，生成
  `run_instplot.sh`，可选生成用户级 `.desktop`，但不默认修改桌面环境。
- 推荐的无预装 Python 路线：包装脚本安装或调用 `uv`，由 `uv` 管理 Python 3.12 和
  `.venv`。必须提供下载来源、校验和、离线/代理失败提示以及仅使用已有 Python 的备用模式。

## Milestones

### M0 — 审计与计划 — COMPLETE

- Goal：记录当前结构、依赖、运行基线、风险和跨平台安装方向。
- Non-goals：不修改业务代码，不改变现有未提交修改。
- Acceptance：计划书、状态和交接文件内容一致；模型切换检查器通过。
- Evidence：本文件“Audit baseline”；`STATUS.md`；`MODEL_HANDOFF.md`。

### M1 — 回归基线与依赖修复 — COMPLETE

- Goal：建立能保护现有行为的测试基线，并让干净环境可靠安装和启动。
- Scope：`pyproject.toml`、依赖/约束文件、测试目录、最小运行修复、README。
- Non-goals：不拆分大型 GUI，不重做交互，不改变算法结果。
- Work：
  1. 冻结当前 `InstPlot.py` 改动基线并记录样本行为。
  2. 增加导入、导出回环、算法边界和无界面启动测试。
  3. 补齐 `qtawesome`、`openpyxl`、`xlrd`，确认并清理直接依赖。
  4. 修复短数据去噪、非法转义和明确可复现的导入边界问题。
  5. 定义 Python 3.12 和依赖版本范围，生成初始锁定方案。
- Acceptance：所有测试通过；`python -m compileall` 无项目警告；新虚拟环境能安装并完成
  无界面启动；导出文件可以重新导入且列结构一致。
- Stop conditions：现有未提交修改来源不明；修复会改变科研算法语义；样本格式无法确认。
- Evidence：`reports/M1-validation.md`、`reports/M1-review.md` 和测试输出摘要。
- Review gate：inline Reviewer 已返回 `REVIEW_M1: ACCEPT_M1`；六项合法空白变体、两项列数
  异常、完整 20 项测试及干净 CPython 3.12.14 证据均通过。
- Frozen decisions for M1：
  1. 现有 `InstPlot.py` 修改作为受保护的既有工作，不允许回退、覆盖或顺手重写。
  2. 初始可复现环境以 Python 3.12 为目标；Python 3.11/3.13 扩展支持留待验证后决定。
  3. 保留当前 `.xls` 功能，因此 M1 声明 `xlrd`；是否移除旧格式不在本阶段讨论。
  4. M1 继续使用完整 `PySide6`；切换 `PySide6-Essentials` 属于 M5，避免依赖修复与体积
     优化混合。
  5. 只修复已复现的短数据去噪异常和源码警告，不改变正常长度数据的算法输出。
  6. M1 不实现 `uv`、三系统包装器或一键安装；这些内容属于 M6。

### M2 — 数据 I/O 模块化 — AUTOMATED_ACCEPTED / REAL_FILE_VALIDATION_DEFERRED

- Goal：把解析和导出从 GUI 中提取为可测试的纯接口。
- Scope：TXT/CSV/DAT/VSM/XLS/XLSX、编码和表头检测、导出回环。
- Non-goals：不改出版绘图和数据处理 UI。
- Work：定义 `ImportResult`、格式检测器、解析器和用户错误类型；减少重复全文件读取；保留
  原始行号和诊断信息。
- Acceptance：固定样本全部通过；错误包含文件、编码、分隔符和原始行号；100 MB 合成文本
  峰值内存和耗时形成可比较基线。
- Stop conditions：不同仪器格式存在冲突且无法从样本判定。
- Evidence：`reports/M2-data-io.md`。
- Execution contract：`docs/M2_DATA_IO_CONTRACT.md`。合同冻结新核心模块边界、格式与错误不变量、
  四个顺序阶段、允许路径、100 MiB 前后基准和停止条件；第一动作是只做 M2.1 特征测试与前置
  基准，不移动生产逻辑。
- Execution progress：M2.1 已完成。新增 DAT/VSM、XLS/XLSX、追加、物理行号和源表无副作用测试；
  当前完整套件为 28 passed、1 xfailed，100 MiB 前置中位值为 89.131 s 与 2.40 GiB
  `tracemalloc` peak。详见 `reports/M2-data-io.md`；下一步为 M2.2 导入核心。
- Execution progress：M2.2 已完成。`instplot_io.py` 已承接导入、格式元数据与结构化错误，
  `PlotApp.load_file` 为薄 GUI 适配层；完整套件为 36 passed。下一步为 M2.3 导出核心。
- Execution progress：M2.3 已完成。导出准备、CSV/TXT/XLSX 写入、追加校验和 sheet 命名已迁入
  `instplot_io.py`，`PlotApp.export_data` 为薄适配层；完整套件为 42 passed。下一步为 M2.4 验证。
- Execution progress：M2.4 已完成，待 inline review。100 MiB 后置中位耗时为 88.657 s（前置的
  99.47%），峰值为 2.40 GiB（不高于前置）；干净 CPython 3.12.13 安装、wheel 内容、完整 42 项
  测试、编译和无界面启动均通过。详见 `reports/M2-data-io.md`。
- Review result：M2 inline review 退回返工。空/仅空白文本会从核心与 GUI 泄漏裸 `ValueError`；
  合同第 5 节的永久测试矩阵和第 8 节的报告证据仍不完整。只允许修复 M2 并补齐证据，不进入 M3。
- Rework progress：返工已完成并等待 inline review。空/空白/BOM-only 统一为 `empty_file`
  `DataIOError`；合同矩阵扩展后完整套件 61 passed；100 MiB 返工后中位 88.196 s、峰值
  2,578,612,595 bytes，clean CPython 3.12.13 安装、wheel 和无界面启动全部通过。
- Second review result：独立 capable review 发现前置注释 + 全字符串 CSV/分号及引号字段会静默
  退化为单列；M2 再次进入高风险返工。必须统一探测、列宽校验和 Pandas 的逻辑字段语义；M3 草案
  继续保持 `NOT_ACTIVE`。
- Second rework progress：失败优先组合矩阵初次得到 8 个具名失败；统一评论候选与 CSV 引号字段
  语义后完整套件为 70 passed。最终 100 MiB 中位 `80.856231 s / 2,578,612,579 bytes`，clean
  CPython 3.12.13 已安装 wheel 的 70 项测试、编译、`pip check`、67 SVG 和 offscreen 启动均通过。
  当前只等待新的 capable inline review，不得启动 M3。
- Third review result：独立 capable review 发现分号/tab 文件的引号字段含逗号时，fallback 会先以
  错误的逗号候选得到相同列宽并静默错列。M2 保持高风险返工；必须补齐跨候选分隔符碰撞矩阵，并
  让分隔符发现只依据引号外字符。不得只重排候选，M3 继续保持 `NOT_ACTIVE`。
- Third rework progress：5 个跨候选失败优先组合和 1 个未引用字面双引号相邻变体已闭环；fallback
  现在只尝试引号外真实存在的候选字符，后续仍共享 CSV 字段语义。完整套件 76 passed，clean
  CPython 3.12.13 installed-wheel 验证通过；等待新的 capable inline review，M3 仍未激活。
- Fourth review result：独立 capable review 发现错误候选字符若也出现在未引用字段里，“引号外存在”
  过滤仍按顺序误选；6 个跨方言样本中 3 个静默错列。M2 保持高风险返工，必须按完整 CSV 方言结构
  证据选择候选，不得只检查存在性、调序或按扩展名强制；M3 继续 `NOT_ACTIVE`。
- Direct remediation：按用户指示暂停合同往返，改用多记录完整方言评分并对真实并列返回
  `ambiguous_separator`；comments-only 裸错误及额外整文件行列表同时修复。完整 86 项、最终 100 MiB
  和 clean CPython 3.12 installed-wheel 全部通过。实现完成；M3 仍未开始。
- Automated substitute acceptance：用户当前无法提供真实仪器文件，已批准用黄金样本、固定种子
  1,000 组随机方言对照、导出再导入和 offscreen GUI 列映射作为 M2 收尾门槛。通过后 M2 标记
  `AUTOMATED_ACCEPTED / REAL_FILE_VALIDATION_DEFERRED`，真实仪器样本验证转入 M7 发布前风险清单。

### M3 — 算法、状态和撤销重构 — COMPLETE

- Goal：数据处理函数具备清晰契约，撤销不再全量复制所有数据。
- Scope：去噪、局部处理、对称、归一化、去背底、删除点、历史记录。
- Non-goals：不改变经确认的正常输入数值结果。
- Work：统一 NaN/Inf/短数组规则；用纯函数和类型化结果；引入命令/差量撤销。
- Acceptance：算法单元测试和基准通过；10 次撤销的额外内存不再接近 10 份全量数据；
  撤销/重做结果与原始数据一致。
- Stop conditions：算法预期需要领域决策；历史格式影响用户数据兼容。
- Evidence：`reports/M3-processing-history.md`。
- Detailed plan：`docs/M3_PROCESSING_HISTORY_CONTRACT_DRAFT.md`。已完成代码现状核对、五阶段拆分、
  数值/历史高风险矩阵、35% 差量历史内存门槛和验收命令；M3.1 已独立接受，M3.2 首次审核缺陷已按
  根因批次修复；共享 conversion matrix 最终闭环，本机/clean 3.12 均为 181 passed。当前为
  `M3 COMPLETE`。
- Current boundary：M3.4 GUI 迁移及 M3.5 内存/安装验收均已关闭；下一步只规划 M4.1 测量基准和
  Qt 弃用警告清理，不立即混入线程或结构拆分。
- M3.2 result：新增 `instplot_processing.py` 的五类纯处理 API 和稳定错误，旧入口改为薄包装，背景计算
  接入核心；定向 35 项、本机完整 123 项及 clean CPython 3.12.13 installed-wheel 123 项均通过，八处
  历史快照所有者保持不变。独立审核用未覆盖反例证明二维输入泄漏裸异常、窗口类型/奇偶性静默归一、
  NaN 区间误判及有限极值 RuntimeWarning；合并返工已补 35 项矩阵、严格校验和安全数值路径，本机及
  clean CPython 3.12 installed-wheel 均为 158 passed。第二次审核以 ragged 嵌套和一维复数数组复现同一
  shape/type 根不变量失败；扩大矩阵后以单一转换边界闭环，最终本机/clean 3.12 均为 181 passed，
  M3.2 标记 COMPLETE。
- M3.3 result：新增无 Qt `instplot_history.py`，以 sidecar token 区分重复路径和共享 DataFrame 条目，
  实现列补丁、删行、删文件、组合命令及十步 undo/redo；本机和 clean 3.12 installed-wheel 均为
  196 passed，原八处 GUI 快照保持不变。GUI 接入属于 M3.4。
- M3.4/M3.5 result：八处 GUI 快照迁移为差量命令并增加 redo；取消、全失败、no-op 不入栈，重复
  索引按位置处理。正式十步载荷为旧基线的 25.00%，十步往返通过；本机和 clean 3.12 wheel 均为
  203 passed，M3 正式关闭。

### M4 — GUI 拆分与交互优化 — PLANNED

- Goal：拆分超长主类并改善大数据操作流畅性。
- Scope：对话框、绘图控制器、后台工作线程、鼠标交互和错误展示。
- Non-goals：不进行全面视觉重新设计。
- Work：分离对话框；耗时任务后台化并可取消；缓存数值列；绘制节流；延迟示例图和重模块。
- Acceptance：主线程不执行完整大文件解析或拟合；交互基准无明显回退；关闭窗口时后台任务安全
  取消；异常能在 UI 和日志中定位。
- Stop conditions：线程模型可能导致数据竞争或破坏 Matplotlib/Qt 主线程约束。
- Evidence：`reports/M4-gui-performance.md`。

### M5 — 体积优化与可复现依赖 — PLANNED

- Goal：去除未使用的 Qt Addons 和非运行资源，形成可复现安装集合。
- Scope：依赖、资源清单、锁文件和安装目录。
- Non-goals：不以替换 Qt、Pandas 或 SciPy 为默认目标。
- Work：验证 `PySide6-Essentials`；排除 README 图片；检查可选 Excel 依赖是否应分组；记录
  三系统下载和落盘体积。
- Acceptance：三系统所有功能测试通过；安装目录不包含文档截图；体积报告列出优化前后数据；
  若 Essentials 不兼容则记录证据并回退完整 PySide6。
- Stop conditions：核心功能依赖 Addons；拆除 SciPy 会改变算法结果。
- Evidence：`reports/M5-footprint.md`。

### M6 — 三系统一键安装 — PLANNED

- Goal：用户通过对应系统入口即可创建隔离环境、安装、验证和启动 PlotApp。
- Scope：共享安装器、Windows/macOS/Linux 包装器、启动器、日志和安装测试。
- Non-goals：不制作 `.app`、`.exe`、AppImage、MSI、DMG 或系统级安装包；不默认写系统目录。
- Work：沿用已安装模型切换工具的安全设计，增加运行时和依赖管理；提供在线、已有 Python、
  代理失败和修复安装路径。
- Acceptance：
  - 每个系统在干净用户环境首次安装成功；
  - 重复安装不破坏环境；
  - 路径含空格和中文时可安装、启动；
  - 安装失败返回非零退出码并产生可读日志；
  - 安装后 TXT/XLSX 导入、绘图、PNG/CSV/XLSX 导出冒烟测试通过。
- Stop conditions：需要系统管理员权限、静默安装第三方系统软件或修改全局配置。
- Evidence：`reports/M6-install-matrix.md`。

### M7 — 发布验收与文档 — PLANNED

- Goal：形成可交付版本和面向普通用户的三系统说明。
- Scope：CI、README、故障排查、版本号、发布清单。
- Non-goals：不建立自动更新服务。
- Acceptance：Windows、macOS、Linux CI 全绿；所有锁文件可重建；安装步骤由非开发环境复核；
  当前限制和磁盘需求准确记录。
- Stop conditions：任一系统没有真实或 CI 环境可验证。
- Evidence：`reports/M7-release.md`。

## Ordered execution

1. M1：测试和依赖可靠性。
2. M2：数据 I/O 提取。
3. M3：算法与撤销。
4. M4：GUI 和性能。
5. M5：体积。
6. M6：跨平台安装。
7. M7：发布验收。

M1 未完成前不进行大规模拆分；M2/M3 未稳定前不引入后台线程；核心功能未通过三系统测试前
不冻结精简依赖。

## Global acceptance gates

- 功能：现有导入、绘图、处理、拟合、出版绘图和导出能力不得无说明丢失。
- 正确性：正常输入保持结果兼容；所有有意行为变化必须有测试和迁移说明。
- 稳定性：测试不得依赖开发者全局 Python；错误不得只输出到终端。
- 性能：每个优化里程碑记录同一数据集上的时间和内存，禁止只凭主观感受宣称变快。
- 安装：不修改全局 Python、不覆盖用户文件、不要求制作 App、不默认需要管理员权限。
- 跨平台：路径拼接使用 `pathlib`，处理空格、中文路径、换行符、编码和执行权限差异。
- 清理：临时文件、后台进程和测试环境有明确归属和清理结果。
- 证据：完成状态必须附命令、退出码、测试数量或报告路径。

## Risks and decisions requiring user approval

1. 是否冻结 Python 3.12，还是同时支持 3.11/3.13。
2. 是否必须保留旧 `.xls`；若不需要，可不安装 `xlrd`。
3. 安装脚本是否允许自动下载 `uv` 和受控 Python，还是只接受机器已有 Python。
4. Linux 是否需要生成 `.desktop` 启动入口。
5. 科研算法在 NaN、全负值、短数组情况下的预期结果。
6. 当前 `InstPlot.py` 未提交修改应先提交、拆分提交还是继续作为工作区基线。

## Deferred roadmap

1. 自动更新和版本迁移。
2. 插件化仪器格式解析器。
3. 大数据降采样、分块绘制或内存映射。
4. 项目会话保存与恢复。
5. 独立 App/安装包；用户当前明确不需要。

延后项目不是当前实施授权。开始任何里程碑前，应在 `MODEL_HANDOFF.md` 中建立对应合同并获得
用户明确批准。
