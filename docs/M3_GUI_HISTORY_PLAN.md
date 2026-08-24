# M3.4 差量历史 GUI 集成合同

- 状态：`COMPLETE`
- 前置：M3.3 `COMPLETE`，纯核心与 installed-wheel 完整基线 `196 passed`。
- 范围：迁移现有八处历史 owner、接入 redo、同步非历史数据替换；不做内存基准和 GUI 性能重构。

## 发布边界

1. 数据导入、手工输入、清空和示例数据属于新会话：调用 `HistoryManager.reset`，清空 undo/redo。
2. 单点/框选删点、删线和五类处理都先计算差量命令，再由 manager 一次发布；生产算法不得先原地修改
   DataFrame。多文件部分成功合为一个 `CompositeCommand`，一次 undo/redo 往返。
3. 取消、参数校验失败、全失败、缺列和数值 no-op 不入栈；新成功操作清空 redo。
4. undo/redo 后统一更新列下拉和绘图；空数据状态清空画布/下拉。失败保留当前可见数据并显示错误。
5. 删除按行位置而非索引标签定位，避免重复索引误删；文件按内部 entry identity 定位，避免重复路径误删。
6. 迁移完成后 `copy.deepcopy(self.loaded_files)` owner 必须从 8 降为 0，且不保留第二套 legacy 历史。

## 失败优先矩阵

- GUI 初始化持有绑定当前 `loaded_files` 的 `HistoryManager`，工具栏同时提供 undo/redo。
- center/normalize：成功、部分成功、全失败、no-op、受过滤行、undo/redo 和原 DataFrame 不被原地发布。
- 删除：单点、框选多文件、重复索引、重复路径、删除全部文件和撤销后刷新。
- 对话框处理：取消不入栈；denoise/local/background 的成功子集只生成一个历史步骤。
- 外部数据会话：加载、输入、清空、示例替换后两个栈均清空。
- 静态门禁：八处旧深拷贝归零，数据处理路径使用差量命令；完整/clean installed-wheel 验证通过。

## 验收与边界

- 本阶段验证 GUI 行为和发布原子性；M3.5 才执行 4×250,000×8×10 的 35% 内存门槛。
- 保留现有状态提示、参数记忆、部分成功和视图保留语义；不改变正常处理数值结果。
- 若发现必须改变领域算法、公共 `loaded_files` 结构或 M3.3 根不变量，停止并回到合同层处理。

## 完成记录

- 五项 GUI 失败优先测试最初全部失败；迁移后新增 redo、统一刷新/会话 reset 和列/行/文件提交边界，
  八处 `copy.deepcopy(self.loaded_files)` 降为零。
- 单点和框选改为行位置，重复索引过滤新增反例通过；五类处理先在临时结果计算，成功子集组成一个命令，
  原 DataFrame 不被原地发布。取消、全失败和 no-op 不入栈。
- M3.4/M3.5 最终共同验收：本机与 clean CPython 3.12.14 installed-wheel 均为
  `203 passed, 6 warnings`；wheel 四模块、67 SVG、`pip check`、编译和 offscreen history binding 通过。
