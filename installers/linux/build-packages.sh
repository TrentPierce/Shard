#!/bin/bash
# Shard Linux Package Builder
# Creates AppImage, .deb, and .rpm packages

set -e

VERSION="0.4.0"
APP_NAME="shard-daemon"
INSTALL_DIR="/opt/shard"

echo "Building Shard Linux packages..."

# Create staging directory
STAGING=$(mktemp -d)
mkdir -p "${STAGING}${INSTALL_DIR}"
mkdir -p "${STAGING}/usr/bin"
mkdir -p "${STAGING}/usr/share/doc/shard"
mkdir -p "${STAGING}/usr/share/shard"
mkdir -p "${STAGING}/etc/shard"

# Copy executable
cp "target/release/shard-daemon" "${STAGING}${INSTALL_DIR}/"
chmod +x "${STAGING}${INSTALL_DIR}/shard-daemon"

# Copy docs
cp LICENSE "${STAGING}/usr/share/doc/shard/"
cp README.md "${STAGING}/usr/share/doc/shard/"

# Create systemd service
cat > "${STAGING}/etc/shard/shard.service" <<EOF
[Unit]
Description=Shard Daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=shard
Group=shard
ExecStart=${INSTALL_DIR}/shard-daemon --contribute --public-api
Restart=always
RestartSec=10

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=${INSTALL_DIR} /var/log/shard

[Install]
WantedBy=multi-user.target
EOF

# Create init script for SysV
cat > "${STAGING}/etc/init.d/shard" <<EOF
#!/bin/bash
### BEGIN INIT INFO
# Provides:          shard
# Required-Start:    $network $remote_fs $syslog
# Required-Stop:     $network $remote_fs $syslog
# Default-Start:     2 3 4 5
# Default-Stop:      0 1 6
# Short-Description: Shard Daemon
### END INIT INFO

NAME=shard-daemon
DESC="Shard Daemon"
DAEMON=${INSTALL_DIR}/\$NAME
PIDFILE=/var/run/shard.pid

case "\$1" in
  start)
        echo "Starting \$DESC: "
        start-stop-daemon -S -x \$DAEMON -- --contribute
        ;;
  stop)
        echo "Stopping \$DESC: "
        start-stop-daemon -K -x \$DAEMON
        ;;
  restart)
        \$0 stop
        sleep 2
        \$0 start
        ;;
  *)
        echo "Usage: \$0 {start|stop|restart}"
        exit 1
        ;;
esac
exit 0
EOF

chmod +x "${STAGING}/etc/init.d/shard"

# Build AppImage
echo "Building AppImage..."
mkdir -p "${STAGING}/appimage"
cd "${STAGING}/appimage"
cp -r "${STAGING}/opt" .
cp -r "${STAGING}/usr" .
ARCH=x86_64 ./appimagetool.sh "${STAGING}/appimage" "shard-${VERSION}-linux-x86_64.AppImage" 2>/dev/null || echo "AppImage build requires appimagetool"

# Build .deb
echo "Building .deb..."
cd "${STAGING}"
fakeroot dpkg-deb --build . "shard-${VERSION}_amd64.deb" 2>/dev/null || echo "dpkg-deb not available, skipping .deb"

# Build .rpm
echo "Building .rpm..."
mkdir -p "${STAGING}/rpmbuild"
cp -r "${STAGING}"/* "${STAGING}/rpmbuild/SOURCES/" 2>/dev/null || true
rpmbuild -bb "shard.spec" 2>/dev/null || echo "rpmbuild not available, skipping .rpm"

echo "Package build complete!"
ls -la "${STAGING}"/*.deb "${STAGING}"/*.rpm "${STAGING}"/*.AppImage 2>/dev/null || true
