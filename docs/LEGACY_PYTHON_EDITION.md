# 旧 InstPlot Python 版

> [!NOTE]
> 本页记录历史版本。普通用户应下载当前的 [InstPlot Lite 原生安装包](../README.md#下载与安装)，不需要安装 Python，也不需要运行本页提到的脚本。

InstPlot 最初使用 Python、PySide6、Matplotlib、NumPy、Pandas 和 SciPy 开发。该版本提供更完整的绘图和科学计算能力，也推动了项目早期的数据解析、处理、撤销历史、性能和三系统安装验证。

随着项目目标转向“让不懂编程的学生能够直接下载安装”，我们新增了 Rust 原生的 InstPlot Lite。Lite 版不依赖 Python 环境，安装包和运行占用更小，当前普通用户文档均以 Lite 版为准。

## 为什么仍保留 Python 代码？

- 它是部分处理算法和行为的参考实现。
- 历史测试、阶段报告和性能数据仍有审计价值。
- 将来如需恢复出版绘图或更复杂科学计算，可以参考已有实现。

## 历史源码与入口

- 主程序：`InstPlot.py`
- Python 项目配置：`pyproject.toml`
- 历史依赖：`requirements.txt`、`requirements.lock`
- 历史脚本安装入口：`install_windows.bat`、`install_macos.command`、`install_linux.sh`
- 历史测试：`tests/`
- 历史计划与合同：`docs/IMPLEMENTATION_PLAN.md`、`docs/M2_*` 至 `docs/M7_*`
- 历史验证报告：`reports/`

这些入口不再是普通用户的推荐安装方式。开发者如需运行旧版，应先阅读历史计划、锁文件和测试要求，避免把 Python 版安装说明误用于 Lite 原生安装包。
