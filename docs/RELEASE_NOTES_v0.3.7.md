# InstPlot Lite v0.3.7（未签名预览版）

此版本修复 Windows 应用内更新完成后没有自动重新打开程序的问题。

## 主要变化

- Windows 静默更新完成后，由 Inno Setup 的标准安装后启动步骤直接运行安装目录中的新版程序。
- 更新流程不复制程序，也不安装或保留额外的更新助手 EXE。
- 不依赖 Windows Restart Manager 或可选安装任务，更新路径保持简单、明确。
- Windows 自动测试监听系统进程启动事件，确认旧程序被关闭并且新的 `instplot-lite.exe` 确实启动，不再仅凭安装日志判断。
- 继续验证更新清单签名、安装包 SHA-256、文件覆盖结果、程序图标和安装生命周期。

## 更新说明

Windows v0.3.3 及后续版本可从应用内直接更新到本版本。v0.3.2 因旧更新器的 64 字节签名边界问题，仍需手动覆盖安装一次 v0.3.3 或更高版本。

## 安全提示

此预览版尚无 Windows Authenticode 或 Apple Developer ID 商业签名，因此 Windows SmartScreen 和 macOS Gatekeeper 仍可能显示未知发布者提示。Windows 应用内更新会独立验证 Ed25519 签名、文件大小和 SHA-256。
