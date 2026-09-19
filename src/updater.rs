use std::{
    fmt::Write as FmtWrite,
    fs::File,
    io::{Read, Write as IoWrite},
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

use ed25519_dalek::{Signature, VerifyingKey};
use eframe::egui;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const MANIFEST_URL: &str =
    "https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-lite/stable/latest.json";
const RELEASE_URL_PREFIX: &str =
    "https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-lite/releases/";
const SIGNATURE_URL_PREFIX: &str =
    "https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-lite/stable/signatures/";
const MAX_MANIFEST_BYTES: usize = 256 * 1024;
const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;
const UPDATE_PUBLIC_KEY: [u8; 32] = [
    0xe0, 0x08, 0x91, 0x87, 0x1d, 0x27, 0x2e, 0x89, 0x4b, 0xb2, 0xa6, 0xb3, 0x0c, 0x77, 0x71, 0x3d,
    0x71, 0xa7, 0xaf, 0x30, 0x11, 0x41, 0xd7, 0xf9, 0x23, 0xe4, 0xa8, 0x6a, 0x5f, 0x8e, 0xe8, 0x22,
];

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseManifest {
    schema: u32,
    version: String,
    published_at: String,
    notes: String,
    installer_url: String,
    signature_url: String,
    sha256: String,
    size_bytes: u64,
}

enum UpdateState {
    Idle,
    Checking,
    Current,
    Available(ReleaseManifest),
    Downloading {
        release: ReleaseManifest,
        downloaded: u64,
    },
    Installing,
    Error(String),
}

enum UpdateEvent {
    CheckFinished {
        manual: bool,
        result: Result<Option<ReleaseManifest>, String>,
    },
    DownloadProgress(u64),
    DownloadFinished(Result<PathBuf, String>),
}

pub struct WindowsUpdater {
    state: UpdateState,
    events: Receiver<UpdateEvent>,
    sender: Sender<UpdateEvent>,
    dialog_open: bool,
}

impl WindowsUpdater {
    pub fn new(context: egui::Context) -> Self {
        let (sender, events) = mpsc::channel();
        let mut updater = Self {
            state: UpdateState::Idle,
            events,
            sender,
            dialog_open: false,
        };
        if !cfg!(debug_assertions) {
            updater.start_check(context, false);
        }
        updater
    }

    pub fn poll(&mut self, context: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                UpdateEvent::CheckFinished { manual, result } => match result {
                    Ok(Some(release)) => {
                        self.state = UpdateState::Available(release);
                        self.dialog_open = true;
                    }
                    Ok(None) => {
                        self.state = UpdateState::Current;
                        self.dialog_open = manual;
                    }
                    Err(error) if manual => {
                        self.state = UpdateState::Error(error);
                        self.dialog_open = true;
                    }
                    Err(_) => self.state = UpdateState::Idle,
                },
                UpdateEvent::DownloadProgress(downloaded) => {
                    if let UpdateState::Downloading {
                        downloaded: current,
                        ..
                    } = &mut self.state
                    {
                        *current = downloaded;
                    }
                }
                UpdateEvent::DownloadFinished(result) => match result {
                    Ok(path) => match launch_installer(&path) {
                        Ok(()) => self.state = UpdateState::Installing,
                        Err(error) => {
                            self.state = UpdateState::Error(error);
                            self.dialog_open = true;
                        }
                    },
                    Err(error) => {
                        self.state = UpdateState::Error(error);
                        self.dialog_open = true;
                    }
                },
            }
        }
        if matches!(
            self.state,
            UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Installing
        ) {
            context.request_repaint_after(Duration::from_millis(100));
        }
    }

    pub fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let (label, enabled) = match &self.state {
            UpdateState::Idle | UpdateState::Error(_) => ("检查更新", true),
            UpdateState::Checking => ("正在检查…", false),
            UpdateState::Current => ("已是最新版", true),
            UpdateState::Available(release) => {
                if ui.button(format!("更新到 {}", release.version)).clicked() {
                    self.dialog_open = true;
                }
                return;
            }
            UpdateState::Downloading {
                release,
                downloaded,
            } => {
                let percent = downloaded
                    .saturating_mul(100)
                    .checked_div(release.size_bytes)
                    .unwrap_or(0)
                    .min(100);
                ui.add_enabled(false, egui::Button::new(format!("正在下载 {percent}%")))
                    .on_hover_text("下载完成后将自动安装并重新启动");
                return;
            }
            UpdateState::Installing => ("正在安装…", false),
        };
        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
            self.start_check(context, true);
        }
    }

    pub fn show_dialog(&mut self, context: &egui::Context) {
        if !self.dialog_open {
            return;
        }
        let mut open = self.dialog_open;
        let mut begin_download = None;
        let mut close_dialog = false;
        let mut retry = false;
        egui::Window::new("InstPlot Lite 更新")
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .open(&mut open)
            .show(context, |ui| match &self.state {
                UpdateState::Available(release) => {
                    ui.heading(format!("新版本 {} 可用", release.version));
                    ui.label(format!("发布日期：{}", release.published_at));
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .show(ui, |ui| {
                            ui.label(&release.notes);
                        });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("稍后").clicked() {
                            close_dialog = true;
                        }
                        if ui
                            .button(egui::RichText::new("更新并重启").strong())
                            .clicked()
                        {
                            begin_download = Some(release.clone());
                        }
                    });
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "更新会关闭当前程序；如有尚未导出的数据或处理结果，请先选择“稍后”。",
                    );
                    ui.small("安装包会先验证发布签名和 SHA-256，验证失败不会执行。未购买 Windows 代码签名证书，因此系统仍可能显示未知发布者提示。");
                }
                UpdateState::Current => {
                    ui.label(format!("当前已是最新版本 {}。", env!("CARGO_PKG_VERSION")));
                    if ui.button("确定").clicked() {
                        close_dialog = true;
                    }
                }
                UpdateState::Downloading {
                    release,
                    downloaded,
                } => {
                    ui.label(format!("正在下载 InstPlot Lite {}…", release.version));
                    let progress = if release.size_bytes == 0 {
                        0.0
                    } else {
                        *downloaded as f32 / release.size_bytes as f32
                    };
                    ui.add(egui::ProgressBar::new(progress.clamp(0.0, 1.0)).show_percentage());
                    ui.label("下载完成后将自动验证并启动安装程序。");
                }
                UpdateState::Installing => {
                    ui.heading("正在安装更新");
                    ui.label("安装程序将关闭 InstPlot Lite，并在完成后重新打开。请不要重复启动安装程序。");
                }
                UpdateState::Error(error) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, "更新失败");
                    ui.label(error);
                    ui.horizontal(|ui| {
                        if ui.button("关闭").clicked() {
                            close_dialog = true;
                        }
                        if ui.button("重试").clicked() {
                            retry = true;
                        }
                    });
                }
                UpdateState::Idle | UpdateState::Checking => {
                    ui.spinner();
                    ui.label("正在安全检查新版本…");
                }
            });
        self.dialog_open = open && !close_dialog;
        if let Some(release) = begin_download {
            self.start_download(context.clone(), release);
        } else if retry {
            self.start_check(context.clone(), true);
        }
    }

    fn start_check(&mut self, context: egui::Context, manual: bool) {
        if matches!(
            self.state,
            UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Installing
        ) {
            return;
        }
        self.state = UpdateState::Checking;
        self.dialog_open = manual;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = check_for_update();
            let _ = sender.send(UpdateEvent::CheckFinished { manual, result });
            context.request_repaint();
        });
    }

    fn start_download(&mut self, context: egui::Context, release: ReleaseManifest) {
        self.state = UpdateState::Downloading {
            release: release.clone(),
            downloaded: 0,
        };
        self.dialog_open = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = download_installer(&release, &sender);
            let _ = sender.send(UpdateEvent::DownloadFinished(result));
            context.request_repaint();
        });
    }
}

fn check_for_update() -> Result<Option<ReleaseManifest>, String> {
    let agent = http_agent(Duration::from_secs(20));
    let manifest_bytes = read_small_response(&agent, MANIFEST_URL, MAX_MANIFEST_BYTES)?;
    let release: ReleaseManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("更新清单格式错误：{error}"))?;
    validate_signature_location(&release)?;
    let signature_bytes = read_small_response(&agent, &release.signature_url, 64)?;
    verify_manifest_signature(&manifest_bytes, &signature_bytes)?;
    validate_manifest(&release)?;
    let available =
        Version::parse(&release.version).map_err(|error| format!("服务器版本号无效：{error}"))?;
    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|error| format!("当前版本号无效：{error}"))?;
    Ok((available > current).then_some(release))
}

fn http_agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(concat!("InstPlot-Lite/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn read_small_response(agent: &ureq::Agent, url: &str, limit: usize) -> Result<Vec<u8>, String> {
    let mut response = agent
        .get(url)
        .call()
        .map_err(|error| format!("无法访问更新服务器：{error}"))?;
    response
        .body_mut()
        .with_config()
        .limit(limit as u64)
        .read_to_vec()
        .map_err(|error| format!("无法读取更新信息：{error}"))
}

fn verify_manifest_signature(manifest: &[u8], signature: &[u8]) -> Result<(), String> {
    verify_signature(&UPDATE_PUBLIC_KEY, manifest, signature)
}

fn verify_signature(public_key: &[u8; 32], message: &[u8], signature: &[u8]) -> Result<(), String> {
    let signature_bytes: [u8; 64] = signature
        .try_into()
        .map_err(|_| "更新签名长度无效".to_owned())?;
    let verifying_key =
        VerifyingKey::from_bytes(public_key).map_err(|_| "内置更新公钥无效".to_owned())?;
    verifying_key
        .verify_strict(message, &Signature::from_bytes(&signature_bytes))
        .map_err(|_| "更新清单签名验证失败，已拒绝本次更新".to_owned())
}

fn validate_manifest(release: &ReleaseManifest) -> Result<(), String> {
    if release.schema != 1 {
        return Err(format!("不支持的更新清单版本：{}", release.schema));
    }
    Version::parse(&release.version).map_err(|error| format!("服务器版本号无效：{error}"))?;
    let expected_installer = format!(
        "{RELEASE_URL_PREFIX}{}/InstPlot-Lite-{}-windows-x64-setup.exe",
        release.version, release.version
    );
    if release.installer_url != expected_installer {
        return Err("安装包地址不属于受信任的 InstPlot Lite OSS 目录".to_owned());
    }
    if release.size_bytes == 0 || release.size_bytes > MAX_INSTALLER_BYTES {
        return Err("安装包大小超出允许范围".to_owned());
    }
    if release.sha256.len() != 64 || !release.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("安装包 SHA-256 格式无效".to_owned());
    }
    Ok(())
}

fn validate_signature_location(release: &ReleaseManifest) -> Result<(), String> {
    Version::parse(&release.version).map_err(|error| format!("服务器版本号无效：{error}"))?;
    let expected = format!("{SIGNATURE_URL_PREFIX}{}.sig", release.version);
    if release.signature_url != expected {
        return Err("更新签名地址不属于当前版本的受信任 OSS 目录".to_owned());
    }
    Ok(())
}

fn download_installer(
    release: &ReleaseManifest,
    sender: &Sender<UpdateEvent>,
) -> Result<PathBuf, String> {
    let agent = http_agent(Duration::from_secs(15 * 60));
    let mut response = agent
        .get(&release.installer_url)
        .call()
        .map_err(|error| format!("下载安装包失败：{error}"))?;
    if let Some(length) = response.body().content_length()
        && length != release.size_bytes
    {
        return Err(format!(
            "安装包大小不符：预期 {} 字节，服务器返回 {} 字节",
            release.size_bytes, length
        ));
    }
    let version =
        Version::parse(&release.version).map_err(|error| format!("服务器版本号无效：{error}"))?;
    let path = std::env::temp_dir().join(format!(
        "InstPlot-Lite-{}-{}-windows-x64-setup.exe",
        version,
        std::process::id()
    ));
    let temporary_path = path.with_extension("exe.part");
    let result = write_verified_download(
        response.body_mut().as_reader(),
        &temporary_path,
        release,
        sender,
    );
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(error);
    }
    if path.exists() {
        std::fs::remove_file(&path).map_err(|error| format!("无法替换旧临时安装包：{error}"))?;
    }
    std::fs::rename(&temporary_path, &path).map_err(|error| format!("无法准备安装包：{error}"))?;
    Ok(path)
}

fn write_verified_download(
    mut reader: impl Read,
    path: &Path,
    release: &ReleaseManifest,
    sender: &Sender<UpdateEvent>,
) -> Result<(), String> {
    let mut file = File::create(path).map_err(|error| format!("无法创建临时安装包：{error}"))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("读取安装包失败：{error}"))?;
        if count == 0 {
            break;
        }
        downloaded = downloaded
            .checked_add(count as u64)
            .ok_or_else(|| "安装包大小溢出".to_owned())?;
        if downloaded > release.size_bytes || downloaded > MAX_INSTALLER_BYTES {
            return Err("下载内容超过清单声明的大小".to_owned());
        }
        file.write_all(&buffer[..count])
            .map_err(|error| format!("写入临时安装包失败：{error}"))?;
        hasher.update(&buffer[..count]);
        let _ = sender.send(UpdateEvent::DownloadProgress(downloaded));
    }
    file.sync_all()
        .map_err(|error| format!("保存临时安装包失败：{error}"))?;
    if downloaded != release.size_bytes {
        return Err(format!(
            "下载不完整：预期 {} 字节，实际 {} 字节",
            release.size_bytes, downloaded
        ));
    }
    let actual = hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(&mut output, "{byte:02x}").expect("writing to a string cannot fail");
            output
        });
    if !actual.eq_ignore_ascii_case(&release.sha256) {
        return Err("安装包 SHA-256 校验失败，已拒绝执行".to_owned());
    }
    Ok(())
}

fn launch_installer(path: &Path) -> Result<(), String> {
    Command::new(path)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
            "/RESTARTAPPLICATIONS",
            "/SP-",
        ])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法启动更新安装程序：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_hex<const N: usize>(text: &str) -> [u8; N] {
        assert_eq!(text.len(), N * 2);
        let mut output = [0_u8; N];
        for (index, byte) in output.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap();
        }
        output
    }

    fn manifest() -> ReleaseManifest {
        ReleaseManifest {
            schema: 1,
            version: "0.3.3".to_owned(),
            published_at: "2026-09-19".to_owned(),
            notes: "test".to_owned(),
            installer_url: format!(
                "{RELEASE_URL_PREFIX}0.3.3/InstPlot-Lite-0.3.3-windows-x64-setup.exe"
            ),
            signature_url: format!("{SIGNATURE_URL_PREFIX}0.3.3.sig"),
            sha256: "a".repeat(64),
            size_bytes: 123,
        }
    }

    #[test]
    fn validates_expected_release_manifest() {
        assert!(validate_manifest(&manifest()).is_ok());
    }

    #[test]
    fn rejects_unsigned_redirect_targets_and_invalid_hashes() {
        let mut release = manifest();
        release.installer_url = "https://example.com/update.exe".to_owned();
        assert!(validate_manifest(&release).is_err());
        release = manifest();
        release.sha256 = "not-a-hash".to_owned();
        assert!(validate_manifest(&release).is_err());
        release = manifest();
        release.signature_url = "https://example.com/latest.sig".to_owned();
        assert!(validate_signature_location(&release).is_err());
    }

    #[test]
    fn verifies_ed25519_signatures_and_rejects_modified_messages() {
        // RFC 8032, section 7.1, test vector 1 (empty message).
        let public_key: [u8; 32] =
            decode_hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let signature: [u8; 64] = decode_hex(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155\
             5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
                .replace(char::is_whitespace, "")
                .as_str(),
        );
        assert!(verify_signature(&public_key, b"", &signature).is_ok());
        assert!(verify_signature(&public_key, b"changed", &signature).is_err());
    }

    #[test]
    fn production_public_key_accepts_the_release_key_fixture() {
        let manifest = include_bytes!("../tests/fixtures/update-manifest-signed.json");
        let signature: [u8; 64] =
            decode_hex(include_str!("../tests/fixtures/update-manifest-signed.sig.hex").trim());
        verify_manifest_signature(manifest, &signature).unwrap();

        let mut changed = manifest.to_vec();
        changed[0] ^= 1;
        assert!(verify_manifest_signature(&changed, &signature).is_err());
    }

    #[test]
    fn downloaded_installer_must_match_declared_size_and_hash() {
        let bytes = b"test installer bytes";
        let mut release = manifest();
        release.size_bytes = bytes.len() as u64;
        release.sha256 =
            Sha256::digest(bytes)
                .iter()
                .fold(String::with_capacity(64), |mut output, byte| {
                    write!(&mut output, "{byte:02x}").unwrap();
                    output
                });
        let path = std::env::temp_dir().join(format!(
            "instplot-updater-test-{}-{}.part",
            std::process::id(),
            line!()
        ));
        let (sender, _receiver) = mpsc::channel();
        write_verified_download(&bytes[..], &path, &release, &sender).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(&path).unwrap();

        release.sha256 = "0".repeat(64);
        let bad_path = std::env::temp_dir().join(format!(
            "instplot-updater-test-{}-{}.part",
            std::process::id(),
            line!()
        ));
        assert!(write_verified_download(&bytes[..], &bad_path, &release, &sender).is_err());
        std::fs::remove_file(&bad_path).unwrap();
    }
}
