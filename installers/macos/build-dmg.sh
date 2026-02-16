#!/bin/bash
# Shard macOS DMG Builder
# Creates a signed .dmg with launchd auto-start support

set -e

VERSION="0.4.0"
APP_NAME="Shard"
DMG_NAME="Shard-${VERSION}-macOS.dmg"
VOLUME_NAME="Shard"
SOURCE_DIR="target/release/shard-daemon"
BUNDLE_ID="network.shard.daemon"

echo "Building Shard macOS DMG..."

# Create staging directory
STAGING=$(mktemp -d)
APP_DIR="${STAGING}/${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

mkdir -p "${MACOS_DIR}"
mkdir -p "${RESOURCES_DIR}"

# Copy executable
cp "${SOURCE_DIR}" "${MACOS_DIR}/"

# Create Info.plist
cat > "${CONTENTS_DIR}/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>shard-daemon</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>Shard</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleMinimumSystemVersion</key>
    <string>10.15</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>LSUIElement</key>
    <false/>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2024 Shard Network. All rights reserved.</string>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.utilities</string>
</dict>
</plist>
EOF

# Create launchd plist for auto-start
mkdir -p "${HOME}/Library/LaunchAgents"
cat > "${HOME}/Library/LaunchAgents/network.shard.daemon.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${BUNDLE_ID}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${HOME}/Applications/Shard.app/Contents/MacOS/shard-daemon</string>
        <string>--contribute</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>${HOME}/Library/Logs/shard-daemon.log</string>
    <key>StandardErrorPath</key>
    <string>${HOME}/Library/Logs/shard-daemon.error.log</string>
</dict>
</plist>
EOF

# Copy app to Applications (requires sudo for system-wide, user for local)
# cp -R "${APP_DIR}" "${HOME}/Applications/"

# Create DMG
hdiutil create -volname "${VOLUME_NAME}" -srcfolder "${STAGING}" -ov -format UDZO "${DMG_NAME}"

# Sign DMG (if certificate available)
if [ -n "$MACOS_CERTIFICATE" ]; then
    codesign -s "$MACOS_CERTIFICATE" "${DMG_NAME}"
fi

echo "Built: ${DMG_NAME}"
