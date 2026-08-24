# Current project status

- Date: `2026-08-24`
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
