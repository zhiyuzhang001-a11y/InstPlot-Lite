# M1 review report

- Date: `2026-08-22`
- Reviewer: Bugbot
- Diff: uncommitted changes against `1020bc16444077934971209757edab67fc4884f8`
- Result: `REVIEW_M1: REJECT_M1`

## Findings

1. **P1 — `InstPlot.py:4774` — 空白分隔文件绕过列数校验。**
   输入 `x y\n1 2 3\n` 会触发 Pandas `ParserWarning`，但仍显示加载成功并静默丢弃第三列，
   与 M1“明确拒绝列数异常”的验收声明冲突。
2. **P2 — `InstPlot.py:4582` — 单行纯数值文件被误判为表头。**
   输入 `1,2\n` 得到列名 `["1", "2"]` 和 0 行数据，而不是无表头的一行数据；现有测试只覆盖
   两行无表头数据。
3. **P2 — `pyproject.toml:27` — 安装产物缺失符号资源。**
   当前配置仅打包 `InstPlot.py`，没有包含 `symbol_icons/` 的 67 个运行时 SVG；安装后符号选择器
   找不到资源并退化为文本按钮。现有冒烟测试只初始化主窗口。
4. **P2 — `docs/IMPLEMENTATION_PLAN.md:164` — 初始锁定方案缺失。**
   M1 明确要求生成初始锁定方案，但仓库没有锁文件、约束文件或分平台 requirements；一次安装
   解析出的版本记录不能阻止依赖随时间漂移。

## Required acceptance changes

- 为前两个导入缺陷增加失败优先的回归测试并做最小修复。
- 从实际构建并安装的产物验证 67 个 SVG 可访问，且符号选择器使用资源而非退化文本。
- 提交 Python 3.12 的初始依赖锁定方案，并在干净环境验证安装及 `pip check`。
- 保持原有 11 项测试和 SyntaxWarning 编译检查通过，更新 `reports/M1-validation.md` 的命令、
  测试数量、安装产物和清理证据。

## Scope boundary

只返工 M1；不开始 M2，不进行模块化、性能或跨平台安装脚本工作，不覆盖既有未提交修改。

## Second review — 2026-08-22

- Result: `REVIEW_M1: REJECT_M1`
- **P1 — `InstPlot.py:4811` — 正常空白分隔数据被正则分隔符字面量破坏。**
  输入 `x y\n1 2\n3 4\n` 会得到 `x=["1\\s+2", "3\\s+4"]`、`y=[NaN, NaN]`。
  根因是校验后以 `sep.join(fields)` 重新拼接，而 `sep` 此时是正则表达式字符串 `r'\s+'`。
- Root invariant：验证和规范化不能改变合法字段边界或字段值；正则分隔符只能用于解析。
- Required variants：新增正常单空格、多空格、制表符分隔测试，并保留列数异常拒绝测试。
- Rejected shallow fix：不能继续把 `r'\s+'` 当成真实分隔文本拼回数据，也不能只覆盖异常路径。
- 其余三项原缺陷已核实关闭：单行数据、67 个 SVG 安装资源、Python 3.12 锁定安装流程。

## Inline review — 2026-08-22

- Result: `REVIEW_M1: REJECT_M1`
- **P1 — `InstPlot.py:4585` — 混合空白分隔被误判为列数异常。**
  输入 `x y\n1 \t 2\n3\t  4\n` 时，数据行先匹配候选 `\t`，表头按 `\t` 仅有 1 列，随后被
  严格校验拒绝为“第 2 行有 2 列，但表头只有 1 列”，而预期是 `x=[1,3]`、`y=[2,4]`。
- Root invariant：空格、多个空格和制表符属于统一空白分隔规则；探测和校验必须在整份文件中一致。
- Required variants：混合空白同行、空格表头配制表符数据、制表符表头配空格数据；继续保留纯分隔和三列异常测试。
- Rejected shallow fix：仅保留原始正则空白行或只增加纯空格/纯制表符测试，无法解决制表符候选提前命中的规则分裂。
- 独立完整测试仍为 17 passed，但现有测试没有覆盖该组合，因此不足以接受 M1。

## Final inline review — 2026-08-22

- Result: `REVIEW_M1: ACCEPT_M1`
- 混合空白根因已关闭：逗号、分号优先，随后以统一的 `r'\s+'` 规则处理空格和制表符；
  正则分隔路径保留原始数据行，不再把正则文本写回字段。
- 六项合法空白变体与两项列数异常测试独立复跑通过；完整测试为 20 passed，编译检查和
  model-handoff 检查通过。
- 额外临时验证逗号、分号和混合空白各一例，3/3 正确导入，未发现候选顺序回归。
- Implementer 提供的干净 CPython 3.12.14 证据为 20 passed，且 SyntaxWarning 编译检查通过。
- M1 的四项初审缺陷及两轮相邻回归均已关闭；M2～M7 未进入本次审查范围。
