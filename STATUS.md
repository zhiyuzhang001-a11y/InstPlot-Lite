# Current project status

- Date: `2026-08-25`
- Project state: `M3-M7.4 COMPLETE / M7.5 AUTOMATION COMPLETE / PUBLIC RELEASE PENDING_USER_AUTHORIZATION / REAL SAMPLES PENDING_USER_VALIDATION`
- Active plan: `docs/M7_RELEASE_PLAN.md`
- Model Handoff Protocol: `paused`；文件保留，`MODEL_HANDOFF.md` 当前不参与普通工作路由。
- Last verified baseline: local current Python `279 passed`；GitHub Actions run `32681539816` 中 Python
  3.10–3.14 各 `279 passed`，Ubuntu/macOS 各 `279 passed`，Windows `278 passed, 1 skipped`。三系统固定
  uv 自举、托管 CPython 3.14.7、首次/重复/repair、`pip check`、
  TXT/XLSX/CSV/PNG/67 SVG 和 offscreen 绘图均通过。

## Overall goal

不制作独立 App 安装包；逐步完成数据正确性、差量撤销、GUI 流畅性、依赖体积优化，以及 Windows、
macOS、Linux 对应的一键隔离环境安装和启动。

## Completed

- InstPlot Lite 原生拟合已完成：纯 Rust 支持 1–10 阶多项式、指数、对数、幂函数和安全自定义表达式；
  界面支持当前/合并同名列、X/Y 范围和角度/弧度转换，并显示方程、R²、点数及拟合线。本地
  `38 passed`，真实 CoGd 数据的二次拟合原生界面验证为 `R² = 0.928739`。Release 二进制约
  `5.6 MiB`，导入 32 列真实测试文件并导出图片的峰值 RSS 为 `125,370,368 bytes`，相较拟合前只增加
  约 `1.2 MB`；三系统 CI 打包复核等待下一次推送触发。
- 拟合窗口的自定义函数与初始参数已改为带灰度高对比边框、背景、示例和“可编辑”标题的输入区；全局字体顺序
  改为清晰 Latin 字体优先、内置中文字体回退，正文/提示最小字号同步提高。本地原生视觉复核与
  `39 passed` 通过。
- InstPlot Lite 已内置 117 KiB 的 Arial 度量兼容英文字体子集（Liberation Sans 2.1.5，按 OFL 要求
  重命名为 InstPlot Sans），作为比例界面字体首选；中文继续由内置 Noto Sans SC 子集回退。字体随
  Rust 二进制进入 Windows、macOS、Linux 安装包，不依赖系统字体或用户额外安装。`39 passed`，macOS
  原生界面中英混排复核通过；Release 二进制为 `5,985,280 bytes`（增加 `132,096 bytes`），重建 DMG
  为 `3,595,601 bytes`，磁盘映像校验及应用签名验证通过。
- InstPlot Lite 绘图区新增 `12 px` 底部留白，并从绘图高度中等量扣除；不改变数据比例、缩放、左侧布局
  或坐标轴实现。导出裁剪范围同步包含该留白，真实 `0V_IP_1.dat` 导出视觉复核及 `39 passed` 通过。
- 空白启动状态不再显示内部字体加载信息，改为文件导入引导；导入后仍
  自动显示实际导入结果。原生启动界面与 `39 passed` 复核通过。
- InstPlot Lite 常用数据格式与批量导出已完成：导入支持 TXT/CSV/DAT/TSV/XLSX/XLS，多工作表 Excel
  按有效数值工作表拆成独立数据集并跳过空白/说明页；导出支持当前数据集 CSV/XLSX/TSV/TXT，以及全部
  数据集写入单个多工作表 XLSX 或多个文本文件。批量文本导出自动避让磁盘已有文件和同名数据集，不覆盖；
  Excel 读写保持纯 Rust，不需要 Python 或 Microsoft Excel。TSV、旧 XLS、多工作表、删除行往返与同名
  防覆盖测试纳入回归，当前 `44 passed`。Release 二进制为 `6,813,392 bytes`，较加入 Excel 前增加
  `828,112 bytes`；1.9 MiB、10 万行、4 列、2 工作表的 XLSX 解析耗时 `0.07 s`、峰值 RSS
  `38,567,936 bytes`。重建 macOS DMG 为 `4,251,088 bytes`，映像及应用签名验证通过。

- M0 审计、M1 回归/依赖基线已完成。
- M2 数据 I/O 核心和自动化替代验收已接受；真实仪器文件验证延期到 M7。
- M3.1 行为和旧历史基线已接受：八处全量快照，10 步载荷 `640,005,280 bytes`。
- M3.2 已创建五类无 Qt 处理核心和 GUI 薄包装；参数、NaN/Inf、短数组和有限极值第一轮修复完成。
- M3.2 shared-conversion 全矩阵已闭环；本机/clean Python 3.12 wheel 均为 181 passed，`pip check`、
  offscreen 启动和八处 history owner 门禁通过。
- M3.3 差量历史纯核心完成：15 项新核心测试，本机及 clean installed-wheel 完整套件 196 passed；
  列、行、文件和组合命令支持精确 undo/redo，八处旧 GUI owner 尚未迁移。
- M3.4 GUI 历史迁移和 M3.5 验收完成：八处快照归零，redo/会话 reset/原子发布接入；正式基准
  `160,010,560 / 640,005,280 bytes = 25.00%`，十步往返通过。
- M4 GUI/性能、M5 Essentials 功能兼容与三系统体积证据、M6 三系统安装矩阵均已完成。
- M7 自动发布门禁已完成：发布文档、真实仓库元数据、标准库验证器、固定 `uv 0.12.5` 字节级锁
  重建和三系统 CI 均通过；macOS 正式启动器的原生桌面复核也已完成，阶段只等待真实仪器样本。
- M7.4 已完成：支持范围为 CPython 3.10–3.14；三入口可把固定且经 SHA-256 校验的 uv 安装到项目内，
  并由 uv 在需要时提供托管 Python。macOS 本地、三系统 CI 自举、五版本兼容、哈希锁和 271 项回归均通过。
- M7.5 自动化已完成：Windows 桌面 `.lnk`、macOS 桌面 `.command`、Linux 应用菜单 `.desktop` 均通过
  原生验证；三套轻量下载包已构建并上传 CI artifact。公开 GitHub Release 尚未获授权发布。

## Current unresolved issues

1. GUI：大型 `PlotApp` 仍影响维护性，但 M4 的异步导入/拟合、交互重绘和诊断边界已闭环。
2. Footprint：同一 CI 环境、同版本 PySide6 的逻辑字节比较已完成；Essentials 相比完整 PySide6 在
   Linux/macOS/Windows 分别节省 `40.54% / 57.97% / 44.25%`。测量使用临时环境副本，不污染测试环境。
3. Installation：共享安装器和三系统入口已通过原生 CI 的首次、重复、repair 和安装后功能验证；
   Linux 入口会在下载前检测宿主 `libEGL.so.1`，缺少时按发行版显示安装命令但不执行 sudo；日志文件
   使用时间戳前缀和原子唯一分配，避免 Windows 低时钟分辨率下的名称碰撞。
4. Release：README、锁重建、三系统自动发布门禁和 macOS 原生桌面启动已闭环；真实仪器文件仍未取得。
5. Fonts：启动时只使用已安装的命名字体并保留通用字体族回退；缺少 `SimHei` 不再产生重复日志。
6. Packaging：许可证元数据已迁移到 SPDX `MIT` 和 `license-files`，warnings-as-error 构建通过。

## Stage order

`A M3.2 conversion → B M3.3 history core → C M3.4 GUI history → D M3.5 benchmark → E M4 GUI/performance → F M5 footprint → G M6 installers → H M7 release`

## Recording rule

每阶段结束同步更新 `docs/PROJECT_ROADMAP.md`、本文件和对应 `reports/M*-*.md`，记录修改范围、命令、
退出码、测试数量、耗时/内存/体积、残余风险、清理状态和唯一下一动作。后续阶段不自动获得实施授权。

## Next action

`用户确认是否创建公开 GitHub Release v1.0.0，并把三套已验证安装包作为下载资源发布。`
