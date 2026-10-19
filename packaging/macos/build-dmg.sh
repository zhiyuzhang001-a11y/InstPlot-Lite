#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(CDPATH='' cd -- "$script_dir/../.." && pwd)
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
install_dir="/Applications"
install_path="$install_dir/$app_name.app"
install_staging="$install_dir/.InstPlotLite-install-$$"
install_backup="$install_dir/.InstPlotLite-previous-$$"
install_committed=0

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

icon_count=$(find "$resources_dir" -maxdepth 1 -type f -name '*.icns' | wc -l | tr -d ' ')
test "$icon_count" -eq 1

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
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIconFile' "$contents_dir/Info.plist")" = \
    "InstPlotLite"
codesign --force --deep --sign - "$app_dir"
codesign --verify --deep --strict "$app_dir"
"$macos_dir/instplot-lite" --check "$project_dir/tests/fixtures/smoke.csv"

ditto "$app_dir" "$dmg_root/$app_name.app"
ditto "$licenses_dir" "$dmg_root/Licenses"
ln -s /Applications "$dmg_root/Applications"
hdiutil create -quiet -fs HFS+ -format UDBZ -volname "$app_name" \
    -srcfolder "$dmg_root" "$dmg_path"
hdiutil verify "$dmg_path"

if [ "${INSTPLOT_INSTALL_LOCAL:-0}" = "1" ]; then
    cleanup_install() {
        if [ "$install_committed" -ne 1 ] && [ -d "$install_backup" ]; then
            rm -rf "$install_path"
            mv "$install_backup" "$install_path"
        fi
        rm -rf "$install_staging"
    }
    trap cleanup_install EXIT HUP INT TERM

    rm -rf "$install_staging" "$install_backup"
    ditto "$app_dir" "$install_staging"
    codesign --verify --deep --strict "$install_staging"
    "$install_staging/Contents/MacOS/instplot-lite" \
        --check "$project_dir/tests/fixtures/smoke.csv"

    # The signed hidden staging copy is now self-contained. Remove every App
    # bundle under the build tree before registering the canonical install, or
    # Spotlight can index a second bundle with the same identity and stale icon.
    rm -rf "$app_dir" "$iconset_dir" "$dmg_root"

    if [ -d "$install_path" ]; then
        mv "$install_path" "$install_backup"
    fi
    mv "$install_staging" "$install_path"
    codesign --verify --deep --strict "$install_path"
    install_committed=1
    rm -rf "$install_backup"
    trap - EXIT HUP INT TERM

    lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
    "$lsregister" -u "$install_path" 2>/dev/null || true
    "$lsregister" -f "$install_path"
    # Prime the application importer before committing metadata to the index.
    # A newly swapped bundle can otherwise return no Bundle attributes on the
    # first import even though its Info.plist is already valid.
    mdimport -t -d1 "$install_path" >/dev/null 2>&1
    mdimport "$install_path"

    test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
        "$install_path/Contents/Info.plist")" = "$version"
    spotlight_matches=$(mdfind \
        'kMDItemCFBundleIdentifier == "com.zhiyu.instplot-lite"c')
    test "$spotlight_matches" = "$install_path"
fi

# The packaged DMG is the distributable artifact. Keeping either staging app
# makes Spotlight report extra InstPlot Lite installations.
rm -rf "$app_dir" "$iconset_dir" "$dmg_root"

printf '%s\n' "$dmg_path"
