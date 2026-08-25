# InstPlot Lite packaging

All packages contain the native Rust executable. End users do not need Python,
Rust, Cargo, or a terminal.

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

## Linux

Run `packaging/linux/build-packages.sh` on Ubuntu 22.04 or a compatible build
host. It creates a DEB for graphical installation on Debian/Ubuntu and a
portable tarball. Desktop package managers install the declared GUI libraries
automatically; users do not need to enter package commands.

The DEB can be removed from the same graphical package manager; package-owned
program, icon, menu, and documentation files are removed together. Portable
users uninstall by deleting the extracted folder. InstPlot Lite creates no
separate user data.
