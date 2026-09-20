# InstPlot Lite v0.3.3（未签名预览版）

此版本修复 Windows 最大化状态下的工具窗口显示，并增强 Windows 应用内更新的可靠性。

## 主要变化

- Windows 主窗口最大化时，数据处理、曲线拟合和数据导出改为嵌入主界面的独立工具窗口，可同时打开、移动和分别关闭。
- 点击已经打开的工具功能会将对应窗口调到最前面；恢复普通窗口后仍使用带标题栏和“×”的 Windows 原生窗口。
- 修复 Ed25519 签名文件恰好为 64 字节时被 HTTP 读取上限错误拒绝的问题。
- 更新清单和签名使用独立连接读取，兼容会主动关闭连接的 HTTP 服务。
- Windows 构建新增旧版本检查、下载、验签、校验、覆盖安装和更新后导入测试的完整端到端验证。
- macOS 继续仅提供 Apple Silicon 版本，不再构建 Intel 版本。

## 安全提示

此预览版尚无 Windows Authenticode 或 Apple Developer ID 商业签名，因此 Windows SmartScreen 和 macOS Gatekeeper 仍可能显示未知发布者提示。Windows 应用内更新会独立验证 Ed25519 签名、文件大小和 SHA-256，拒绝被替换或损坏的安装包。

已安装 v0.3.2 的 Windows 用户可以在应用内点击“检查更新”，自动下载并安装 v0.3.3。
