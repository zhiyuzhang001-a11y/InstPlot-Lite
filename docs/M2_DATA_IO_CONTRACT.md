# M2 数据 I/O 模块化执行合同

## 1. 目标与用户可见结果

M2 将文件解析、导出表构造和磁盘写入从 `PlotApp` 主窗口中提取到独立的
`instplot_io.py`。完成后，用户仍以相同方式打开、拖入和导出文件；支持的格式、合法数据结果、
列选择、角度转弧度和覆盖/追加选择保持一致。失败信息必须能指出文件、处理阶段，以及在适用时
指出编码、分隔符和原始物理行号。

本阶段解决的是边界清晰、行为可测和重复读取问题，不引入后台线程。大文件操作是否异步、进度条
和取消机制属于 M4。

## 2. 已确认的现状

- `InstPlot.py::PlotApp.load_file` 同时承担扩展名判断、VSM 识别、编码检测、表头/分隔符推断、
  行校验、Pandas 读取、列名清理、GUI 状态修改和异常展示。
- `InstPlot.py::PlotApp.export_data` 同时承担对话框、覆盖/追加决策、DataFrame 构造、弧度转换、
  sheet 命名、磁盘写入和异常展示。
- `open_file` 与 `dropEvent` 都调用 `load_file`；M2 后仍只保留一个 GUI 导入适配入口。
- M1 已有 20 项完整测试，其中 `tests/test_data_io.py` 覆盖文本导入边界和 CSV 导出回环；尚缺
  DAT/VSM、XLS/XLSX、追加、错误元数据和无 Qt 核心测试。
- 当前普通文本路径会进行检测样本读取、全文读取和规范化副本构造；M2 必须至少消除应用层重复
  打开/全文读取，且不能让 100 MiB 基准回退。

## 3. 冻结架构与接口语义

新增单一运行模块 `instplot_io.py`，不得依赖 Qt、Matplotlib 或 `PlotApp`。模块提供三层边界：

1. `read_data_file(path) -> ImportResult`
   - 接受 `str` 或 `os.PathLike`，返回 DataFrame 与已确认的格式、编码和分隔符元数据。
   - 支持 `.txt`、`.csv`、普通 `.dat`、VSM `.dat`、`.xls` 和 `.xlsx`。
   - 文本解析沿用 M1 已验收规则；不得把 GUI 控件或状态栏传入核心。
2. `prepare_export(...) -> ExportBundle`
   - 只根据源表、拟合数组、列选择、X 列和角度模式构造输出表或 Excel sheets。
   - 不打开文件、不显示对话框、不修改传入 DataFrame，也不接收 Matplotlib line 对象。
3. `write_export(path, bundle, mode) -> ExportResult`
   - `mode` 仅允许 `overwrite` 或 `append`，负责 CSV/TXT/XLSX 的实际写入和追加校验。
   - 写入层不做 GUI 决策；所有错误转换成统一的 `DataIOError`。

需要定义以下数据合同；具体私有辅助函数命名由 Implementer 决定：

- `ImportResult`：至少包含 `path`、`frame`、`format`、`encoding`、`separator`。
- `ExportSource`：源路径标签和 DataFrame。
- `FittedCurve`：名称、X 数组和 Y 数组；核心层不得依赖绘图库对象。
- `ExportBundle`：输出格式及单表或有序 sheets。
- `ExportResult`：目标路径、写入模式、数据行数和 sheet 名称摘要。
- `DataIOError`：至少包含 `path`、`operation`、稳定错误码、原始原因；文本解析错误额外包含
  `encoding`、`separator`、一基原始物理 `line_number`。不适用或尚未探测出的字段显式为
  `None`，展示文本使用“未知”，不得伪造值。

这些名称和边界属于合同。模块内检测器、解码器、行扫描器、清理器和 sheet 命名器可由
Implementer 自行拆分，只要不扩大公共接口。

## 4. 行为不变量

- 同一合法输入经核心接口和 GUI 入口得到相同列顺序、值、行数与列名；M1 的空白统一规则、
  表头前空行/注释、无表头单行、尾部分隔符兼容和真实列数异常拒绝全部保留。
- 原始行号以文件的物理第一行为 1；空行和注释仍计入行号。任何过滤或规范化不得改变错误定位。
- 普通文本文件在应用代码中最多执行一次全文字节读取；编码检测使用同一份有界前缀，Pandas
  从已解码内容或流读取，不再重新打开同一路径。Excel 和 VSM 由格式专用 reader 处理。
- `prepare_export` 对输入 DataFrame 和数组无副作用；调用前后对象内容、列顺序和索引不变。
- CSV/TXT 仍输出一张标准表，保留 `source_file`、`data_type` 元数据；UTF-8 BOM 和当前列选择、
  弧度转换语义不变。
- XLSX 仍按源文件生成 sheet，并处理非法字符、31 字符上限和重名；拟合数据保留独立 sheet。
- CSV/TXT 追加时按现有文件列顺序写入；列集合不一致必须在写入前失败，目标文件字节不变。
- GUI 保留文件对话框、取消、覆盖/追加确认、`loaded_files` 的 `(path, DataFrame)` 结构、下拉框
  更新和成功/失败状态栏；核心模块不得触发绘图或 Qt 事件。
- 不支持的扩展名、空文件、解码失败、解析失败、Excel 引擎失败和写入失败都必须成为可测试的
  `DataIOError`，GUI 不再依赖捕获裸 `Exception` 来推断错误类型。

## 5. 格式与变体覆盖

固定样本或确定性生成样本必须覆盖：

- TXT/CSV：UTF-8 BOM、GBK、逗号、分号、单空格、多空格、制表符、混合空白、空行、注释、
  无表头、多余尾部分隔符、列数异常和 Unicode/空格路径。
- 普通 DAT：按文本规则读取，且前十行不含 VSM 标记。
- VSM DAT：前十行含大小写变化的 VSM 标记，跳过 31 行元数据，取第 4、5 列并命名为
  `B (Oe)`、`M (emu)`；不足 31 行或列不足时返回具名错误。
- XLSX：第一张 sheet、Unicode 列名和数值保持；导出覆盖与追加生成合法且唯一的 sheet 名。
- XLS：使用仓库内最小固定二进制 fixture 验证真实 `xlrd` 路径。规划机已确认存在 `soffice`，
  可在 M2.1 生成 fixture；不增加 `xlwt` 或其他运行依赖。
- 导出：单源、多源、选列、空选列沿用当前“全部列”语义、角度转弧度、拟合曲线、CSV/TXT
  回环、XLSX 多 sheet、同结构追加和异结构拒绝。
- 边界：空文件、不支持扩展名、缺失文件、只读/不可写目标、非法 Excel sheet 名、重复基名，
  以及错误行前存在空行和注释的物理行号。

## 6. 分阶段任务

### M2.1 — 行为固化与前置基准

1. 在不移动业务逻辑前，为 DAT/VSM、XLS/XLSX、错误元数据、导出追加和无副作用补充失败优先的
   特征测试；当前已正确的行为测试必须先通过，尚未实现的新错误合同可以明确失败。
2. 建立 `tests/fixtures/data_io/`；文本 fixture 优先由测试生成，真实 `.xls` 使用 `soffice`
   一次生成后固定入库，并记录来源命令。
3. 新建 `scripts/benchmark_data_io.py`，在独立临时目录确定性生成约 100 MiB、两列 UTF-8 CSV，
   文件生成不计入计时和内存；对预热后的 `PlotApp.load_file` 调用测量 3 次。
4. 把 Python、Pandas、操作系统、文件字节数、行列数、每次耗时、中位耗时和 `tracemalloc`
   峰值写入 `reports/M2-data-io.md`，形成移动代码前基线。

M2.1 只增加测试、fixture、基准脚本和报告，不创建核心模块、不移动生产逻辑。完成并验证后才进入
M2.2。

### M2.2 — 导入核心提取

1. 创建 `instplot_io.py` 的导入数据类型、`DataIOError` 和 `read_data_file`。
2. 按格式拆出 VSM、Excel、普通文本路径；普通文本复用一次字节读取完成编码检测、解码、表头与
   分隔符识别、行校验和 Pandas 解析。
3. 将既有 M1 GUI 测试下沉或参数化为无 Qt 核心测试，同时保留少量 GUI 集成测试证明状态更新。
4. 将 `PlotApp.load_file` 缩为适配层：调用核心、更新 `loaded_files`/下拉框/状态栏并展示
   `DataIOError`；不得残留 Pandas 读取、编码探测或分隔符推断。

### M2.3 — 导出核心提取

1. 实现 `prepare_export`，覆盖源数据、选列、弧度和拟合数组，证明输入对象不变。
2. 实现 `write_export`，覆盖 CSV/TXT 单表、XLSX 多 sheet、覆盖和追加。
3. 将 `PlotApp.export_data` 缩为对话框与用户决策适配层；GUI 负责把 Matplotlib line 转为
   `FittedCurve`，核心负责数据与文件规则。
4. 保留 M1 CSV 回环，并增加 TXT、XLSX 和追加回环。

### M2.4 — 集成、性能与交接

1. 运行全部核心、GUI 和既有回归测试；检查两个 GUI 方法中不存在 Pandas read/write、编码检测、
   分隔符推断或 sheet 命名实现。
2. 对同一 100 MiB fixture 和同一预热 GUI 调用复跑 3 次；后置中位耗时不得超过前置基线的
   110%，`tracemalloc` 峰值不得超过前置基线的 110%。本阶段记录相对结果，不设与硬件绑定的
   绝对秒数或内存值。
3. 在干净 CPython 3.12 环境安装项目，确认 wheel 中包含 `InstPlot`、`instplot_io` 和符号资源，
   再运行完整测试、`pip check` 和无界面启动。
4. 更新 `reports/M2-data-io.md`、主计划、状态和交接，返回 `EXECUTION_TO_REVIEW`，Review mode
   使用 `inline`。

## 7. 允许修改的路径

- `InstPlot.py`：仅导入/导出适配层及必要 import。
- `instplot_io.py`：新增数据 I/O 核心。
- `pyproject.toml`：仅把 `instplot_io` 纳入打包；不得改变依赖版本或新增依赖。
- `tests/conftest.py`、`tests/test_data_io.py`、`tests/test_io_core.py`、
  `tests/fixtures/data_io/`。
- `scripts/benchmark_data_io.py`。
- `reports/M2-data-io.md`、`docs/M2_DATA_IO_CONTRACT.md`、
  `docs/IMPLEMENTATION_PLAN.md`、`STATUS.md`、`MODEL_HANDOFF.md`。

不得修改绘图、算法、撤销、主题、布局、依赖锁、安装脚本或 symbol SVG。不得提交、推送、发布，
不得增加网络访问或后台进程。

## 8. 验收命令与证据

Implementer 必须在报告中记录实际解释器、退出码、测试数量、持续时间和产物路径：

```bash
QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q tests/test_io_core.py tests/test_data_io.py
QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q
/Users/zhiyu/miniconda3/bin/python -W error::SyntaxWarning -m py_compile InstPlot.py instplot_io.py scripts/benchmark_data_io.py
QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python scripts/benchmark_data_io.py --size-mib 100 --runs 3
/Users/zhiyu/miniconda3/bin/python .model-handoff/handoff.py check .
git diff --check
```

最终还必须在干净 CPython 3.12 虚拟环境执行安装、`pip check`、完整测试、编译检查和无界面窗口
初始化。临时环境、100 MiB fixture、wheel 和构建目录在记录结果后移入系统废纸篓或由临时目录
自动清理；仓库中只保留小型固定 fixture 和报告摘要。

`reports/M2-data-io.md` 至少记录：格式覆盖清单、错误字段示例、前后基准原始 3 次数据与比率、
完整命令结果、wheel 内容、失败尝试、偏差、临时资源清理和未验证平台。

## 9. 停止条件与决策边界

出现以下任一情况必须停止并切换为 `BLOCKED_TO_DECIDE`：

- 真实样本证明两个仪器格式规则冲突，且无法在扩展名/内容特征内稳定区分。
- 为支持格式必须增加或移除依赖、改变 Python 版本范围或更换 DataFrame 库。
- 需要改变用户可见列名、数值、sheet 布局、追加语义、`.xls` 支持或 `loaded_files` 结构。
- 100 MiB 后置基准超过任一 110% 上限，且在允许路径内无法修复。
- 发现既有未提交修改与 M2 目标区域重叠且无法安全区分。
- 需要原子替换、跨进程锁、后台线程、进度/取消或 M3～M7 范围才能继续。

## 10. 无真实文件时的自动化替代验收

用户远程操作且当前无法提供真实仪器文件，已批准以下替代收尾门槛：

1. 固定黄金样本逐项核对编码、分隔符、列名、列顺序和单元格值；
2. 使用标准库 `csv.writer` 和固定随机种子生成至少 1,000 个非歧义 comma、semicolon、tab 方言
   文件，以生成时的字段矩阵作为独立预期，禁止仅比较解析器自身输出；
3. 保留并执行 CSV/TXT/XLSX 导出再导入测试，核对列名、顺序和数据；
4. offscreen GUI 导入后，`loaded_files` DataFrame、X/Y 下拉框列名和顺序必须一致；
5. 真正结构歧义继续返回 `ambiguous_separator`，不得为了提高随机通过率恢复静默猜测。

随机生成必须确定性、无网络、无新依赖，并在 `tmp_path` 自动清理。若以上门槛、完整回归、编译、
`git diff --check` 和 handoff check 均通过，可将 M2 记为
`AUTOMATED_ACCEPTED / REAL_FILE_VALIDATION_DEFERRED` 并进入 M3.1。真实仪器样本仍是 M7 发布前的
残余风险和验证项，不得表述为已经实测。

Implementer 可在允许路径内调整私有辅助函数、缓存方式、测试参数化和内部数据结构；不得降低上述
不变量、格式矩阵或证据门槛。

## 11. 第一个精确动作

先执行 M2.1：不移动生产逻辑，新增 DAT/VSM、XLS/XLSX、追加、错误物理行号和导出无副作用的
特征测试，并记录现有 100 MiB `PlotApp.load_file` 三次前置基准；验证并更新报告后再进入 M2.2。
