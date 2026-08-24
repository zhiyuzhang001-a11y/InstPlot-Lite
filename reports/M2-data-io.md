# M2 数据 I/O 报告

## M2.1 — 行为固化与前置基准 — COMPLETE

- Date: `2026-08-23`
- Scope: 新增特征测试、固定 `.xls` fixture、前置基准脚本与本报告；未移动生产 I/O 逻辑。

### 固化的行为

- 普通 DAT 继续走文本解析路径；前十行带 VSM 标记的 DAT 在跳过 31 行元数据后读取第 4、5 列。
- XLSX 保留 Unicode 列名和值；真实 `.xls` fixture 已由 `soffice --headless --convert-to 'xls:MS Excel 97'`
  从两列小型 XLSX 转换，并经当前 `xlrd` 路径读取。
- CSV 追加保留目标文件既有列顺序；列集合不一致时在写入前失败，目标文件字节保持不变。
- 角度转弧度导出不修改传入的源 DataFrame。
- 列数异常的错误行号继续按原始物理行计数，含空行和注释。
- 结构化错误上下文（文件、编码、分隔符、物理行号）尚无核心层，因此有 1 项 `strict xfail`；M2.2
  实现 `DataIOError` 与 GUI 适配后必须移除此标记并使它通过。

### 前置 100 MiB 基准

- Command: `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python scripts/benchmark_data_io.py --size-mib 100 --runs 3`
- Environment: Python 3.13.5, pandas 2.3.3, macOS 26.5 arm64。
- Input: 104,857,588 bytes, 6,553,599 rows, 2 columns；文件生成不计入测量，窗口预热一次不计入结果。
- Run 1: 89.131121 s, 2,578,619,362 bytes `tracemalloc` peak。
- Run 2: 90.025714 s, 2,578,619,346 bytes `tracemalloc` peak。
- Run 3: 89.056903 s, 2,578,619,490 bytes `tracemalloc` peak。
- Median: 89.131121 s, 2,578,619,362 bytes (2.40 GiB)。
- M2.4 gate: 在相同脚本、相同大小和 3 次测量下，后置中位耗时与 `tracemalloc` 峰值均不得超过此值的 110%。

### 验证

- Command: `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q tests/test_data_io.py`
- Result: exit 0; 23 passed, 1 xfailed; 1.90 s。当前 GUI 的 `QMessageBox.setButtonText` 产生 6 条
  PySide6 deprecation warnings，未在 M2.1 改动，因为属于 GUI API 更新而非 I/O 行为。
- Command: `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q`
- Result: exit 0; 28 passed, 1 xfailed; 1.68 s。
- Command: `/Users/zhiyu/miniconda3/bin/python -W error::SyntaxWarning -m py_compile InstPlot.py scripts/benchmark_data_io.py`
- Result: exit 0。

### Failed attempt and cleanup

- 首次直接执行基准脚本时，Python 将 `scripts/` 设为导入根，导致 `ModuleNotFoundError: InstPlot`。
  已在脚本中显式将项目根加入 `sys.path`；随后基准完成。该修正只影响脚本执行入口。
- 基准 CSV 位于 `TemporaryDirectory`，运行结束自动删除；转换源目录与一次性 JSON 输出均已移入系统废纸篓。
- 未开始 M2.2，未创建 `instplot_io.py`，未更改 `InstPlot.py`、依赖或公共行为。

## M2.2 — 导入核心提取 — COMPLETE

- Added `instplot_io.py` with `ImportResult`, `DataIOError`, and `read_data_file`; it imports only
  standard-library modules, `chardet`, and pandas, never Qt, Matplotlib, or `PlotApp`.
- Text input reads file bytes once, detects/decodes in memory, preserves M1 header and whitespace rules,
  and reports width mismatches with stable code, source path, encoding, separator, and physical line number.
- Excel and VSM paths retain existing values and metadata; unsupported extensions, missing files, decoding,
  parsing, VSM, and Excel failures now map to `DataIOError`.
- `PlotApp.load_file` now only calls the core, updates existing GUI state, and displays the structured error.
  The former embedded import implementation was removed; `InstPlot.py` no longer contains pandas input readers.
- Added 7 no-Qt core tests; the former strict error-context xfail now passes. `pyproject.toml` packages
  `instplot_io` without changing dependencies.
- Verification: `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q` returned
  exit 0, 36 passed in 1.45 s. SyntaxWarning compilation, handoff check, and `git diff --check` also passed.
- Known warning: the unchanged export dialog emits 6 PySide6 `setButtonText` deprecation warnings; M2.2 did
  not change that UI API because it is outside the data I/O boundary.

## M2.3 — 导出核心提取 — COMPLETE

- Added `ExportSource`, `FittedCurve`, `ExportBundle`, `ExportResult`, `prepare_export`, and `write_export`
  to `instplot_io.py`. Preparation is side-effect free; only `write_export` opens a destination file.
- CSV/TXT retain the standard metadata table and UTF-8 BOM. Append reads only the destination header, preserves
  its column order, and returns `append_schema_mismatch` before any write when column sets differ.
- XLSX keeps one source sheet per input plus a fitted-data sheet; illegal characters, 31-character limits, and
  existing names are sanitized/uniquified in the writer.
- A new sheet-name test exposed an incorrectly escaped illegal-character regex; the minimal correction now
  replaces `[]:*?/\\` before openpyxl receives the name.
- `PlotApp.export_data` now only collects dialogs, explicit overwrite/append intent, and fitted coordinates;
  the duplicated pandas export implementation was removed. `InstPlot.py` contains no pandas read/write calls,
  encoding detector, `StringIO`, or sheet-naming logic.
- Verification: full suite exit 0, 42 passed in 1.30 s; SyntaxWarning compilation, handoff check, and
  `git diff --check` passed. The 6 unchanged PySide6 dialog deprecation warnings remain noted.

## M2.4 — 集成、性能与交接 — COMPLETE_PENDING_REVIEW

### 后置 100 MiB 基准

- Environment/input: 与 M2.1 相同的 Python 3.13.5、pandas 2.3.3、macOS 26.5 arm64，
  104,857,588 bytes、6,553,599 行、2 列、预热后 3 次。
- Run 1: 87.932676 s, 2,578,612,595 bytes `tracemalloc` peak。
- Run 2: 88.981655 s, 2,578,612,579 bytes `tracemalloc` peak。
- Run 3: 88.656764 s, 2,578,612,723 bytes `tracemalloc` peak。
- Median: 88.656764 s (前置 89.131121 s 的 99.47%), 2,578,612,595 bytes (前置
  2,578,619,362 bytes 的 100.00%)。两项均低于 110% 门槛。

### 最终验证

- Local command: `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q`
  returned exit 0; 42 passed, 6 unchanged PySide6 deprecation warnings。
- Local compile/check: SyntaxWarning-as-error compilation of `InstPlot.py`, `instplot_io.py`, and benchmark
  script; model-handoff check; and `git diff --check` all returned exit 0。
- Clean environment: temporary Conda CPython 3.12.13 installed `requirements.lock` with hashes, then local
  project with `--no-deps`; `pip check` returned `No broken requirements found`.
- Wheel: `instplot-1.0.0-py3-none-any.whl` contains `InstPlot.py`, `instplot_io.py`, and 67 SVG assets.
- Clean CPython 3.12.13: full suite exit 0, 42 passed in 1.50 s; SyntaxWarning compilation passed; importing
  installed modules from `/tmp` and creating an offscreen `PlotApp` window passed.

### Cleanup and review request

- Benchmark CSVs were auto-deleted by `TemporaryDirectory`; benchmark JSON, wheel directory, XLS conversion
  source, navigation cache, and temporary Python 3.12 environment were moved to the system Trash.
- No commit, push, release, dependency/version change, M3 work, or cross-platform real-machine test was made.
- Requested decision: `REVIEW_M2: ACCEPT_M2` after inline review of the import/export core boundary, test
  coverage, 100 MiB gate, clean-install evidence, and dirty-worktree preservation.

## Inline review — 2026-08-23

- Result: `REVIEW_M2: REJECT_M2`.
- **P1 — `instplot_io.py:285` — 空文件绕过统一错误边界并从 GUI 泄漏裸异常。**
  `empty.txt` 和仅含空白的文本都会在 `_detect_fallback_separator` 抛出裸 `ValueError`；
  `read_data_file` 不转换它，`PlotApp.load_file` 又只捕获 `DataIOError`，所以导入动作异常退出，
  没有合同要求的路径、阶段、错误码和“未知”字段展示。
- **P2 — `tests/test_io_core.py` / `tests/test_data_io.py` — 固定测试矩阵未满足合同第 5 节。**
  当前 42 项虽全部通过，但永久测试仍缺少分号与 Unicode/空格路径、VSM 行/列不足、空文件、
  缺失文件、只读/不可写目标、多源与重复基名、TXT 回环，以及对 XLSX 追加结果数据的回读断言；
  解码失败、Excel 引擎失败和实际写入失败的 `DataIOError` 归一化也没有被锁定。
- **P2 — 本报告未满足合同第 8 节的最低证据格式。**
  报告没有独立的格式覆盖清单和 `DataIOError` 字段实例；干净 CPython 3.12 部分也没有记录实际
  命令、解释器/临时产物路径和逐项退出码。临时资源已正确清理，不要求恢复已删除资源，但返工后
  必须记录可审核的重跑命令与结果。

### Reviewer evidence

- `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q`：exit 0，
  42 passed，6 条既有 PySide6 deprecation warnings，1.67 s。
- SyntaxWarning 编译、`git diff --check` 和 model-handoff check：均 exit 0。
- 独立临时反例：空文件、仅空白文件和 GUI 空文件入口均得到裸 `ValueError: 文件为空或只包含空行`；
  分号加 Unicode/空格路径正常；缺失文件得到 `file_not_found`，短 VSM 得到 `vsm_parse_failed`。
- 边界核对：`instplot_io.py` 没有 Qt、Matplotlib 或 `PlotApp` 依赖；两个 GUI 方法未残留 Pandas
  读写、编码检测、分隔符推断或 sheet 命名逻辑。100 MiB 前后比率均在 110% 内，未发现性能门槛
  或打包声明冲突。

## Inline review rework — 2026-08-23 — COMPLETE_PENDING_REVIEW

### Correction

- 先新增空字节、仅空白和 BOM-only 的核心/GUI 失败优先测试；首次运行准确得到 6 failures、
  48 passed，证明裸 `ValueError` 同时存在于核心和 GUI 入口。
- `_read_text` 在解码后识别空/纯空白内容并抛出稳定的 `empty_file` `DataIOError`；GUI 继续只捕获
  统一错误类型，没有增加宽泛异常捕获。
- 修复后 I/O 定向套件为 56 passed；完整套件为 61 passed。未修改公共接口、合法输入语义、依赖、
  GUI 布局或 M3 范围。

### Format and failure coverage checklist

- TXT/CSV：UTF-8 BOM、GBK、逗号、分号、单/多空格、制表符、混合空白、前置空行/注释、无表头、
  单行无表头、尾部分隔符、列数异常、Unicode/空格路径均由永久测试覆盖。
- DAT/VSM：普通 DAT、大小写 VSM 标记、跳过 31 行并取第 4/5 列、行不足和列不足均覆盖。
- Excel：真实 `.xls` fixture、XLSX Unicode 导入、非法/超长/重名 sheet、重复源基名、覆盖及追加后
  数据回读均覆盖。
- 导出：单源、多源、选列、空选列为全部列、角度转弧度、有效/无效拟合数组、CSV/TXT 回环、
  CSV/TXT 同结构追加、异结构追加拒绝、XLSX 多 sheet 和追加均覆盖；源 DataFrame 与拟合数组不变。
- 失败：空/空白/BOM-only、不支持扩展名、缺失文件、强制解码失败、列解析失败、VSM 失败、Excel
  引擎失败、不可写目标、CSV 与 XLSX 写入失败均断言为具名 `DataIOError`。

### Structured error example

- Empty text example: `path=<tmp>/empty.txt`, `operation=import`, `code=empty_file`,
  `reason=文件为空或只包含空行`, `encoding=utf-8-sig`, `separator=None`, `line_number=None`。
- GUI status contains the real path, `阶段: import`, `错误: empty_file`, detected encoding and
  `分隔符: 未知`; no exception escapes `PlotApp.load_file`.

### Rework performance gate

- Command: `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python scripts/benchmark_data_io.py --size-mib 100 --runs 3 --json-output <tmp>/post-rework.json`.
- Environment/input: Python 3.13.5, pandas 2.3.3, macOS 26.5 arm64；104,857,588 bytes、
  6,553,599 rows、2 columns；预热一次不计入结果。
- Runs: 88.196108 s / 2,578,612,595 bytes；88.580421 s / 2,578,612,579 bytes；
  88.084521 s / 2,578,612,723 bytes。
- Median: 88.196108 s，为前置 89.131121 s 的 98.95%；2,578,612,595 bytes，为前置
  2,578,619,362 bytes 的 100.00%。两项均低于 110%。脚本 JSON 中沿用的
  `pre-refactor baseline` 标签只是旧显示名称，测量的是当前返工代码。

### Local verification

- `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q tests/test_io_core.py tests/test_data_io.py`：
  exit 0，56 passed、6 warnings，1.35 s。
- `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q`：exit 0，
  61 passed、6 warnings，1.32 s。
- `/Users/zhiyu/miniconda3/bin/python -W error::SyntaxWarning -m py_compile InstPlot.py instplot_io.py scripts/benchmark_data_io.py`：exit 0。
- `git diff --check`：exit 0。6 条 warning 仍是既有 `QMessageBox.setButtonText` deprecation，
  与数据 I/O 返工无关。

### Clean CPython 3.12.13 verification

- Temporary root: `/var/folders/2t/gkd8kqg10gv_ynpqm07tgzz00000gn/T/plotapp-m2-clean.XXXXXX.NDvxKgeYsl`。
- Created with `/Users/zhiyu/miniconda3/bin/conda create -p <tmp>/env python=3.12.13 pip -y`；exit 0。
- `<tmp>/env/bin/python -m pip install --require-hashes -r requirements.lock`、安装测试运行器
  `pytest==8.4.2`、再以 `--no-deps` 安装本项目：均 exit 0；`pip check` exit 0，输出
  `No broken requirements found`。
- Full suite: exit 0，61 passed、6 warnings，48.12 s；SyntaxWarning-as-error compile exit 0。
- Wheel: `<tmp>/wheel/instplot-1.0.0-py3-none-any.whl`；包含 `InstPlot.py`、`instplot_io.py` 和
  67 SVG。`pip wheel . --no-deps` exit 0。
- 从 `/tmp` 导入的路径均位于该环境 `site-packages`；offscreen `PlotApp` 初始化为
  `OFFSCREEN_WINDOW=PlotApp`，exit 0。

### Cleanup and review request

- 100 MiB fixture 由 `TemporaryDirectory` 自动删除；基准 JSON 所在临时目录和 clean 3.12 环境/wheel
  在记录上述路径和结果后移入系统废纸篓。
- Requested decision: `REVIEW_M2: ACCEPT_M2`。M3 未开始。

## Second capable inline review — 2026-08-23

- Result: `REVIEW_M2: REJECT_M2`。
- **P1 — `instplot_io.py:217-269` — 注释与 CSV 引号使合法字段边界静默退化为单列。**
  `_find_header_row_index` 在全字符串数据中找不到数值行后进入 fallback；fallback 把前置注释当成
  header/data 候选，并用普通字符串拆分比较列数。结果是合法的
  `# metadata\nname,label\nalice,left` 被选择为制表符格式，导入为列 `name,label` 和值
  `alice,left`。分号版本同样失败。
- 相邻反例 `name,note\nalice,"left,right"` 也被静默导入为单列，因为探测和行宽校验不理解 CSV
  引号。这还会破坏应用自身导出的合法 CSV 回环：当 `source_file` 文件名包含逗号时，Pandas 会
  正确加引号，但核心无法重新读回。
- 现有 61 项完整测试 exit 0（1.49 s），说明测试矩阵把“注释”“逗号/分号”和“CSV 回环”分别
  覆盖，却没有覆盖它们与全字符串/引号字段的适用组合；因此报告中的覆盖清单声明过宽。
- 核心/GUI 边界、空文件修复、100 MiB 门槛、clean 3.12 与协议升级未发现新问题；本次只退回
  文本逻辑字段边界及其证据。

### Required correction contract

- Root invariant：表头/分隔符探测、列数校验和 Pandas 解析必须使用一致的逻辑字段边界；注释和
  空行不得参与候选 header/data，合法引号字段中的分隔符不得被计作额外列。
- Required variants：前置注释 + 全字符串逗号/分号；无注释全字符串；引号内逗号/分号；转义双引号；
  文件名含逗号的应用 CSV 导出回环；现有数字、空白、尾部分隔符和物理错误行号回归。
- Failure variants：未闭合引号和真实列数异常必须返回带编码、分隔符和原始物理行号的
  `DataIOError`，不得静默退化到制表符单列。
- Rejected shallow fixes：只在 fallback 过滤注释只能修复第一个复现；只换 `csv.Sniffer` 而继续用
  `str.split` 校验，也无法统一引号字段边界。Implementer 可选择标准库 `csv`、Pandas 解析元数据或
  其他无新依赖机制，但探测、校验和最终解析必须共享同一语义。

## Second inline review rework — 2026-08-23 — COMPLETE_PENDING_REVIEW

### Failure-first matrix and correction

- 在生产代码修改前新增评论 + 全字符串逗号/分号、无评论全字符串、引号内逗号/分号、转义双引号、
  未闭合引号、quote-aware 真实列宽和含逗号源文件名的应用 CSV 导出回环测试。定向首次运行 exit 1：
  `8 failed, 2 passed, 55 deselected in 1.05s`，准确复现第二次 review 的根不变量。
- `instplot_io.py` 现在让 fallback 与数值探测使用同一组非空、非注释候选物理行；逗号、分号和 tab
  字段统一由标准库 `csv` 语义处理，引号内分隔符和双引号转义不再改变逻辑列宽。
- 行宽校验保留合法原始 CSV 行；只有兼容既有尾分隔符时才用同一 CSV writer 重建该行。未闭合引号
  转为 `text_parse_failed`，真实列宽异常仍为 `column_count_mismatch`，两者均携带编码、已探测分隔符
  和一基原始物理行号。
- 无双引号行使用与 CSV 语义等价的快速分割路径；带双引号行仍走严格 CSV reader。未增加依赖、
  未改变公共接口、GUI 状态结构、合法列顺序或 M3 范围。

### Verification and performance

- 定向修复后命令：`QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q
  tests/test_io_core.py tests/test_data_io.py -k 'all_string or csv_field_boundaries or unclosed_quote or
  quoted_delimiter or csv_export_round_trip'`；exit 0，`10 passed, 55 deselected in 0.97s`。
- I/O 套件：合同第 8 节命令 exit 0，`65 passed, 6 warnings in 1.30s`。完整套件 exit 0，
  `70 passed, 6 warnings in 1.31s`；最终优化后重跑为 `70 passed, 6 warnings in 1.38s`。6 条 warning
  仍为既有 `QMessageBox.setButtonText` deprecation。
- 首次直接让全部非空白行创建 CSV reader 的 100 MiB 三次结果为 `107.032607 / 106.809264 /
  107.035893 s`，中位 `107.032607 s`，是前置的 `120.08%`，超过门槛；峰值
  `2,578,612,595 bytes`。该尝试未交付，随后加入无引号等价快速路径并完整重验。
- 最终 100 MiB 命令 exit 0；三次为 `80.050527 s / 2,578,612,387 bytes`、`81.248521 s /
  2,578,612,579 bytes`、`80.856231 s / 2,578,612,723 bytes`。中位 `80.856231 s`，为前置
  `89.131121 s` 的 `90.72%`；峰值中位 `2,578,612,579 bytes`，约为前置的 `100.00%`，均低于
  110% 门槛。fixture 由 `TemporaryDirectory` 自动删除。
- SyntaxWarning-as-error 编译、`git diff --check` 与 handoff check 均 exit 0。

### Clean CPython 3.12.13 installed-wheel verification

- 临时根：`/var/folders/2t/gkd8kqg10gv_ynpqm07tgzz00000gn/T/plotapp-m2-clean.5HlCT6`。以 Conda 创建
  Python 3.12.13 环境，`pip install --require-hashes -r requirements.lock`、`pytest==8.4.2`、
  `pip wheel . --no-deps` 和 wheel `--no-deps` 安装均 exit 0。
- `pip check` exit 0：`No broken requirements found.`。从临时测试副本运行完整套件 exit 0：
  `70 passed, 6 warnings in 47.64s`；trace 显示 `InstPlot` 与 `instplot_io` 均来自隔离环境的
  `site-packages`。
- 安装模块的 SyntaxWarning-as-error 编译和 offscreen 窗口初始化均 exit 0，输出
  `OFFSCREEN_WINDOW=PlotApp`。wheel 为 `instplot-1.0.0-py3-none-any.whl`，包含两个顶层模块和
  67 个 SVG 资源。
- 隔离环境、wheel、测试副本和代码阅读导航缓存已移入系统废纸篓；无存活进程或大文件。未提交、
  推送、发布或启动 M3。请求新的 capable inline Reviewer 返回 `REVIEW_M2: ACCEPT_M2` 或具名缺陷。

## Third capable inline review — 2026-08-23

- Result: `REVIEW_M2: REJECT_M2`。
- **P1 — `instplot_io.py:223-249` — 错误候选分隔符仍会把引号内字符当成真实边界并静默错列。**
  fallback 依次尝试逗号、分号、空白和 tab，却没有先证明候选分隔符出现在引号外。对于合法分号文件
  `name;"note, label"\nalice;"left,right"`，以逗号调用 CSV reader 时，双引号位于分号之后，
  因而不处于“逗号字段起始”位置；两行都会被错误解析为两个逗号字段，探测器立即接受逗号，最终
  静默得到列 `name;"note` / `label"`，而不是 `name` / `note, label`。
- 相邻反例 tab 文件 `name\t"note, label"\nalice\t"left,right"` 同样被误判成逗号文件；加前置
  注释也仍失败。反向的逗号文件 + 引号内分号目前只是因为逗号候选排在第一而通过，不能作为正确
  性保证。当前测试分别覆盖“分号中引用分号”和“逗号中引用逗号”，但没有覆盖不同候选分隔符在
  引号内容中碰撞，因此覆盖矩阵仍不完整。
- Root invariant：分隔符发现必须基于引号外的物理候选字符；确定分隔符后，表头探测、列宽校验与
  Pandas 才能以该分隔符共享逻辑字段语义。使用错误候选解析后恰好得到相同列宽，不得成为选择依据。
- Required variants：分号/tab 文件的引号字段含逗号；逗号文件的引号字段含分号或 tab；带前置
  注释、转义双引号及不同字符串值的相邻组合；现有同分隔符引号、未闭合引号、物理行号和含逗号
  文件名回环全部保留。
- Rejected shallow fixes：不得只调整候选顺序；这只会把当前分号/tab 失败转移到逗号文件。也不得
  绕过统一行宽校验。应先以 quote-aware 方式识别引号外候选，再用同一 CSV 语义完成校验和 Pandas
  解析；不得增加依赖。
- Reviewer evidence：独立临时反例输出分别为分号/Tab 输入均选择 `','` 并静默产生错误列；反向
  逗号 + 引号内分号正常。完整套件 exit 0，`70 passed, 6 warnings in 1.37s`，证明缺少交叉候选
  碰撞组合。SyntaxWarning 编译、handoff check 和 `git diff --check` 均 exit 0。报告中的最终
  100 MiB 比率计算和 clean 3.12 命令链未发现新冲突，但行为 P1 已阻止 M2 接受。

## Third inline review rework — 2026-08-23 — COMPLETE_PENDING_REVIEW

### Failure-first matrix and correction

- 生产代码修改前新增 5 个跨候选组合：分号/tab 文件的引号字段含逗号，逗号文件的引号字段含
  分号/tab，以及前置注释 + 分号 + 转义双引号 + 引号内逗号。首次定向运行 exit 1：
  `3 failed, 2 passed, 38 deselected in 0.21s`；失败项均错误返回 separator `','`。
- 新增 `_has_unquoted_separator`，只允许 fallback 在候选字符真实出现在 CSV 引号外时尝试该候选；
  明确 tab 优先于统一空白，因为带引号 tab 字段必须继续使用 CSV quote 语义。候选确定后仍复用
  `_split_fields`、严格行宽校验和 Pandas，不重排行为来掩盖错误。
- 相邻审计发现合法未引用字面双引号列名 `height 5";label` 会被初版扫描器误认为引用区；先新增
  测试并得到 `1 failed, 43 deselected in 0.19s`，再将引用区起点收紧为记录开头或已知显式分隔符
  之后。该测试与 5 个跨候选组合最终均通过。
- 未增加依赖、公共接口或 GUI 修改；M3 未开始。预算化代码阅读把检查限制在 fallback、字段拆分、
  行宽校验和对应核心测试，没有改动无关 GUI/算法区域。

### Verification and artifact evidence

- 跨候选与字面引号定向套件 exit 0：`6 passed, 38 deselected in 0.17s`。
- I/O 合同套件 exit 0：`71 passed, 6 warnings in 1.31s`；完整本地套件 exit 0：
  `76 passed, 6 warnings in 1.32s`。6 条 warning 仍为既有 `QMessageBox.setButtonText` deprecation。
- SyntaxWarning-as-error 编译、handoff check 和 `git diff --check` 均 exit 0。
- 本次新增扫描只在数值探测未找到分隔符时检查最多十条 fallback 候选；100 MiB 基准的数值 CSV
  在 `_find_header_row_index` 已确定逗号，不进入该函数，逐行校验/Pandas 热路径也未改变。因此未重复
  消耗约 5 分钟重测，最终有效性能证据保持 `80.856231 s / 2,578,612,579 bytes`，均低于 110%。
- clean CPython 3.12.13 临时根：
  `/var/folders/2t/gkd8kqg10gv_ynpqm07tgzz00000gn/T/plotapp-m2-clean.BAlNRQ`。锁文件哈希安装、
  `pytest==8.4.2`、wheel 构建/安装及 `pip check` 均 exit 0；安装 wheel 后的隔离测试副本为
  `76 passed, 6 warnings in 47.49s`。
- 安装模块编译、从 `site-packages` 导入和 offscreen `PlotApp` 初始化均 exit 0；wheel 含两个模块与
  67 SVG。临时环境、wheel 和测试副本已移入系统废纸篓，无存活资源。
- Requested decision：新的 capable inline Reviewer 返回 `REVIEW_M2: ACCEPT_M2` 或具名缺陷；
  不得 self review、提交、推送或开始 M3。

## Fourth capable inline review — 2026-08-23

- Result: `REVIEW_M2: REJECT_M2`。
- **P1 — `instplot_io.py:112-133,247-275` — “引号外存在”仍无法区分结构分隔符与字段文本，合法
  分号/Tab 会按候选顺序静默错列。** `_has_unquoted_separator` 只回答候选字符是否出现在引号外；
  fallback 随后仍接受第一个列宽一致的候选。当错误候选是未引用字段中的普通文字、而真正分隔符
  引出后续引用字段时，两个候选都在引号外且都能得到稳定列宽，逗号/分号的固定顺序再次决定结果。
- 具名复现：实际分号文件
  `key, literal;"note , ""quoted"""\nalice, literal;"value , ""quoted"""` 被选择为逗号并拆成
  3 个错误列；实际 Tab 文件的第一个未引用字段含逗号或分号时同样失败。6 个由标准库 `csv.writer`
  生成的跨方言组合中 3 个失败，均为静默错误；反向逗号组合只因逗号候选排首而通过。
- Root invariant：发现分隔符必须区分“重复出现在字段文本中的候选字符”和“构成 CSV 字段边界的
  方言字符”。候选存在且列宽一致只是必要条件；还必须评估完整记录的 quote 边界/方言一致性，
  错误候选恰好等宽不得获胜。
- Required variants：分号文件的未引用字段含逗号、后续引用字段也含逗号/转义引号；Tab 文件的
  未引用字段分别含逗号和分号；反向逗号文件；前置注释和至少三条数据记录；现有纯引号碰撞、
  字面双引号、未闭合引号、行宽和导出回环全部保留。
- Rejected shallow fixes：不得只检查引号外是否出现、调换候选顺序或用扩展名强制逗号；这些都会
  把失败转移到另一方言。应对多条候选记录收集结构证据，例如候选分隔符下 quote 是否位于合法字段
  边界、记录宽度一致性及显式边界重复性，再以同一选定方言进行严格校验和 Pandas 解析。
- Reviewer evidence：独立临时生成 6 个 comma/semicolon/tab 交叉样本，输出 `CASES=6 FAILURES=3`；
  三个失败均返回错误 separator 和列/值。永久完整套件仍 exit 0：`76 passed, 6 warnings in 2.15s`；
  SyntaxWarning 编译、handoff check 和 `git diff --check` 均 exit 0。性能路径与 clean 3.12 证据未见
  新冲突，但行为 P1 阻止 M2 接受。

## Direct delimiter remediation — 2026-08-23 — COMPLETE

用户明确要求暂时停止合同往返并直接解决已知问题。本次未继续叠加单例条件，而是替换全字符串
fallback 的判定模型。

### Unified dialect decision

- 对 comma、semicolon、tab 和统一空白分别解析最多十条有效物理记录，收集：合法字段起始 quote、
  未引用字面 quote、与表头等宽记录、异宽记录及严格 CSV 解析错误。候选按该完整结构证据比较，
  不再根据存在性或固定顺序直接成功。
- comma/semicolon/tab 同分时返回结构化 `ambiguous_separator`，不静默猜测；纯 tab 与统一空白若产生
  完全相同记录，选择更精确的 tab。最终选定方言继续由 `_split_fields`、行宽校验和 Pandas 共享。
- 仅评论文件现在与空/空白/BOM-only 一样得到 `empty_file` `DataIOError`，核心和 GUI 均无裸
  `ValueError`。有效记录检查使用短路正则；fallback 前十行通过有界物理行扫描采样，不构造额外的
  整文件行列表。

### Failure-first and combination evidence

- 新增 6 个由标准库 `csv.writer` 生成的完整竞争方言组合，每个包含表头、前置注释和三条数据；
  覆盖 actual comma/semicolon/tab × competing comma/semicolon/tab。旧实现定向运行 exit 1：
  `6 failed, 1 passed, 45 deselected in 0.23s`；其中另 1 项为真实歧义未拒绝。
- 新增真正等价的 `a,b;c` 多记录歧义核心/GUI 测试、comments-only 核心/GUI 测试，并保留此前
  5 个引用碰撞、字面双引号、转义、未闭合引号、物理行号及应用导出回环。
- 方言评分实现后相关 17 项 exit 0：`17 passed, 35 deselected in 0.19s`；完整本地套件 exit 0：
  `86 passed, 6 warnings in 1.34s`。SyntaxWarning-as-error 编译和 `git diff --check` 均 exit 0。

### Final performance and clean installation

- 100 MiB 三次最终结果：`79.933742 s / 2,578,612,595 bytes`、`81.358133 s /
  2,578,612,579 bytes`、`80.132906 s / 2,578,612,723 bytes`。中位 `80.132906 s`，为最初
  `89.131121 s` 的 `89.90%`；峰值中位 `2,578,612,595 bytes`，约为最初的 `100.00%`。
- clean CPython 3.12.13 临时根：
  `/var/folders/2t/gkd8kqg10gv_ynpqm07tgzz00000gn/T/plotapp-m2-clean.kOxvm0`。锁文件哈希安装、
  pytest、wheel 构建/安装、`pip check`、安装模块编译和 offscreen 启动均 exit 0；隔离测试副本为
  `86 passed, 6 warnings in 49.82s`，wheel 含两个模块和 67 SVG。
- 基准 fixture 自动删除；clean 环境、wheel、测试副本和导航缓存已移入系统废纸篓。未增加依赖、
  提交、推送、发布或进入 M3。

## Automated substitute acceptance — 2026-08-23 — COMPLETE_PENDING_INDEPENDENT_VERIFICATION

用户远程操作且当前没有真实仪器文件，批准以确定性自动化证据完成 M2 收尾，并把真实样本验证明确
延期到 M7 发布前。本批次只修改测试和项目记录，没有修改 `InstPlot.py` 或 `instplot_io.py` 生产代码。

### Added acceptance coverage

- 黄金矩阵：UTF-8 BOM/comma、GB18030/semicolon、Big5/tab，覆盖中文/繁体列名、空格、候选分隔符、
  CSV 引号及整数/浮点/文本值；逐项核对选定分隔符、规范化列名、列顺序和单元格语义值。
- 固定种子 `20260823` 生成 1,000 个独立临时文件，均由标准库 `csv.writer` 构造；actual dialect 在
  comma、semicolon、tab 间轮换，竞争字符、未引用字段、引用字段、转义双引号、3～6 行数据及
  有/无前置注释组合变化。预期直接来自生成矩阵，不复用 PlotApp 解析结果。
- 回环门槛复用并执行现有 TXT 核心回环、XLSX 多 sheet/同 basename 回环和 GUI CSV 导出再导入。
- 新增 offscreen GUI 测试，证明 `loaded_files` DataFrame、`combo_x`、`combo_y` 的列名及顺序一致。
- 真实结构歧义的 `ambiguous_separator` 既有永久测试继续随完整套件执行。

### Commands and results

- 首次新增定向测试 exit 1：`1 failed, 4 passed in 2.49s`。唯一失败是测试把文本 `2.50` 预期为
  原字符串，而 Pandas 合法解析为数值 `2.5`；改为按整数/浮点/文本语义类型比较，未修改生产代码。
- 四层定向验收 exit 0：`8 passed in 1.51s`；其中包括三项编码黄金样本、1,000 组随机方言对照、
  TXT/XLSX/GUI CSV 三种回环和 GUI 列映射。
- M2 核心 + GUI 套件 exit 0：`86 passed, 6 warnings in 1.78s`。
- 全项目套件 exit 0：`91 passed, 6 warnings in 1.82s`。
- `/Users/zhiyu/miniconda3/bin/python -W error::SyntaxWarning -m py_compile InstPlot.py instplot_io.py
  scripts/benchmark_data_io.py` exit 0；`git diff --check` exit 0。
- 6 条 warning 均为既有 `QMessageBox.setButtonText` deprecation，不是本批次新增失败。
- 未重新运行 100 MiB 与 clean CPython 3.12：生产代码、依赖、锁和打包输入均未变化，沿用上一节
  `80.132906 s / 2,578,612,595 bytes` 及 clean installed-wheel 86 项证据。

### Residual risk and cleanup

- 结论候选：`AUTOMATED_ACCEPTED / REAL_FILE_VALIDATION_DEFERRED`。自动化证据未发现静默错列；
  但不能声称已经实测未知仪器私有变体，获得真实样本后仍需补测，并在 M7 发布前复核。
- 1,000 个文件全部位于 pytest `tmp_path`，测试结束自动清理；无网络下载、新依赖、后台进程、
  大型持久 fixture、提交、推送或 M3 修改。

## Independent automated acceptance verification — 2026-08-23 — ACCEPT

- 结论：`AUTOMATED_ACCEPTED / REAL_FILE_VALIDATION_DEFERRED`。
- 独立检查确认黄金/随机预期在调用 PlotApp 解析器前由标准库 `csv.writer` 和内存字段矩阵生成，
  不是解析器自证；1,000 组覆盖三种实际方言、两类竞争候选、有/无注释及 3～6 行记录。结构模板
  仍不能替代未知仪器私有格式，故真实样本风险继续保留到 M7。
- 独立复跑定向套件 exit 0：`8 passed in 2.66s`；M2 套件 exit 0：
  `86 passed, 6 warnings in 2.00s`；完整套件 exit 0：`91 passed, 6 warnings in 1.89s`。
- SyntaxWarning-as-error 编译和 `git diff --check` 均 exit 0；6 条 warning 均为既有 Qt deprecation。
- 未发现测试自证、静默错列、生产代码改动、新依赖、网络访问或未清理资源。M2 接受，允许只激活 M3.1。
