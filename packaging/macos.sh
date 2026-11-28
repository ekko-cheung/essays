#!/usr/bin/env bash
set -euo pipefail

version="${1:?Pass the Cargo package version}"
target="${2:?Pass a macOS Rust target}"
case "$target" in
  aarch64-apple-darwin) arch=arm64 ;;
  x86_64-apple-darwin) arch=x64 ;;
  *) echo "Unsupported target: $target" >&2; exit 1 ;;
esac
staging="$(mktemp -d)"
trap 'rm -rf -- "$staging"' EXIT
bundle="$staging/Essays.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources" dist
install -m 755 "target/$target/release/essays" "$bundle/Contents/MacOS/essays"
cp desktop/assets/icon.icns "$bundle/Contents/Resources/icon.icns"
cat > "$bundle/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Essays</string>
  <key>CFBundleDisplayName</key><string>Essays</string>
  <key>CFBundleExecutable</key><string>essays</string>
  <key>CFBundleIdentifier</key><string>com.essays.app</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>15.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
plutil -lint "$bundle/Contents/Info.plist"

if [[ -n "${MACOS_SIGNING_IDENTITY:-}" ]]; then
  codesign --force --timestamp --options runtime --sign "$MACOS_SIGNING_IDENTITY" "$bundle"
else
  codesign --force --sign - "$bundle"
fi
codesign --verify --deep --strict "$bundle"
ln -s /Applications "$staging/Applications"
image="dist/Essays-$version-macos-$arch.dmg"
# Leave room for filesystem metadata and copy overhead beyond the signed bundle.
bundle_kib="$(du -sk "$staging" | cut -f1)"
image_mib=$(( (bundle_kib + 1023) / 1024 + 32 ))
hdiutil create -megabytes "$image_mib" -volname Essays -srcfolder "$staging" -ov -format UDZO "$image"
if [[ -n "${MACOS_NOTARY_PROFILE:-}" ]]; then
  xcrun notarytool submit "$image" --keychain-profile "$MACOS_NOTARY_PROFILE" --wait
  xcrun stapler staple "$image"
fi
