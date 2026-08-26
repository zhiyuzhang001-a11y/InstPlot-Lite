# 当前项目状态

- 更新日期：`2026-08-26`
- 当前产品：`InstPlot Lite`
- 当前公开版本：`v0.2.0 未签名预览版`
- 当前主线：Rust 原生桌面应用，Windows / macOS / Linux 安装包
- 旧 Python 版：保留为参考实现和历史工程记录，不再作为普通用户推荐入口

## 总目标

让不熟悉编程的学生和实验人员能够直接下载安装一个轻量、流畅、可离线运行的实验数据工具，不需要 Python、Rust、Microsoft Excel 或终端命令。

## 已具备的能力

- 导入 TXT、CSV、DAT、TSV、XLSX 和 XLS，支持 UTF-8、UTF-16、常见 GBK 及多工作表 Excel。
- 自动显示点线图；选择 X/Y 列后立即更新。
- 鼠标滚轮缩放、右键平移、单点与框选删除、撤销和重做。
- 对称、归一化、多项式去背底、局部展平和 Savitzky–Golay 去噪。
- 原生多项式、指数、对数、幂函数和自定义表达式拟合，不依赖 SciPy。
- 导出 CSV、XLSX、TSV、TXT，以及白底、黑色刻度、浅灰网格的 PNG。
- Windows x64 Setup EXE、Apple Silicon / Intel Mac DMG、Debian/Ubuntu DEB 和 Linux x64 便携包。
- 三系统安装、启动、导入、导出和卸载自动化验证。
- 内置 Arial 度量兼容英文字体和简体中文字体，不依赖系统字体。
- 应用不创建用户数据库、设置、日志或缓存，可按系统标准方式干净卸载。

## 当前验证基线

- Lite 原生测试：`48 passed`
- 最新功能矩阵：Windows、macOS ARM、macOS Intel、Ubuntu 和 Linux 安装包全部通过
- 优化后的 macOS ARM 可执行文件约 `6.8 MB`
- 当前安装包约 `4–6 MB`
- 10 万行、4 列、2 工作表的 1.9 MiB XLSX 在开发用 Apple Silicon Mac 的检查模式中解析约 `0.07 s`，峰值 RSS 约 `38.6 MB`

## 待完成事项

1. 发布包含近期滚轮缩放、统一主题、许可证和白底 PNG 改进的新预览版本。
2. 购买并配置 Apple Developer ID 和 Microsoft 代码签名证书；在此之前保留清晰的未签名提示。
3. 继续收集可公开或脱敏的真实仪器文件，验证厂商特殊 TXT/DAT 变体。
4. 根据真实用户反馈决定是否增加白色界面主题、更多拟合模型或出版绘图；这些不是当前 Lite 核心目标。

## 文档入口

- 普通用户：[README.md](README.md)
- 文档导航：[docs/README.md](docs/README.md)
- Lite 详细状态：[instplot-lite/docs/STATUS.md](instplot-lite/docs/STATUS.md)
- 旧 Python 版：[docs/LEGACY_PYTHON_EDITION.md](docs/LEGACY_PYTHON_EDITION.md)

## 下一步

完成 GitHub 用户文档重组后，准备并发布包含当前主分支改进的新未签名预览版。
