#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(CDPATH= cd -- "$script_dir/../.." && pwd)
repository_dir="$project_dir"
package_dir="$project_dir/target/package"
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$project_dir/Cargo.toml" | head -n 1)
architecture=$(uname -m)
app_name="InstPlot Lite"
app_dir="$package_dir/$app_name.app"
contents_dir="$app_dir/Contents"
resources_dir="$contents_dir/Resources"
licenses_dir="$resources_dir/Licenses"
macos_dir="$contents_dir/MacOS"
iconset_dir="$package_dir/InstPlotLite.iconset"
dmg_root="$package_dir/dmg-root"
dmg_path="$package_dir/InstPlot-Lite-$version-macos-$architecture.dmg"

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

mkdir -p "$package_dir"
rm -rf "$app_dir" "$iconset_dir" "$dmg_root"
rm -f "$dmg_path"
mkdir -p "$macos_dir" "$licenses_dir" "$iconset_dir" "$dmg_root"

cp "$project_dir/target/release/instplot-lite" "$macos_dir/instplot-lite"
chmod 755 "$macos_dir/instplot-lite"
install -m 644 "$repository_dir/LICENSE" "$licenses_dir/LICENSE.txt"
install -m 644 "$project_dir/THIRD_PARTY_NOTICES.md" \
    "$licenses_dir/THIRD_PARTY_NOTICES.md"
install -m 644 "$project_dir/assets/OFL-Liberation.txt" \
    "$licenses_dir/OFL-Liberation.txt"
install -m 644 "$project_dir/assets/OFL.txt" \
    "$licenses_dir/OFL-Noto-Sans-SC.txt"

for icon_size in 16 32 128 256 512; do
    retina_size=$((icon_size * 2))
    sips -z "$icon_size" "$icon_size" "$repository_dir/InP_logo.png" \
        --out "$iconset_dir/icon_${icon_size}x${icon_size}.png" >/dev/null
    sips -z "$retina_size" "$retina_size" "$repository_dir/InP_logo.png" \
        --out "$iconset_dir/icon_${icon_size}x${icon_size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset_dir" -o "$resources_dir/InstPlotLite.icns"

cat > "$contents_dir/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>zh_CN</string>
    <key>CFBundleDisplayName</key>
    <string>$app_name</string>
    <key>CFBundleExecutable</key>
    <string>instplot-lite</string>
    <key>CFBundleIconFile</key>
    <string>InstPlotLite</string>
    <key>CFBundleIdentifier</key>
    <string>com.zhiyu.instplot-lite</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$app_name</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$version</string>
    <key>CFBundleVersion</key>
    <string>$version</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.education</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

plutil -lint "$contents_dir/Info.plist"
codesign --force --deep --sign - "$app_dir"
codesign --verify --deep --strict "$app_dir"
"$macos_dir/instplot-lite" --check "$project_dir/tests/fixtures/smoke.csv"

ditto "$app_dir" "$dmg_root/$app_name.app"
ditto "$licenses_dir" "$dmg_root/Licenses"
ln -s /Applications "$dmg_root/Applications"
hdiutil create -quiet -fs HFS+ -format UDBZ -volname "$app_name" \
    -srcfolder "$dmg_root" "$dmg_path"
hdiutil verify "$dmg_path"

printf '%s\n' "$dmg_path"
