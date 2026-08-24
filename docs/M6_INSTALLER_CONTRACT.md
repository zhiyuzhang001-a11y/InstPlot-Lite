# M6 三系统一键安装合同

- 状态：`M6 COMPLETE / 2026-08-24 ZERO-PYTHON AMENDMENT COMPLETE`
- 目标：不制作 App/EXE/DMG/MSI/AppImage；用户运行系统对应入口，在项目内创建 `.venv`、安装、验证并
  启动 InstPlot，不修改系统 Python、不请求管理员权限。

## 安全与状态合同

1. `scripts/install.py` 只使用 Python 标准库；默认 dry-run，只有 `--apply` 可以写入。
2. 项目根必须包含 `pyproject.toml`、`requirements.lock`、`InstPlot.py` 和验证脚本；`.venv` 或启动器的
   符号链接、非预期文件类型及用户修改均报告 `conflict`，不得覆盖或删除。
3. 状态固定为 `missing / healthy / repair-needed / conflict`。`repair-needed` 只有显式 `--repair --apply`
   才重新安装；`healthy` 重复运行不得重建环境。
4. 接受 CPython 3.10–3.14；命令全部以参数数组执行，不拼接 shell 字符串，中文、空格和 shell 特殊字符
   路径不得改变目标。
5. 安装顺序固定为：创建项目内 `.venv` → 哈希锁依赖 → `--no-deps` 安装项目 → `pip check` →
   `verify_install.py`。任一步失败返回非零，不发布“健康”状态。
6. apply 日志只写项目 `.install-logs/`，不记录完整环境变量或用户数据。失败输出日志位置和安全的重试
   命令；不静默下载系统软件。入口可下载固定 uv/托管 CPython/锁定 Python 依赖，但不得提权或修改
   shell 配置。
7. 启动器只在缺失时原子创建；已有内容必须与模板逐字一致。Unix 启动器设可执行位，Windows 使用
   `%~dp0`，macOS/Linux 使用脚本自身目录，均不依赖当前工作目录。
8. 系统入口在核心安装成功后创建用户级快捷入口：Windows 桌面 `.lnk`、macOS 桌面 `.command`、Linux
   应用菜单 `.desktop`。相同入口可重复使用；任何已有不同内容或目标的同名入口必须保留，不得覆盖。

## 系统入口

- Windows：`install_windows.bat`；生成 `run_instplot.bat` 和桌面 `InstPlot.lnk`，快捷方式使用 `pythonw.exe`。
- macOS：`install_macos.command`；生成 `run_instplot.command` 和桌面 `InstPlot.command`，后者后台启动。
- Linux：`install_linux.sh`；生成 `run_instplot.sh` 和应用菜单 `InstPlot.desktop`。
- 三个入口优先使用现有 uv；没有 uv 时下载固定 `0.12.5` 官方安装器，校验预置 SHA-256 后安装到
  项目 `.installer/uv`，不修改 PATH。uv 选取已有兼容 CPython；没有时自动下载托管 CPython。

## 验收矩阵

- 纯核心：dry-run 零写入、路径安全、Python 版本、四状态、命令参数、启动器冲突与原子创建。
- 本机真实流程：首次安装、健康重复运行、显式 repair、中文空格路径、失败日志、TXT/XLSX 导入、绘图、
  PNG/CSV/XLSX 导出。
- Windows/Linux：入口语法和纯逻辑先由本机冻结；首次/重复/repair/GUI 必须由对应系统 CI 或实机通过，
  否则保持 pending。
