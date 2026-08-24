# InstPlot Lite packaging

All packages contain the native Rust executable. End users do not need Python,
Rust, Cargo, or a terminal.

## macOS

Run `packaging/macos/build-dmg.sh`. It creates an application bundle, applies an
ad-hoc signature for structural verification, runs the bundled executable's
import smoke check, and creates a DMG in `target/package`.

The local DMG is not notarized. Public releases still require a Developer ID
Application signature and Apple notarization in CI.

## Windows

Build the release executable, set `INSTPLOT_VERSION`, and compile
`packaging/windows/InstPlotLite.iss` with Inno Setup 6. The per-user installer
adds a Start Menu shortcut and offers an optional desktop shortcut without
requiring administrator privileges.

## Linux

Run `packaging/linux/build-packages.sh` on Ubuntu 22.04 or a compatible build
host. It creates a DEB for graphical installation on Debian/Ubuntu and a
portable tarball. Desktop package managers install the declared GUI libraries
automatically; users do not need to enter package commands.
