# M7 发布验收计划

- 状态：`M7.1-M7.2 COMPLETE / M7.3 DESKTOP COMPLETE / M7.4 COMPLETE / M7.5 AUTOMATION COMPLETE / PUBLICATION PENDING_USER_AUTHORIZATION / REAL SAMPLES PENDING_USER_VALIDATION`
- 版本：`1.0.0`
- 发布形态：源代码目录加三系统入口；不制作 App、DMG、MSI、EXE 或 AppImage。
- 运行时：CPython 3.10–3.14；由 M7.4 扩展并验证，不承诺当前 PySide6 不支持的 3.15。
- M7.4 扩展：按用户要求改为 CPython 3.10–3.14，并增加无需预装 Python/uv 的安全自举入口；已完成。

## M7.1 自动化发布门禁

1. README 版本必须与 `pyproject.toml` 一致，不得包含占位邮箱或仓库链接。
2. README 必须覆盖 Windows、macOS、Linux、至少 1 GB 磁盘、Linux `libEGL.so.1`、repair、日志、
   数据格式边界和真实样本限制。
3. 使用固定 `uv 0.12.5` 从 `pyproject.toml` 重新生成通用哈希锁，结果必须与 `requirements.lock`
   字节相同。
4. Windows、macOS、Ubuntu 继续运行 M6 的首次安装、健康重复、缺包门禁、repair、安装后冒烟和完整测试。

## M7.2 跨系统体积证据

1. 三系统在安装和 repair 后、加入测试依赖前统计 Essentials 环境的逻辑文件字节数。
2. 在同一环境安装与 Essentials 完全同版本的完整 `PySide6`，再次统计逻辑文件字节数。
3. 记录两者差值和节省比例；若任一系统没有缩小，M5 保持 `PENDING_VALIDATION` 并调查。
4. 体积测量复制现有环境并只修改临时副本，不改变原测试环境、项目运行依赖或用户安装器。

## M7.3 普通用户与真实样本

1. 干净 CI runner 作为无开发依赖的自动验收；macOS 另用正式生成的启动器完成原生 Cocoa 桌面启动。
2. 尚无用户真实 TXT/DAT/VSM/XLS/XLSX 文件；自动 fixture 不能证明厂商私有变体兼容。
3. 在收到样本前，M7 总状态只能是 `PENDING_USER_VALIDATION`，不得宣称最终发布验收完成。
4. 收到样本后只核对列名、列数、数值、编码/分隔符决策和错误位置，不把样本内容提交到公开仓库。

## M7.4 零 Python 前置安装与版本兼容

1. 三系统入口固定 uv 0.12.5 官方安装脚本与 SHA-256，项目内安装且不修改 PATH。
2. uv 在没有兼容解释器时自动取得托管 CPython；安装器接受 3.10、3.11、3.12、3.13、3.14。
3. 哈希锁从最低支持版本 3.10 通用解析；CI 对五个 Python 次版本运行安装后烟雾和完整测试。
4. 原生三系统矩阵强制走本地 uv 自举和 only-managed Python，证明入口不依赖 setup-python 提供的解释器。

## M7.5 双击安装、用户入口与发布包

1. Windows 安装后创建桌面 `.lnk` 并用 `pythonw.exe` 启动；macOS 创建后台启动的桌面 `.command`；
   Linux 创建用户应用菜单 `.desktop`。已有不同同名入口不得覆盖。
2. 构建平台专用轻量包，排除测试、Git 元数据和开发文档；macOS/Linux 包显式保留执行权限。
3. CI 必须验证三系统快捷入口真实存在、三套包内容和权限、五版本兼容及完整回归。
4. CI artifact 不是面向学生的永久发布页；创建公开 GitHub Release 和上传资源属于单独外部发布动作，
   必须取得用户明确授权。

## 验收记录

M7.1-M7.2 及桌面启动修复已由 GitHub Actions run `32678530521` 验收通过，详细证据集中记录到
`reports/M7-release.md`。M7.3 的桌面启动已完成，在收到真实样本前仍保持待验证；`STATUS.md` 和
`docs/PROJECT_ROADMAP.md` 保留唯一下一动作。M7.4 由 GitHub Actions run `32679629432` 验收通过：
三系统强制自举 uv/托管 CPython，五个受支持 Python 次版本的完整测试全部通过。
