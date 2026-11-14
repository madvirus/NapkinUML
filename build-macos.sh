#!/bin/sh
# Builds the macOS version of NapkinUML on a Mac: one NapkinUML.app that runs on both
# Apple Silicon and Intel Macs, plus a .zip and a .dmg of it, all in dist/.
# Needs: Xcode Command Line Tools (xcode-select --install) and Rust (https://rustup.rs).
# The app gets an ad-hoc signature only: the first time, open it with right-click > Open
# (or run: xattr -dr com.apple.quarantine dist/NapkinUML.app).
set -e
cd "$(dirname "$0")"

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

APP=dist/NapkinUML.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/NapkinUML" \
  target/aarch64-apple-darwin/release/NapkinUML \
  target/x86_64-apple-darwin/release/NapkinUML
cp icons/napkin-uml.icns "$APP/Contents/Resources/napkin-uml.icns"
cp LICENSE-MIT LICENSE-APACHE "$APP/Contents/Resources/"
# Open source notices (needs cargo-about: cargo install --locked cargo-about)
if cargo about --version >/dev/null 2>&1; then
  cargo about generate --fail about.hbs -o dist/THIRD-PARTY-LICENSES.html
  cp dist/THIRD-PARTY-LICENSES.html "$APP/Contents/Resources/"
else
  echo "cargo-about not found: skipped THIRD-PARTY-LICENSES.html"
fi
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>NapkinUML</string>
  <key>CFBundleDisplayName</key><string>NapkinUML</string>
  <key>CFBundleIdentifier</key><string>net.napkinuml.app</string>
  <key>CFBundleExecutable</key><string>NapkinUML</string>
  <key>CFBundleIconFile</key><string>napkin-uml.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
codesign --force --deep --sign - "$APP"

rm -f dist/NapkinUML-macos.zip dist/NapkinUML-macos.dmg
(cd dist && ditto -c -k --keepParent NapkinUML.app NapkinUML-macos.zip)
hdiutil create -volname NapkinUML -srcfolder "$APP" -ov -format UDZO dist/NapkinUML-macos.dmg
echo "Built $APP, dist/NapkinUML-macos.zip and dist/NapkinUML-macos.dmg"
