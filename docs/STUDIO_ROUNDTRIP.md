# Studio 单组数据编辑

支持 Studio 发起的 `--studio-edit /private/session/request.json` 专用会话。
该窗口只载入目标数据组并恢复当前 X/Y 选列。用户可正常处理数据、增加列、删点及撤销。
“发回 Studio”发送当前目标数据组及选列，保留空单元和 alive 状态；不发送拟合 overlay，不覆盖原始文件。

支持持续会话 V2：每次结果写入同一私有目录对应修订的 `result.json` 或 `result-N.json`，完整发布一次，不能覆盖已有结果。Studio 确认后发送下一修订；相同数据保留 Lite 撤销历史，Studio 撤销/重做或切换目标则同步新基线并重置 Lite 历史。同一窗口可反复处理并发送，不必重开。双精度值按位往返，不隐式处理数值。

身份沿用原请求，不能将其他导入文件或拟合曲线作为返回目标。发送后 Lite 不自动关闭，Studio 单独确认更新；关闭/换项目/结束联动后的旧会话不会更新新项目。

协议 schema 1 与 Studio `lite_edit_protocol.rs` 保持一致。本功能不改常规 CSV/Excel、算法、更新器、生产版本或发布渠道。独立于旧的 Lite → Studio 新图 handoff。
