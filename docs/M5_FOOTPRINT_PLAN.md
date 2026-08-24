# M5 依赖与安装体积计划

- 状态：`IMPLEMENTED_ON_MACOS / WINDOWS_LINUX_VALIDATION_PENDING`
- 前置：M4 `COMPLETE`，本机及 clean CPython 3.12 installed-wheel `238 passed`。

## 目标与验收

1. 以相同 Python 3.12、锁定科学计算依赖和测试矩阵，对比完整 `PySide6` 与
   `PySide6-Essentials` 的环境落盘体积、启动峰值内存和 GUI 功能。
2. 只有 Core、Gui、Widgets、Svg、Matplotlib Qt backend、QtAwesome、完整测试、wheel、`pip check` 和
   offscreen 启动全部通过，才允许修改项目元数据与通用哈希锁文件。
3. 确认 wheel 不包含 README 图片，核对每项直接依赖用途；不以替换 Pandas、SciPy 或 Matplotlib 换取
   不可接受的行为变化。
4. macOS 完成后，Windows/Linux 必须通过对应安装流程或 CI 复验，才把 M5 标为 `COMPLETE`。

## 当前结果

- 完整环境 `1,487,064 KiB`；导入并测试后的 Essentials 环境 `607,412 KiB`，减少 `59.15%`。
- `PySide6-Addons` 分发文件约 `885,118,755 bytes`；项目没有 Addons 模块导入。
- 三次 offscreen 启动峰值中位数由 `351,797,248` 降至 `347,979,776 bytes`，减少 `1.09%`；主要收益
  是安装/下载体积，而非运行期内存，这符合 Addons 原本未加载的事实。
- `pyproject.toml` 已改为 `PySide6-Essentials>=6.6,<7`，通用 Python 3.12 哈希锁只包含 Essentials，
  不含 PySide6 元包和 Addons。
- 最终锁定环境的 `pip check`、238 项、QtCore/Gui/Widgets/Svg、QtAwesome 和 offscreen 主窗口通过。
  Windows/Linux 当前没有可运行环境；本机 Docker CLI 存在但 daemon 未运行，未伪造容器结果。

## 下一步

在 M6 为 Windows、macOS、Linux 建立各自一键安装入口和 dry-run/repair 测试；安装矩阵同时完成 M5
剩余的 Windows/Linux Essentials 验证。
