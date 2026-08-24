# M1 验证报告

## 基线

- Date: 2026-08-22
- Branch: `main`
- Base HEAD: `1020bc16444077934971209757edab67fc4884f8`
- Initial working tree: dirty

## 既有修改（受保护）

`InstPlot.py` 在 M1 开始前已相对 Base HEAD 修改：174 additions、60 deletions。
该差异包括扩展文件选择、TXT/CSV 导入解析修复，以及导出结构调整；它不是 M1 自有改动，
不得被回退、覆盖或归属为 M1 实施结果。

## M1 自有改动

- 新增 `pyproject.toml`：声明 Python `>=3.12,<3.13`、所有直接运行依赖及 `test` 可选依赖。
- 更新 `requirements.txt`：改为从项目元数据安装，避免与 `pyproject.toml` 的依赖重复漂移。
- 更新 `README.md`：说明 macOS/Linux 和 Windows 的 Python 3.12 虚拟环境、安装和启动命令。
- 新增 11 项回归测试：文本导入、编码、导出回环、去噪边界与 Qt 无界面启动。
- `InstPlot.py` 的 M1 最小叠加修改：
  - 忽略表头前的 `#` 注释行；
  - 将紧邻数据行但列数不匹配的非数值行视为表头，明确拒绝列数异常，避免误导入为无表头数据；
  - 对无法满足 Savitzky-Golay 条件的短数据或无效窗口返回原值；
  - 使用原始 f-string 修复 5 个 `\pi` 非法转义警告。

除上述项目外，`InstPlot.py` 的原有差异仍保持受保护状态。

## 验证记录

- `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python3 -m pytest -q`
  - Exit: 0；11 passed；1.07 s。
- `/Users/zhiyu/miniconda3/bin/python3 -W error::SyntaxWarning -m py_compile InstPlot.py`
  - Exit: 0；无 SyntaxWarning。
- 使用 `/Users/zhiyu/miniconda3/bin/uv` 创建临时 CPython 3.12.14 环境，并执行
  `uv pip install ".[test]"`。
  - Exit: 0；解析/安装 30 个包。
  - 直接依赖版本：PySide6 6.11.2、NumPy 2.5.2、Pandas 2.3.3、Matplotlib 3.11.1、
    SciPy 1.18.1、QtAwesome 1.4.2、OpenPyXL 3.1.5、xlrd 2.0.2、chardet 5.2.0。
- `QT_QPA_PLATFORM=offscreen <python-3.12> -m pytest -q`
  - Exit: 0；11 passed；1.18 s。
- `uv pip check --python <python-3.12>`
  - Exit: 0；30 个包兼容。
- 在第二个干净 CPython 3.12.14 `venv` 中执行 `<python> -m pip install .`，再执行
  `<python> -m pip install -r requirements.txt`。
  - Exit: 0；标准 pip 路径和 requirements 路径均成功。
- `<python> -m pip check`
  - Exit: 0；`No broken requirements found.`
- `QT_QPA_PLATFORM=offscreen <python-3.12> -c "... PlotApp() ..."`
  - Exit: 0；`pip-install smoke: PASS`。

## Observations and cleanup

- 全新 PySide6 6.11.2 环境的首次 Qt 导入在本机可能超过 30 秒；导入缓存建立后，同一无界面
  冒烟测试约 1.3 秒通过。该首次启动成本记录为 M4 性能基线，不在 M1 改动 Qt 版本或体积策略。
- Qt 仍提示缺失 `Sans Serif` 字体别名，但不影响无界面启动；字体/启动性能优化留待 M4。
- 两个临时 Python 3.12 环境和采样文件已移至系统废纸篓，项目中没有留下临时虚拟环境或采样文件。

## Bugbot 返工验证（2026-08-22）

- 先新增 `test_single_numeric_row_without_header_is_preserved` 和
  `test_whitespace_column_count_mismatch_is_not_imported`；修复前运行
  `QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python3 -m pytest -q tests/test_data_io.py -k
  'single_numeric_row or whitespace_column_count'`，结果为 **2 failed**，确认审查中的两个导入缺陷。
- 修复后同一命令：**2 passed**。
- 新增 `test_symbol_selector_assets_are_available_at_runtime`，验证运行时目录有 67 个 SVG，且第一个
  SVG 可构造非空 `QIcon`。
- 新增 `symbol_icons/__init__.py`、Setuptools package discovery 和 package-data 配置；`requirements.lock`
  由 `uv pip compile pyproject.toml --python-version 3.12 --universal --generate-hashes --output-file
  requirements.lock` 生成，共 25 个解析包（666 行），锁定运行时依赖和哈希。
- 本机 `/Users/zhiyu/miniconda3/bin/python3` 为 Python 3.13.5：
  `QT_QPA_PLATFORM=offscreen ... -m pytest -q` → **14 passed，1.12 s**；
  `... -W error::SyntaxWarning -m py_compile InstPlot.py` → 退出码 0。
- 干净 CPython 3.12.14（`uv venv --python 3.12 --seed`）中先执行
  `pip install -r requirements.txt`，再执行 `pip install --no-deps .`：均退出码 0。
  从源码目录外导入已安装的 `InstPlot`，67-SVG/QIcon 冒烟输出
  `installed resource smoke: PASS (67 SVG)`；`pip check` 输出 `No broken requirements found.`。
- 同一干净 CPython 3.12.14 环境中执行
  `QT_QPA_PLATFORM=offscreen <python> -m pytest -q tests` → **14 passed，45.57 s**；
  `<python> -W error::SyntaxWarning -m py_compile InstPlot.py` → 退出码 0。
- 首次未加 `--seed` 的 `uv venv` 没有 pip，未执行安装；该临时环境已移入系统废纸篓。随后使用
  `--seed` 重建并完成上述验证。所有本轮临时环境均已移入系统废纸篓。

## 空白分隔根因返工验证（2026-08-22）

- 第二次 Bugbot 审查发现 `normalize_and_validate_rows()` 把正则分隔符 `r'\s+'` 通过
  `sep.join(fields)` 写回文本，导致正常空白分隔数据成为字面量 `1\\s+2`。
- 先新增参数化 `test_whitespace_delimited_rows_preserve_field_boundaries`，覆盖单空格、多个连续空格
  和制表符。修复前运行其筛选测试：**2 failed、1 passed**；单空格与多空格被破坏，制表符不受影响。
- 修复为：对 `r'\s+'` 仍执行逐行列数校验，但验证通过后保留原始行；CSV/TSV 继续按既有逻辑规范化。
  修复后该参数化测试与空白列数异常测试：**4 passed**。
- 本机 Python 3.13.5：`QT_QPA_PLATFORM=offscreen /Users/zhiyu/miniconda3/bin/python3 -m pytest -q`
  → **17 passed，1.25 s**；`-W error::SyntaxWarning -m py_compile InstPlot.py` → 退出码 0。
- 干净 CPython 3.12.14（`uv venv --python 3.12 --seed`，按 `requirements.txt` 安装依赖并安装 pytest）中：
  `QT_QPA_PLATFORM=offscreen <python> -m pytest -q tests` → **17 passed，48.41 s**；
  `<python> -W error::SyntaxWarning -m py_compile InstPlot.py` → 退出码 0。
  临时环境已移入系统废纸篓。

## 混合空白分隔根因返工验证（2026-08-22）

- Inline 审查发现候选顺序 `['\t', ',', ';', r'\s+']` 会让含制表符的数据行提前选择 `\t`，而
  空格表头按 `\t` 只被识别为一列，合法混合空白文件因此被严格校验错误拒绝。
- 参数化空白测试新增三项：混合空格/制表符同行、空格表头配制表符数据、制表符表头配空格数据。
  修复前运行该筛选测试：**2 failed、4 passed**；混合同行与空格表头配制表符数据失败。
- 修复为将候选顺序改为 `[',', ';', r'\s+', '\t']`：逗号/分号仍优先，空格与制表符统一由
  正则空白规则解析。修复后六项空白参数化测试：**6 passed**；加列数异常测试：**7 passed**。
- 本机 Python 3.13.5：完整 `pytest -q` → **20 passed，1.20 s**；SyntaxWarning 编译检查退出码 0。
- 干净 CPython 3.12.14（`uv venv --python 3.12 --seed`）中：
  `QT_QPA_PLATFORM=offscreen <python> -m pytest -q tests` → **20 passed，47.23 s**；
  `<python> -W error::SyntaxWarning -m py_compile InstPlot.py` → 退出码 0。
  临时环境已移入系统废纸篓。
