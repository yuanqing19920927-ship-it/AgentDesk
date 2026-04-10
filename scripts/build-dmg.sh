#!/usr/bin/env bash
#
# Build a release .app bundle and pack it into a DMG installer.
# Intended to run from the repo root: `./scripts/build-dmg.sh`.
#
# Output:
#   target/release/bundle/AgentDesk.app
#   AgentDesk-<version>.dmg   (at repo root, overwrites any existing)
#
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="AgentDesk"
BUNDLE_ID="com.agentdesk.app"
VERSION="$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)"
BUNDLE_DIR="target/release/bundle"
APP_DIR="${BUNDLE_DIR}/${APP_NAME}.app"
CONTENTS="${APP_DIR}/Contents"
DMG_NAME="${APP_NAME}-${VERSION}.dmg"
DMG_STAGING="target/release/dmg-staging"

echo "==> Building release binary (version ${VERSION})"
cargo build --release

echo "==> Assembling ${APP_DIR}"
rm -rf "${APP_DIR}"
mkdir -p "${CONTENTS}/MacOS" "${CONTENTS}/Resources"

cp target/release/agentdesk "${CONTENTS}/MacOS/agentdesk"
# Universal island-overlay lives in helpers/ and works on both arm64
# and x86_64 so the same bundle runs on both machines.
cp helpers/island-overlay-universal "${CONTENTS}/MacOS/island-overlay"
chmod +x "${CONTENTS}/MacOS/agentdesk" "${CONTENTS}/MacOS/island-overlay"

cp assets/AppIcon.icns "${CONTENTS}/Resources/AppIcon.icns"

cat > "${CONTENTS}/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleExecutable</key>
    <string>agentdesk</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSUIElement</key>
    <false/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
</dict>
</plist>
PLIST

# Ad-hoc sign so the binary launches without the "damaged app" prompt
# on the developer's own machine. (Not notarised — users on other
# machines still need to right-click → Open the first time.)
echo "==> Ad-hoc codesigning"
codesign --force --deep --sign - "${APP_DIR}" >/dev/null

echo "==> Staging DMG contents"
rm -rf "${DMG_STAGING}"
mkdir -p "${DMG_STAGING}"
cp -R "${APP_DIR}" "${DMG_STAGING}/"
ln -s /Applications "${DMG_STAGING}/Applications"

echo "==> Creating ${DMG_NAME}"
rm -f "${DMG_NAME}"
hdiutil create \
    -volname "${APP_NAME}-${VERSION}" \
    -srcfolder "${DMG_STAGING}" \
    -ov \
    -format UDZO \
    "${DMG_NAME}" >/dev/null

# Compute a SHA-256 so downstream (release notes, Homebrew, etc.) can
# reference a stable hash.
SHA256="$(shasum -a 256 "${DMG_NAME}" | awk '{print $1}')"
SIZE_MB="$(du -m "${DMG_NAME}" | awk '{print $1}')"

rm -rf "${DMG_STAGING}"

echo ""
echo "✅ Built ${DMG_NAME} (${SIZE_MB} MB)"
echo "   SHA-256: ${SHA256}"
