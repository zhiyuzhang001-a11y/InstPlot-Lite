# InstPlot Lite v0.3.6（未签名预览版）

此版本修复 Windows 自动更新后的重启行为，并为 Windows 应用和安装包补齐正式图标。

## 主要变化

- 点击“更新并重启”后，安装器会在完成覆盖安装后自动重新打开 InstPlot Lite。
- Windows 主程序嵌入 InstPlot Lite 图标，窗口、任务栏、开始菜单和安装包不再使用默认程序图标。
- 更新流程只使用一个明确的重启路径，避免 Windows Restart Manager 与安装器重复启动应用。
- Windows 打包测试会验证可执行文件图标、静默安装、覆盖安装和应用内更新，并通过安装日志确认更新后启动步骤确实执行。
- 发布工作流新增不发布的 Windows 专项验证模式，安装器和更新器修改可先完整测试，再决定是否发布。

## 更新说明

Windows v0.3.3 及后续版本可从应用内直接更新到本版本。v0.3.2 因旧更新器的 64 字节签名边界问题，仍需手动覆盖安装一次 v0.3.3 或更高版本。

## 安全提示

此预览版尚无 Windows Authenticode 或 Apple Developer ID 商业签名，因此 Windows SmartScreen 和 macOS Gatekeeper 仍可能显示未知发布者提示。Windows 应用内更新会独立验证 Ed25519 签名、文件大小和 SHA-256。
