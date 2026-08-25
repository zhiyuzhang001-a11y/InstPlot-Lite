#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(CDPATH= cd -- "$script_dir/../.." && pwd)
repository_dir=$(CDPATH= cd -- "$project_dir/.." && pwd)
package_dir="$project_dir/target/package"
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$project_dir/Cargo.toml" | head -n 1)
machine_arch=$(uname -m)

case "$machine_arch" in
    x86_64) deb_arch=amd64 ;;
    aarch64|arm64) deb_arch=arm64 ;;
    *) printf 'Unsupported Linux architecture: %s\n' "$machine_arch" >&2; exit 1 ;;
esac

if [ "${INSTPLOT_SKIP_BUILD:-0}" != "1" ]; then
    cd "$project_dir"
    cargo build --release --locked
fi

test -x "$project_dir/target/release/instplot-lite"
test -f "$repository_dir/InP_logo.png"
test -f "$repository_dir/LICENSE"
test -f "$project_dir/THIRD_PARTY_NOTICES.md"
test -f "$project_dir/assets/OFL-Liberation.txt"
test -f "$project_dir/assets/OFL.txt"

stage_dir="$package_dir/linux-stage-$deb_arch"
portable_dir="$package_dir/InstPlot-Lite-$version-linux-$machine_arch"
deb_path="$package_dir/instplot-lite_${version}_${deb_arch}.deb"
portable_path="$package_dir/InstPlot-Lite-$version-linux-$machine_arch.tar.gz"

mkdir -p "$package_dir"
rm -rf "$stage_dir" "$portable_dir"
rm -f "$deb_path" "$portable_path"
mkdir -p \
    "$stage_dir/DEBIAN" \
    "$stage_dir/usr/bin" \
    "$stage_dir/usr/share/applications" \
    "$stage_dir/usr/share/icons/hicolor/512x512/apps" \
    "$stage_dir/usr/share/doc/instplot-lite/licenses" \
    "$portable_dir/licenses"

install -m 755 "$project_dir/target/release/instplot-lite" "$stage_dir/usr/bin/instplot-lite"
install -m 644 "$script_dir/instplot-lite.desktop" \
    "$stage_dir/usr/share/applications/instplot-lite.desktop"
install -m 644 "$repository_dir/InP_logo.png" \
    "$stage_dir/usr/share/icons/hicolor/512x512/apps/instplot-lite.png"
install -m 644 "$repository_dir/LICENSE" "$stage_dir/usr/share/doc/instplot-lite/copyright"
install -m 644 "$project_dir/THIRD_PARTY_NOTICES.md" \
    "$stage_dir/usr/share/doc/instplot-lite/THIRD_PARTY_NOTICES.md"
install -m 644 "$project_dir/assets/OFL-Liberation.txt" \
    "$stage_dir/usr/share/doc/instplot-lite/licenses/OFL-Liberation.txt"
install -m 644 "$project_dir/assets/OFL.txt" \
    "$stage_dir/usr/share/doc/instplot-lite/licenses/OFL-Noto-Sans-SC.txt"

installed_size=$(du -sk "$stage_dir/usr" | cut -f1)
cat > "$stage_dir/DEBIAN/control" <<EOF
Package: instplot-lite
Version: $version
Section: science
Priority: optional
Architecture: $deb_arch
Installed-Size: $installed_size
Maintainer: InstPlot Project <noreply@github.com>
Depends: libc6 (>= 2.35), libgcc-s1, libgl1, libx11-6, libxcursor1, libxi6, libxinerama1, libxkbcommon0, libxrandr2, libwayland-client0
Recommends: xdg-desktop-portal, xdg-desktop-portal-gtk
Description: Lightweight native numeric data plotter
 InstPlot Lite imports text and Excel instrument data, draws point-line curves,
 supports point deletion with undo/redo, and exports data and PNG files.
EOF

dpkg-deb --build --root-owner-group "$stage_dir" "$deb_path"
install -m 755 "$project_dir/target/release/instplot-lite" "$portable_dir/instplot-lite"
install -m 644 "$repository_dir/LICENSE" "$portable_dir/licenses/LICENSE.txt"
install -m 644 "$project_dir/THIRD_PARTY_NOTICES.md" \
    "$portable_dir/licenses/THIRD_PARTY_NOTICES.md"
install -m 644 "$project_dir/assets/OFL-Liberation.txt" \
    "$portable_dir/licenses/OFL-Liberation.txt"
install -m 644 "$project_dir/assets/OFL.txt" \
    "$portable_dir/licenses/OFL-Noto-Sans-SC.txt"
tar -C "$package_dir" -czf "$portable_path" "$(basename "$portable_dir")"
dpkg-deb --info "$deb_path" >/dev/null

printf '%s\n%s\n' "$deb_path" "$portable_path"
