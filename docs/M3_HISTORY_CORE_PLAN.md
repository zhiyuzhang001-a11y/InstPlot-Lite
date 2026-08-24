# M3.3 差量历史纯核心执行合同

- 状态：`COMPLETE`
- 范围：只新增 `instplot_history.py`、纯核心测试和模块打包声明；不接 GUI，不修改现有八处历史入口。
- 前置：M3.2 `COMPLETE`，完整基线 `181 passed`，旧十步载荷 `640,005,280 bytes`。

## 根不变量

1. HistoryManager 为当前 `loaded_files: list[tuple[path, DataFrame]]` 维护独立的 entry identity；重复路径、
   相同 basename 和同一 DataFrame 引用不能让命令作用到错误位置。
2. 命令只保留变化所需数据：列命令保存选中位置 before/after，删行保存被删行，删文件保存对象引用；
   不保留整个 `loaded_files` 深拷贝。
3. execute、undo、redo 后路径顺序、列顺序、dtype、索引和值精确符合对应状态；redo 发布保存的 after，
   不重新运行算法。
4. CompositeCommand 全部成功才提交；任一步失败必须确定性回滚，两个栈和可见状态均不变化。
5. 新操作清空 redo；最多保留 10 步并淘汰最旧命令；no-op 不入栈；状态替换使用 reset 清空历史。

## 核心 API

- `HistoryError(code, reason)`；
- `HistoryManager(loaded_files, max_steps=10)`：`execute`、`undo`、`redo`、`reset`、栈计数和 payload；
- `ColumnPatchCommand.create(...)`；
- `DeleteRowsCommand.create(...)`；
- `DeleteFilesCommand.create(...)`；
- `CompositeCommand(commands)`。

HistoryManager 使用每个列表位置独立的内部 token，不把 identity 写入 DataFrame，也不依赖路径唯一。
命令通过 manager 替换目标位置的 DataFrame 副本，因此同一 DataFrame 被两个列表项引用时只改变目标项。

## 失败优先矩阵

- 列：全量/局部、首中末、不连续位置、不同 dtype、非默认/重复索引、重复列歧义、no-op、状态漂移。
- 行：首中末、不连续/全部行、重复索引、混合 dtype、空选择和越界位置。
- 文件：首中末、多文件、重复路径/basename、共享 DataFrame 引用、引用身份、顺序恢复。
- 顺序：execute→undo→redo、十步往返、上限淘汰、undo 后新操作、reset。
- 失败：命令创建后状态改变、组合中途注入失败、undo/redo 中途失败；不用 sleep。
- payload：列变化不计未变列/文件；删行只计被删行；删文件不计引用 DataFrame 内容。

## 验收

- 纯模块不得导入 Qt、Matplotlib 或 `InstPlot`；输入失败只返回具名 `HistoryError`。
- 定向测试、完整项目、编译、wheel 内容和 `git diff --check` 通过；八处 legacy owner 完全不变。
- M3.3 只证明纯核心；GUI 集成和旧快照移除属于阶段 C。

## 完成记录

- 失败优先基线在收集阶段以 `ModuleNotFoundError: instplot_history` 退出；生产实现前没有同名核心。
- 新增按列表位置分配独立 token 的 `HistoryManager`，以及列补丁、删行、删文件和原子组合命令；
  重复路径、共享 DataFrame、重复索引、dtype、十步淘汰、redo 清空、no-op 和状态漂移矩阵通过。
- 纯核心 15 项、M3 定向 108 项、本机完整 `196 passed, 6 warnings`；编译、差异检查和原八处 owner
  门禁通过。clean CPython 3.12.14 installed-wheel 完整套件为 `196 passed, 6 warnings in 58.74s`，
  `pip check`、四个模块 wheel 内容和 offscreen 启动通过。
- 临时环境和 wheel 已移至废纸篓；项目构建元数据和缓存已清理。GUI 尚未接入，属于 M3.4。
