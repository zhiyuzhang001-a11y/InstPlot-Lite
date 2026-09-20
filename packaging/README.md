# InstPlot Lite packaging

All packages contain the native Rust executable. End users do not need Python,
Rust, Cargo, or a terminal.

Every package also contains the project MIT license, `THIRD_PARTY_NOTICES`, and
the complete SIL Open Font License texts for the bundled Liberation Sans and
Noto Sans SC subsets. macOS keeps a copy both inside the app bundle and in a
visible `Licenses` folder on the DMG; Windows installs them under
`licenses`; Linux installs them under its standard documentation directory or
the portable archive's `licenses` folder.

## macOS

Run `packaging/macos/build-dmg.sh`. It creates an application bundle, applies an
ad-hoc signature for structural verification, runs the bundled executable's
import smoke check, and creates a DMG in `target/package`.

The local DMG is not notarized. Public releases still require a Developer ID
Application signature and Apple notarization in CI.

To uninstall, move `InstPlot Lite.app` from Applications to the Trash. The app
does not create settings, databases, logs, or caches elsewhere in the user's
Library, so no additional cleanup is required.

## Windows

Build the release executable, set `INSTPLOT_VERSION`, and compile
`packaging/windows/InstPlotLite.iss` with Inno Setup 6. The per-user installer
adds a Start Menu shortcut and offers an optional desktop shortcut without
requiring administrator privileges. CI fetches the Simplified Chinese Inno
Setup translation from a pinned upstream revision and verifies its SHA-256
checksum before compiling the bilingual installer.

The installer registers InstPlot Lite in Windows **Settings > Apps**. Its Inno
Setup uninstaller removes the executable and the Start Menu and optional desktop
shortcuts. The app does not leave user data behind.

### Windows updates through OSS

Release builds check the signed update manifest at
`https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-lite/stable/latest.json`.
Only Windows uses this update channel. macOS and Linux never contact OSS.

The manifest has a versioned detached Ed25519 signature under
`stable/signatures/`, and the
installer is checked against the signed size and SHA-256 before execution. This
does not replace Authenticode and therefore does not suppress Windows
SmartScreen, but it prevents an unsigned or modified OSS object from being run
by the updater.

After downloading the Windows installer artifact, prepare the exact upload tree
with:

```sh
python3 packaging/windows/prepare-oss-release.py \
  target/package/InstPlot-Lite-0.3.5-windows-x64-setup.exe \
  --private-key /secure/path/update-signing-key.pem \
  --notes "Release notes"
```

Upload the versioned installer first, the versioned `.sig` second, and
`latest.json` last. Updating that single final object atomically activates the
release. The private key must never be committed or uploaded to OSS.
Its corresponding public-key SHA-256 fingerprint is
`158dbcc357d5c1d11434d38bc8c522ed598b0273a16f85d5af0cd71acd4c9333`.

The Windows packaging job also runs an isolated end-to-end updater test. It
installs a test-enabled old client, serves a newly generated signed manifest
and the normal candidate installer from a loopback HTTP server, and then makes
the installed client perform the real check, download, Ed25519 verification,
SHA-256 verification, installer launch, and in-place replacement flow. The job
passes only when the installed executable is byte-for-byte identical to the
candidate build and its importer smoke check succeeds. The `updater-e2e` Cargo
feature and endpoint overrides are never enabled in public packages.

## Linux

Run `packaging/linux/build-packages.sh` on Ubuntu 22.04 or a compatible build
host. It creates a DEB for graphical installation on Debian/Ubuntu and a
portable tarball. Desktop package managers install the declared GUI libraries
automatically; users do not need to enter package commands.

The DEB can be removed from the same graphical package manager; package-owned
program, icon, menu, and documentation files are removed together. Portable
users uninstall by deleting the extracted folder. InstPlot Lite creates no
separate user data.
