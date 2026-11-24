#!/usr/bin/env bash
set -euo pipefail

# Script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

# Extract version from Cargo.toml
VERSION=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "${VERSION}" ]; then
    echo "Error: Could not extract version from Cargo.toml" >&2
    exit 1
fi

echo "=========================================="
echo " Packaging MLV Explorer v${VERSION} (Linux)"
echo "=========================================="

TARGET="${1:-x86_64-unknown-linux-gnu}"

echo ""
echo "[1/5] Compiling release binary (target: ${TARGET})..."
cargo build --release --target "${TARGET}"

BINARY_SOURCE="target/${TARGET}/release/mlv-explorer"
if [ ! -f "${BINARY_SOURCE}" ]; then
    BINARY_SOURCE="target/release/mlv-explorer"
fi

if [ ! -f "${BINARY_SOURCE}" ]; then
    echo "Error: Compiled binary not found at ${BINARY_SOURCE}" >&2
    exit 1
fi

DIST_DIR="${PROJECT_ROOT}/dist"
PACKAGE_NAME="mlv-explorer-v${VERSION}-linux-x64-portable"
STAGING_DIR="${DIST_DIR}/${PACKAGE_NAME}"
TAR_FILE="${DIST_DIR}/${PACKAGE_NAME}.tar.gz"

echo ""
echo "[2/5] Staging files for portable tar.gz archive..."
rm -rf "${STAGING_DIR}"
mkdir -p "${STAGING_DIR}"
cp "${BINARY_SOURCE}" "${STAGING_DIR}/mlv-explorer"
chmod +x "${STAGING_DIR}/mlv-explorer"
cp README.md "${STAGING_DIR}/"
cp LICENSE "${STAGING_DIR}/"

echo ""
echo "[3/5] Creating portable tar.gz archive: ${TAR_FILE}..."
mkdir -p "${DIST_DIR}"
tar -czvf "${TAR_FILE}" -C "${DIST_DIR}" "${PACKAGE_NAME}"
rm -rf "${STAGING_DIR}"

# 4. AppImage Creation (if appimagetool is available)
APPIMAGE_FILE="${DIST_DIR}/mlv-explorer-v${VERSION}-x86_64.AppImage"
echo ""
echo "[4/5] Checking for appimagetool for AppImage creation..."

if command -v appimagetool >/dev/null 2>&1 || [ -f "/tmp/appimagetool" ]; then
    APPIMAGE_TOOL="appimagetool"
    if [ ! -x "$(command -v appimagetool 2>/dev/null)" ] && [ -f "/tmp/appimagetool" ]; then
        APPIMAGE_TOOL="/tmp/appimagetool"
    fi

    echo "Building AppImage using ${APPIMAGE_TOOL}..."
    APPDIR="${DIST_DIR}/AppDir"
    rm -rf "${APPDIR}"
    mkdir -p "${APPDIR}/usr/bin"
    mkdir -p "${APPDIR}/usr/share/icons/hicolor/256x256/apps"
    mkdir -p "${APPDIR}/usr/share/applications"

    cp "${BINARY_SOURCE}" "${APPDIR}/usr/bin/mlv-explorer"
    chmod +x "${APPDIR}/usr/bin/mlv-explorer"

    cp "assets/mlv-explorer.desktop" "${APPDIR}/mlv-explorer.desktop"
    cp "assets/mlv-explorer.desktop" "${APPDIR}/usr/share/applications/mlv-explorer.desktop"
    cp "assets/icons/mlv-explorer.png" "${APPDIR}/mlv-explorer.png"
    cp "assets/icons/mlv-explorer.png" "${APPDIR}/usr/share/icons/hicolor/256x256/apps/mlv-explorer.png"

    cat << 'EOF' > "${APPDIR}/AppRun"
#!/bin/sh
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
export PATH="${HERE}/usr/bin/:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib/:${LD_LIBRARY_PATH}"
exec "${HERE}/usr/bin/mlv-explorer" "$@"
EOF
    chmod +x "${APPDIR}/AppRun"

    ARCH=x86_64 "${APPIMAGE_TOOL}" "${APPDIR}" "${APPIMAGE_FILE}"
    rm -rf "${APPDIR}"
    echo "AppImage created: ${APPIMAGE_FILE}"
else
    echo "Notice: appimagetool not found on PATH. Skipping AppImage step."
fi

# 5. Debian .deb Installer Creation (if dpkg-deb is available)
DEB_FILE="${DIST_DIR}/mlv-explorer-v${VERSION}-linux-amd64.deb"
echo ""
echo "[5/5] Checking for dpkg-deb for .deb installer creation..."
if command -v dpkg-deb >/dev/null 2>&1; then
    DEB_ROOT="${DIST_DIR}/deb-pkg"
    rm -rf "${DEB_ROOT}"
    mkdir -p "${DEB_ROOT}/DEBIAN" \
             "${DEB_ROOT}/usr/bin" \
             "${DEB_ROOT}/usr/share/applications" \
             "${DEB_ROOT}/usr/share/icons/hicolor/256x256/apps"

    cp "${BINARY_SOURCE}" "${DEB_ROOT}/usr/bin/mlv-explorer"
    chmod 755 "${DEB_ROOT}/usr/bin/mlv-explorer"
    cp "assets/mlv-explorer.desktop" "${DEB_ROOT}/usr/share/applications/mlv-explorer.desktop"
    cp "assets/icons/mlv-explorer.png" "${DEB_ROOT}/usr/share/icons/hicolor/256x256/apps/mlv-explorer.png"

    printf '%s\n' \
        "Package: mlv-explorer" \
        "Version: ${VERSION}" \
        "Section: video" \
        "Priority: optional" \
        "Architecture: amd64" \
        "Maintainer: MLV Explorer Contributors <noreply@github.com>" \
        "Depends: libasound2, libgtk-3-0, libxkbcommon0" \
        "Description: Magic Lantern RAW Video Catalog, Browser, and GPU Player" \
        " Photo catalog-style browser, player, and organizer for Magic Lantern RAW (.mlv) video." > "${DEB_ROOT}/DEBIAN/control"

    dpkg-deb --build --root-owner-group "${DEB_ROOT}" "${DEB_FILE}"
    rm -rf "${DEB_ROOT}"
    echo ".deb created: ${DEB_FILE}"
else
    echo "Notice: dpkg-deb not found on PATH. Skipping .deb installer step."
fi

echo ""
echo "=========================================="
echo " Packaging complete!"
echo " Archives in dist/:"
ls -lh "${DIST_DIR}"/mlv-explorer-v${VERSION}*
echo "=========================================="
