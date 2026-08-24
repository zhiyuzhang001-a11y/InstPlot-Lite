# M3 算法、状态与撤销重构合同

> 状态：`M3.1 ACCEPTED / M3.2 COMPLETE / M3.3 PLANNED`。M3.1 的行为固化、八类历史入口审计和
> 旧内存基线已独立接受；M3.2 shared conversion matrix 已闭环。下一步先规划 M3.3 纯历史核心；
> M3.4～M3.5 未开始。

## 1. 目标与用户结果

M3 把数据处理算法和历史记录从 `PlotApp` 中拆成无 Qt、可独立测试的核心。合法数值输入的结果和
现有操作入口保持一致；NaN、Inf、短数组、空选择和失败路径获得明确合同。撤销不再为每次操作深拷贝
全部 `loaded_files`，并新增可验证的重做能力。

用户可见结果：

- 去噪、局部处理、对称、归一化和去背底仍从现有按钮进入；
- 单点/框选点、线条删除和五类处理都可撤销；新增工具栏“重做”；
- 取消、无变化和失败操作不占用历史槽；
- 十步历史显著小于十份完整数据副本，并保留路径、列顺序、dtype、索引和数据值。

## 2. 已确认现状

- `InstPlot.py:296-297` 用普通列表保存最多 10 份历史。
- 单点删除、框选删除、线条删除、去噪、局部处理、对称、归一化和去背底共有 8 处
  `copy.deepcopy(self.loaded_files)`；每步复制所有文件和所有列。
- `PlotApp.undo` 只弹出一份旧快照，没有 redo 栈、命令身份或失败回滚。
- 去背底在打开参数对话框前就保存历史；多个处理在确认后、确定真正发生变化前保存历史，因此取消、
  缺列或全部失败可能产生空历史项。
- 算法仍位于 `InstPlot.py`：`center_data`、`normalize_data`、`local_flatten_keep_anchor`、
  `denoise_data`；背景拟合仍内嵌 GUI。当前只有 2 项去噪单测。
- `loaded_files` 的公共结构是有序的 `(path, DataFrame)` 列表；M3 不改变该结构。

## 3. 冻结架构

### 3.1 处理核心

新增 `instplot_processing.py`，只依赖标准库、NumPy、Pandas 和 SciPy，不依赖 Qt、Matplotlib 或
`PlotApp`。至少提供：

- `ProcessingError(operation, code, reason)`；
- `ProcessingResult(values, changed_mask, metadata)`；
- `center_values(values)`；
- `normalize_values(values, top_n=20)`；
- `local_flatten_values(x, y, x1, x2, transition=0, anchor="left", strength=1.0)`；
- `denoise_values(y, window_length, polyorder, x=None, x1=None, x2=None)`；
- `remove_polynomial_background(x, y, fit_min, fit_max, order)`。

核心函数不修改输入数组或 DataFrame。`PlotApp` 只负责对话框、选择文件/列/行、把结果发布回
`loaded_files`、状态栏和重绘。

### 3.2 历史核心

新增 `instplot_history.py`，不依赖 Qt、Matplotlib 或 `PlotApp`。提供有界 `HistoryManager`，维护
undo/redo 栈，并使用差量命令而非完整快照：

- `ColumnPatchCommand`：保存文件位置/身份、列、行位置以及 before/after 数组；
- `DeleteRowsCommand`：保存被删行、原位置、dtype/index 恢复信息；
- `DeleteFilesCommand`：保存被移除的 `(path, DataFrame)` 引用及原位置，不复制未删除文件；
- `CompositeCommand`：一次用户操作中多个文件的成功差量作为一个撤销单位。

redo 必须发布保存的 after 数据，不重新运行算法。新操作在 undo 后执行时清空 redo 栈；超过 10 步
只淘汰最旧命令。命令提供可审计的 `payload_bytes`，供内存门槛使用。

## 4. 行为合同

### 4.1 数值与异常

- 所有函数接受一维输入；输出长度和顺序不变，输入对象内容不变。
- 有限数值按当前合法样本结果冻结，误差使用按算法定义的绝对/相对容差。
- NaN/Inf 不参与 min/max、均值、滤波或拟合，其原位置和值保留；去噪按连续有限片段处理，片段短于
  合法窗口时保持原值。
- 空数组、全非有限数组和不足拟合点返回具名 no-op 或 `ProcessingError`，不得产生 NumPy
  RuntimeWarning、裸异常或静默全 NaN。
- 参数错误（偶数/过小窗口、`polyorder >= window`、非法 anchor、strength 越界、无效区间、
  背景阶数或列长度不一致）必须有稳定错误码。
- 归一化沿用“中心化后取最高 `top_n` 有限值平均并夹紧到 [-1, 1]”语义。若零、全负和近零分母的
  特征测试不能证明合理预期，必须停止请求领域决定，不得自行更换为绝对值归一化。

### 4.2 发布与部分失败

- 一次多文件操作先为各文件计算结果，再发布差量；成功文件组成一个 `CompositeCommand`。
- 某文件可测试失败时，该文件保持不变并进入结果摘要；其他成功文件沿用当前“可部分成功”语义。
- 若没有文件发生变化，不创建历史项；取消对话框、缺列、空选择和算法 no-op 同样不创建。
- 发布阶段出现异常时，已发布差量必须回滚，undo/redo 栈保持不变；使用确定性失败注入测试，不用
  sleep 或时序碰运气。

### 4.3 撤销与重做

- 覆盖单点删除、框选删除、删除一条/多条线、五类列处理及多文件部分选择。
- 每次 undo/redo 后，`loaded_files` 的顺序、路径、DataFrame 列顺序、dtype、索引和值精确恢复；
  X/Y 下拉框和重绘只由 GUI 适配层更新。
- 重复路径和相同 basename 不得让命令作用到错误文件；命令必须验证记录的位置和文件身份。
- 连续 10 次 undo 后再连续 10 次 redo，最终状态分别等于初始状态和处理后状态。
- `clear_plot`、加载文件和行过滤器是否进入历史沿用当前语义：不进入；它们不得留下指向失效数据的
  redo 命令，状态替换时应清空历史。

## 5. 高风险覆盖矩阵

- 值：正常、空、单点/短数组、常量、全零、全负、NaN、±Inf、重复 X、乱序 X、极大/极小值。
- 身份：单/多文件、重复路径/basename、同一 DataFrame 引用、选中子集、缺列。
- 路径与位置：首/中/末行删除、不连续行、全部行、首/中/末文件、非默认索引。
- 顺序与发布：execute→undo→redo、连续 undo/redo、undo 后新操作、历史溢出、多文件部分成功。
- 替换：列全量/局部替换、行删除恢复、文件删除恢复、加载/清空后的历史失效。
- 失败与中断：算法参数失败、计算中某文件失败、发布中途注入失败、取消/no-op；M3 同步执行，线程
  取消不适用并留给 M4。

同一根不变量在修复后第二次失败时停止串行补丁，返回 Planner/Verifier 重建矩阵。

## 6. 分阶段计划

### M3.1 — 行为固化与基准（ACCEPTED）

- 只新增处理/历史特征测试、确定性 fixture、`scripts/benchmark_history.py` 和报告；不移动生产代码。
- 固定五类算法正常结果、边界行为、现有部分成功语义、八类历史入口和旧快照内存基线。
- 对需要领域判断的归一化/拟合样本立即停止，不在后续阶段猜测。
- 完成后交回 capable independent verification。

### M3.2 — 纯处理核心（COMPLETE）

- 新建 `instplot_processing.py` 和无 Qt 单测；按函数逐个迁移，旧名称可暂作兼容薄包装。
- 背景拟合从 GUI 提取；证明输入无副作用、非有限值规则和稳定错误码。
- GUI 暂不更换历史机制；完整回归和处理基准通过后交回 capable independent verification。
- 兼容决定：全负、全零和近零的有限值归一化保持 M3.1 冻结结果，不自行改为绝对幅值；NaN/Inf、
  空/短数组和非法参数按第 4.1 节统一为保留位置、具名 no-op 或稳定 `ProcessingError`。

### M3.3 — 差量历史核心

- 新建 `instplot_history.py`，实现三类命令、组合命令、undo/redo、淘汰与 payload 统计。
- 先用纯 DataFrame 状态测试验证身份、顺序、发布失败回滚和 10 步往返，不接 Qt。
- 核心门禁通过后交回 capable independent verification。

### M3.4 — GUI 集成

- 将八处完整 deepcopy 迁至命令边界；只有成功变化才入历史。
- `undo` 改为薄适配并增加“重做”工具栏动作；状态替换清空失效历史。
- 保留现有按钮、对话框、`loaded_files` 结构、部分成功摘要和重绘行为；增加少量 offscreen 集成测试。
- 完成后交回 capable independent verification。

### M3.5 — 内存、安装与交接

- 同一确定性 fixture 比较旧快照估算与新 `payload_bytes`：4 个文件、每个 250,000 行、8 个
  float64 列，连续 10 次单列处理后，新历史 payload 不得超过 10 份完整快照字节的 35%。
- 删除行场景按实际删除比例验证；undo/redo 10 步必须逐帧精确相等。
- 运行完整测试、编译、wheel 内容、clean CPython 3.12 安装、`pip check` 和 offscreen 启动。
- 清理大 fixture/临时环境，更新报告并交回最终 capable independent verification。

## 7. 允许路径

- `InstPlot.py`：仅处理适配、历史边界、undo/redo 工具栏和必要 import；
- `instplot_processing.py`、`instplot_history.py`；
- `pyproject.toml`：仅纳入两个模块，不改依赖版本；
- `tests/test_processing.py`、`tests/test_history.py`、`tests/test_processing_gui.py`、必要 fixture；
- `scripts/benchmark_history.py`、`reports/M3-processing-history.md`；
- 本合同、主计划、`STATUS.md`、`MODEL_HANDOFF.md`。

不得修改数据 I/O 语义、绘图样式/出版功能、拟合功能、主题布局、线程、依赖锁、安装脚本、M4～M7；
不得提交、推送或发布。

## 8. 验收命令

```bash
QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q tests/test_processing.py tests/test_history.py tests/test_processing_gui.py
QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python -m pytest -q
/Users/zhiyu/miniconda3/bin/python -W error::SyntaxWarning -m py_compile InstPlot.py instplot_processing.py instplot_history.py scripts/benchmark_history.py
/Users/zhiyu/miniconda3/bin/python scripts/benchmark_history.py --rows 250000 --files 4 --columns 8 --steps 10
/Users/zhiyu/miniconda3/bin/python .model-handoff/handoff.py check .
git diff --check
```

最终 clean CPython 3.12 验证与 M2 相同，并确认 wheel 包含 `InstPlot`、`instplot_io`、两个 M3 模块
和 67 个 SVG。

## 9. 下一阶段的精确动作

只规划并启动 M3.3：先建立差量历史纯核心的失败优先合同和测试，不接 GUI、不修改现有八处 history
owner；合同和测试矩阵明确后再实现 `instplot_history.py`。
