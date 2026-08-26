# 旧 InstPlot Python 版安装与维护说明

> [!WARNING]
> 本页仅保留旧 Python 版的可重复安装和维护合同。普通用户应下载当前的
> [InstPlot Lite 原生安装包](../README.md#下载与安装)，不需要执行以下命令。

[![Legacy version](https://img.shields.io/badge/version-1.0.0-blue.svg)](../pyproject.toml)
[![Platforms](https://img.shields.io/badge/Windows%20%7C%20macOS%20%7C%20Linux-legacy-555555.svg)](../README.md)

## 历史系统要求

- 支持 CPython 3.10–3.14；旧版一键入口无需预装 Python。
- 需要至少 2 GB 内存和项目所在磁盘至少 1 GB 可用空间。
- Linux 桌面环境必须提供 `libEGL.so.1`。Ubuntu / Debian 可安装
  `libegl1`；其他发行版请使用对应的软件包管理器。
- 真实仪器文件覆盖状态仍为 `PENDING_USER_VALIDATION`，不应把旧版自动化
  测试解释为覆盖所有厂商格式。

## 历史一键入口

- Windows：双击 `install_windows.bat`。
- macOS：双击 `install_macos.command`。
- Linux：运行 `./install_linux.sh`；桌面环境是否允许双击脚本由系统决定。

入口优先使用已有 uv；没有时，从固定的
`https://astral.sh/uv/0.12.5/` 来源下载并校验 uv，在项目内创建 `.venv`，
不会修改系统 Python 或 shell 配置。安装日志保存在项目目录的
`.install-logs`。

安装环境损坏时，在终端给对应入口增加 `--repair`。详细行为合同见
[M6 安装器合同](M6_INSTALLER_CONTRACT.md)。

## 开发者手动运行

在兼容的 Python 环境中安装锁定依赖后，可运行：

```bash
python -m InstPlot
```

重新生成通用依赖锁文件的规范命令是：

```bash
uv pip compile pyproject.toml --python-version 3.10 --universal --generate-hashes --output-file requirements.lock
```

提交锁文件前运行：

```bash
python scripts/verify_release.py --check-docs --check-lock
```

问题请提交到 [GitHub Issues](https://github.com/zhiyuzhang001-a11y/InstPlot/issues)。
