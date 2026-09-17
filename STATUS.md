# 当前项目状态

- 更新日期：`2026-09-17`
- 当前产品：`InstPlot Lite`
- 当前公开版本：`v0.3.2 未签名预览版`
- 当前主线：Rust 原生桌面应用，Windows / macOS / Linux 安装包
- 旧 Python 版：已迁移到独立源码归档仓库，不再打包或作为普通用户安装入口

## 总目标

让不熟悉编程的学生和实验人员能够直接下载安装一个轻量、流畅、可离线运行的实验数据工具，不需要 Python、Rust、Microsoft Excel 或终端命令。

## 已具备的能力

- 导入 TXT、CSV、DAT、TSV、XLSX 和 XLS，支持 UTF-8、UTF-16、常见 GBK 及多工作表 Excel。
- 自动显示点线图；选择 X/Y 列后立即更新。
- 鼠标滚轮缩放、右键平移、单点与框选删除、撤销和重做。
- 对称、归一化、多项式去背底、局部展平和 Savitzky–Golay 去噪。
- 原生多项式、指数、对数、幂函数和自定义表达式拟合，不依赖 SciPy。
- 导出 CSV、XLSX、TSV、TXT，以及白底、黑色刻度、浅灰网格的 PNG。
- Windows x64 Setup EXE、Apple Silicon Mac DMG、Debian/Ubuntu DEB 和 Linux x64 便携包。
- 三系统安装、启动、导入、导出和卸载自动化验证。
- 内置 Arial 度量兼容英文字体和简体中文字体，不依赖系统字体。
- 应用不创建用户数据库、设置、日志或缓存，可按系统标准方式干净卸载。

## 当前验证基线

- Lite 原生测试：`103 passed`
- 最新功能矩阵：Windows、macOS ARM、Ubuntu 和 Linux 安装包全部通过；macOS Intel 自 v0.3.1 起停止维护
- 优化后的 macOS ARM 可执行文件约 `6.8 MB`
- 当前安装包约 `4–6 MB`
- 10 万行、4 列、2 工作表的 1.9 MiB XLSX 在开发用 Apple Silicon Mac 的检查模式中解析约 `0.07 s`，峰值 RSS 约 `38.6 MB`

## 待完成事项

1. 继续验证 v0.3.2 的导入、处理、拟合、导出闭环并收集用户反馈。
2. 购买并配置 Apple Developer ID 和 Microsoft 代码签名证书；在此之前保留清晰的未签名提示。
3. 继续收集可公开或脱敏的真实仪器文件，验证厂商特殊 TXT/DAT 变体。
4. 根据真实用户反馈决定是否增加白色界面主题、更多拟合模型或出版绘图；这些不是当前 Lite 核心目标。

## 文档入口

- 普通用户：[README.md](README.md)
- 文档导航：[docs/README.md](docs/README.md)
- Lite 详细状态：[docs/STATUS.md](docs/STATUS.md)
- Python 原版：[InstPlot](https://github.com/zhiyuzhang001-a11y/InstPlot)

## 下一步

发布并持续验证 v0.3.2 未签名预览版；完成签名条件后再考虑稳定版。
