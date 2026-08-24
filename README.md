# InstPlot - 实验数据可视化工具

<div align="center">

![InstPlot Logo](logo.ico)

**简单、便捷的科研数据绘图软件**

[![Version](https://img.shields.io/badge/version-1.0.0-blue.svg)](#)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](#)

</div>

---

## 📖 简介

**InstPlot** 主要用于实验数据的即时观察和预处理，具有操作简单和响应快速的特点。

### ✨ 核心特性

- 📊 **多数据源导入** - 支持 TXT、CSV、DAT（含自动识别的 VSM 数据）、XLS 和 XLSX
- 🎯 **智能数据处理** - 内置对称处理、归一化、背景去除等功能
- 🖱️ **交互式操作** - 缩放、平移、单点选择和矩形选择
- 💾 **图片导出** - 支持 PNG、JPG、SVG、PDF 等多种图片格式
- 📊 **数据导出** - 支持 CSV、TXT、Excel 等格式
---

## 📚 主要功能

### 1️⃣ 数据导入

**导入方式**：
- 点击工具栏 **"打开文件"** 按钮
- 直接**拖拽文件**到软件窗口

### 2️⃣ 数据可视化

**轴选择**：
- 通过下拉菜单选择 X 轴和 Y 轴数据列，然后点击绘制曲线
- 支持中文列名和 LaTeX 格式

**图表类型**：
- 点线图
- 自动图例生成
- 网格线显示

### 3️⃣ 数据处理功能

#### 🔄 对称处理
将数据沿 Y 轴镜像对称。当我们同时处理多条曲线，其电压或电阻值差别较大，在同一个图形中无法看到它们的形状，”对称处理“可以一键解决

<table>
<tr>
<td width="50%">
<b>处理前</b><br>
<img src="ReadMe图片/bf_对称处理.png" alt="对称处理前" width="100%">
</td>
<td width="50%">
<b>处理后</b><br>
<img src="ReadMe图片/af_对称处理.png" alt="对称处理后" width="100%">
</td>
</tr>
</table>

#### 📏 归一化
将数据缩放到 [0, 1] 范围，便于比较不同曲线横坐标上的差别，例如矫顽场等

<table>
<tr>
<td width="50%">
<b>处理前</b><br>
<img src="ReadMe图片/bf_归一化.png" alt="归一化前" width="100%">
</td>
<td width="50%">
<b>处理后</b><br>
<img src="ReadMe图片/af_归一化.png" alt="归一化后" width="100%">
</td>
</tr>
</table>

#### 🧹 去背底
选择合适的背底区间，对于VSM, 可以取这个范围，注意只需要红色方框中x轴对应的最小值和最大值，图中对应大概为6000，8500，可以移动鼠标观察左下角坐标位置。

<table>
<tr>
<td width="50%">
<b>处理前</b><br>
<img src="ReadMe图片/bf_去背底.png" alt="去背底前" width="100%">
</td>
<td width="50%">
<b>处理后</b><br>
<img src="ReadMe图片/af_去背底.png" alt="去背底后" width="100%">
</td>
</tr>
</table>
对于SMR，也需要找到合适的线性背底，可以找到曲线的两个最低点，大约是96，273。第一次去背底有时候并不完美，原因是最低点没有找准。可以进行第二次去背底，换个点，例如最高点也可以，图中对应4.2, 362.
<table>
<tr>
<td width="33%">
<b>处理前</b><br>
<img src="ReadMe图片/bf_去背底smr.png" alt="去背底前" width="100%">
</td>
<td width="33%">
<b>第一次处理</b><br>
<img src="ReadMe图片/af_去背底smr1.png" alt="去背底后" width="100%">
</td>
<td width="33%">
<b>第二次处理</b><br>
<img src="ReadMe图片/af_去背底smr2.png" alt="去背底后" width="100%">
</td>
</tr>
</table>

#### 🗑️ Remove 功能
快速删除多余的点或实验记录中的跳点，可以鼠标左键单击需要删除的点也可以画矩形框同时删去多个点。

<table>
<tr>
<td width="50%">
<b>删除前</b><br>
<img src="ReadMe图片/bf_remove.png" alt="删除前" width="100%">
</td>
<td width="50%">
<b>删除后</b><br>
<img src="ReadMe图片/af_remove.png" alt="删除后" width="100%">
</td>
</tr>
</table>

### 4️⃣ 交互式操作

**鼠标操作**：
- **左键单击** 高亮显示坐标或选择单个要删除的点
- **左键拖拽** - 拉出矩形选择一个或多个要删除的点
- **滚轮** - 缩放图表
- **右键拖拽** - 移动视图

---

## 🖥️ 系统要求

- **Python**：程序支持 CPython 3.10–3.14；一键安装无需预装 Python。当前 PySide6 的上限是
  Python `<3.15`，因此尚不承诺 Python 3.15。
- **操作系统**：Windows、macOS 或带桌面环境的 Linux。自动化发布矩阵使用 GitHub 当前的
  `windows-latest`、`macos-latest` 和 Ubuntu `ubuntu-latest`；其他版本应以 Qt for Python 当前支持范围为准。
- **内存**：至少 2 GB；处理大型文件时建议 4 GB 或更多。
- **磁盘**：项目所在磁盘至少 1 GB 可用空间；依赖安装在项目内 `.venv`。
- **Linux 图形库**：系统必须提供 `libEGL.so.1`。Linux 入口会在下载 Python 依赖前自动检测；缺少时
  根据 Ubuntu/Debian、Fedora/RHEL、Arch 或 openSUSE 显示对应安装命令并安全退出。安装器不会自行提权。

---

## ⚡ 一键安装与启动

下载并解压对应系统的项目包后，可以直接双击安装入口；也可以在项目目录的终端运行同一个入口。无需预装 Python 或 uv：入口优先使用已有
uv；若没有，则从官方固定地址下载 uv 0.12.5 安装器，核对仓库内固定的 SHA-256 后执行，并只安装到
项目的 `.installer/uv`。uv 会选取已有的兼容 CPython 3.10–3.14；如果电脑上没有，则自动取得托管的
兼容 CPython。项目环境仍只创建在 `.venv`，不会修改系统 Python 或 shell 配置。

发布文件名固定为 `InstPlot-1.0.0-windows.zip`、`InstPlot-1.0.0-macos.zip` 和
`InstPlot-1.0.0-linux.tar.gz`。macOS/Linux 包会保留入口的执行权限。首次打开来自网络的脚本时，Windows
或 macOS 仍可能显示系统安全确认；用户只需核对来源后选择运行/打开，不需要输入终端命令。

- Windows：双击 `install_windows.bat`；也可在 CMD/PowerShell 运行它。
- macOS：双击 `install_macos.command`；发布用压缩包会保留执行权限。
- Linux：双击或运行 `./install_linux.sh`；文件管理器是否允许脚本双击执行取决于桌面设置。

安装成功后会自动启动，并创建普通用户可见的后续入口：

- Windows：桌面上的 `InstPlot` 快捷方式；它使用 `pythonw.exe`，不会打开命令窗口。
- macOS：桌面上的 `InstPlot.command`；双击后在后台启动，终端只短暂出现。
- Linux：桌面环境的应用菜单中的 `InstPlot`。

项目目录中的 `run_instplot.bat`、`run_instplot.command` 或 `run_instplot.sh` 始终作为备用入口。快捷方式
指向当前项目目录，因此安装后不要移动或删除整个 InstPlot 文件夹。修复已有环境时，在终端运行对应
安装入口并增加 `--repair`。

首次安装需要联网下载 uv、CPython（本机没有兼容版本时）及锁定依赖。uv 安装脚本固定为
`https://astral.sh/uv/0.12.5/` 下的系统版本，并在执行前验证 SHA-256；下载或校验失败会直接停止。
Linux 的 `libEGL.so.1` 仍是宿主系统前置条件，安装器不会提权或安装系统软件。安装日志位于项目的
`.install-logs`。已有兼容 Python 时可仅检查共享安装计划而不写入：

```bash
python scripts/install.py --json
```

## 手动安装

### 1. 安装依赖

```bash
# 克隆或下载本项目
git clone https://github.com/zhiyuzhang001-a11y/InstPlot.git
cd InstPlot

# macOS / Linux：使用任一 CPython 3.10–3.14 创建隔离环境
python3 -m venv .venv

source .venv/bin/activate

# Windows PowerShell：创建并激活隔离环境
# py -3 -m venv .venv
# .venv\Scripts\Activate.ps1

# 按 `requirements.lock` 的固定版本安装运行依赖，再安装本项目
python -m pip install --upgrade pip
python -m pip install -r requirements.txt
python -m pip install --no-deps .
```

`requirements.lock` 从最低支持版本 Python 3.10 生成，包含覆盖 3.10–3.14 的环境标记和哈希。使用
经过发布验证的 uv 版本重新生成时运行：

```bash
uv pip compile pyproject.toml --python-version 3.10 --universal --generate-hashes --output-file requirements.lock
```

提交锁文件前必须运行 `python scripts/verify_release.py --check-lock`，并在 Windows、macOS、Linux
安装矩阵中验证。

### 2. 运行程序

```bash
# 直接运行主程序
python -m InstPlot
```



## 📥 获取更新

### 从 GitHub 拉取最新版本

```bash
# 进入项目目录
cd InstPlot

# 获取最新代码
git pull origin main

# 更新锁定依赖与程序
python -m pip install -r requirements.txt
python -m pip install --no-deps .
```

## 诊断日志

程序会保存轮转诊断日志，单个文件最多 1MB，并保留 3 个备份。未捕获错误会显示“复制错误详情”按钮，
便于远程排查；日志不会主动写入导入表格的行内容。

- macOS：`~/Library/Logs/InstPlot/instplot.log`
- Windows：`%LOCALAPPDATA%\InstPlot\Logs\instplot.log`
- Linux：`$XDG_STATE_HOME/instplot/instplot.log`，未设置时为 `~/.local/state/instplot/instplot.log`
- 自定义位置：启动前设置环境变量 `INSTPLOT_LOG_DIR`

## 安装故障排查

- 安装器报告 `repair-needed`：重新运行系统对应入口并增加 `--repair`。安装器不会在未授权时重装环境。
- 安装器报告 `conflict`：不要删除或覆盖提示的 `.venv`/启动器；先备份用户修改，再人工处理冲突。
- Linux 报告找不到 `libEGL.so.1`：复制入口显示的发行版安装命令，完成后重新运行 `./install_linux.sh`。
  Ubuntu/Debian 使用 `libegl1`，Fedora/RHEL 使用 `mesa-libEGL`，Arch 使用 `libglvnd`。
- uv/Python 下载失败：确认可以通过 HTTPS 访问 `astral.sh`、GitHub 和 PyPI 后重试；不需要先手动安装
  Python。若使用离线环境，需要提前准备兼容 CPython 3.10–3.14、uv 缓存和全部锁定依赖。
- 仍然失败：保留 `.install-logs` 中最新安装日志和上方“诊断日志”对应的平台日志，在 GitHub Issue
  中附上系统版本、复现步骤和错误文本；不要上传包含敏感数据的原始实验文件。

## 当前限制

- 自动化样本已经覆盖 TXT、CSV、DAT、XLS 和 XLSX 的主要解析边界，但尚未获得用户的真实仪器
  TXT/DAT/VSM/XLS/XLSX 文件做最终对照，因此该项状态为 `PENDING_USER_VALIDATION`。
- VSM 当前指可从内容识别的 VSM 风格 `.dat` 文件，不代表支持任意 `.vsm` 扩展名或厂商私有变体。
- 安装后会创建 Windows 桌面 `.lnk`、macOS 桌面 `.command` 或 Linux 应用菜单 `.desktop`；已有不同
  内容的同名入口不会被覆盖。它们指向安装目录，移动或删除项目文件夹会使入口失效。
- 本项目不提供 `.app`、DMG、MSI、独立 EXE 或 AppImage。

---

## 🙏 致谢

感谢以下开源项目的支持：
- [Python](https://www.python.org/)
- [PySide6](https://www.qt.io/qt-for-python)
- [Matplotlib](https://matplotlib.org/)
- [NumPy](https://numpy.org/)
- [Pandas](https://pandas.pydata.org/)
- [SciPy](https://scipy.org/)

## 🤝 贡献指南

欢迎提交 Issue 和 Pull Request！

### 报告 Bug
- 请在 GitHub Issues 中详细描述问题
- 包含重现步骤和错误信息
- 附加截图或数据文件（如适用）

### 提交功能建议
- 在 Issues 中开启讨论
- 描述功能的用途和预期表现
- 提供原型或示例

### 代码贡献
1. Fork 本仓库
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

---

## 📞 联系方式

- 🐛 Bug 反馈与功能建议：[GitHub Issues](https://github.com/zhiyuzhang001-a11y/InstPlot/issues)

---

<div align="center">

**⭐ 如果觉得软件好用，欢迎推荐给您的同事和朋友！⭐**

</div>
