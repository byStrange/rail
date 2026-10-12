#!/usr/bin/env bash
# Build a self-contained rail AppImage (bundles GTK4 + WebKitGTK, not glibc/GPU drivers).
# Usage: packaging/build-appimage.sh            -> dist/rail-x86_64.AppImage
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="${RAIL_WORK:-$ROOT/target/appimage}"
APPDIR="$WORK/rail.AppDir"
TOOLS="$WORK/tools"
LIBDIR="$APPDIR/usr/lib"
WK_SRC=/usr/lib/webkitgtk-6.0
WK_DST="$APPDIR/usr/libexec/webkitgtk-6.0"
ARCH=x86_64

# Libraries that must come from the host (libc family, GPU stack, host-config-driven libs).
EXCLUDE='^(ld-linux.*|libc|libm|libdl|libpthread|librt|libutil|libresolv|libnss_.*|libanl|libBrokenLocale|libthread_db|libGL|libGLX|libGLdispatch|libEGL|libGLESv1_CM|libGLESv2|libOpenGL|libgbm|libdrm|libvulkan|libudev|libsystemd|libnvidia.*|libcuda|libasound|libpulse.*|libfontconfig|libfreetype|libharfbuzz)\.so'

echo "==> release build"
(cd "$ROOT" && cargo build --release)

echo "==> fresh AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$LIBDIR" "$WK_DST" "$APPDIR/usr/share" "$TOOLS"
install -m755 "$ROOT/target/release/rail" "$APPDIR/usr/bin/rail"

echo "==> WebKit helper processes"
for h in WebKitWebProcess WebKitNetworkProcess WebKitGPUProcess; do
  install -m755 "$WK_SRC/$h" "$WK_DST/$h"
done
cp -a "$WK_SRC/injected-bundle" "$WK_DST/injected-bundle"

echo "==> GIO modules (glib-networking provides TLS; without it https fails)"
mkdir -p "$LIBDIR/gio/modules"
cp -a /usr/lib/gio/modules/*.so "$LIBDIR/gio/modules/" 2>/dev/null || true

echo "==> collect shared libraries"
mapfile -t ROOTS < <(
  printf '%s\n' "$APPDIR/usr/bin/rail" "$WK_DST"/WebKit*Process "$LIBDIR"/gio/modules/*.so \
    "$WK_DST"/injected-bundle/*.so
)
declare -A SEEN
for f in "${ROOTS[@]}"; do
  while read -r _ _ path _; do
    [[ -n "${path:-}" && -f "$path" ]] || continue
    base="$(basename "$path")"
    [[ "$base" =~ $EXCLUDE ]] && continue
    [[ -n "${SEEN[$base]:-}" ]] && continue
    SEEN[$base]=1
    cp -L "$path" "$LIBDIR/$base"
  done < <(ldd "$f" 2>/dev/null | grep '=> /')
done
echo "    bundled ${#SEEN[@]} libraries"

echo "==> rpaths (so helper processes find bundled libs even inside WebKit's sandbox)"
patchelf --set-rpath '$ORIGIN/../lib' "$APPDIR/usr/bin/rail"
for h in "$WK_DST"/WebKit*Process; do patchelf --set-rpath '$ORIGIN/../../lib' "$h"; done
for so in "$WK_DST"/injected-bundle/*.so; do patchelf --set-rpath '$ORIGIN/../../../lib' "$so"; done
for so in "$LIBDIR"/*.so*; do patchelf --set-rpath '$ORIGIN' "$so" 2>/dev/null || true; done
for so in "$LIBDIR"/gio/modules/*.so; do patchelf --set-rpath '$ORIGIN/..' "$so" 2>/dev/null || true; done

echo "==> GSettings schemas + icons"
mkdir -p "$APPDIR/usr/share/glib-2.0/schemas" "$APPDIR/usr/share/icons"
cp /usr/share/glib-2.0/schemas/gschemas.compiled "$APPDIR/usr/share/glib-2.0/schemas/"
cp -a /usr/share/icons/Adwaita "$APPDIR/usr/share/icons/"
cp -a /usr/share/icons/hicolor "$APPDIR/usr/share/icons/" 2>/dev/null || true

echo "==> desktop entry, icon, AppRun"
cat > "$APPDIR/dev.rail.browser.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Rail
GenericName=Web Browser
Comment=A browser that scrolls sideways
Exec=rail %u
Icon=rail
Terminal=false
Categories=Network;WebBrowser;
MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;
StartupWMClass=dev.rail.browser
EOF
cat > "$APPDIR/rail.svg" <<'EOF'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256">
  <rect width="256" height="256" rx="56" fill="#16161e"/>
  <rect x="34" y="70" width="86" height="116" rx="14" fill="#7aa2f7"/>
  <rect x="136" y="70" width="86" height="116" rx="14" fill="#2a2b3d"/>
  <circle cx="198" cy="206" r="9" fill="#7aa2f7"/>
</svg>
EOF
desktop-file-validate "$APPDIR/dev.rail.browser.desktop"
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export APPDIR="$HERE"
export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export WEBKIT_EXEC_PATH="$HERE/usr/libexec/webkitgtk-6.0"
export WEBKIT_INJECTED_BUNDLE_PATH="$HERE/usr/libexec/webkitgtk-6.0/injected-bundle"
export GIO_EXTRA_MODULES="$HERE/usr/lib/gio/modules"
export GSETTINGS_SCHEMA_DIR="$HERE/usr/share/glib-2.0/schemas"
export XDG_DATA_DIRS="$HERE/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
exec "$HERE/usr/bin/rail" "$@"
EOF
chmod +x "$APPDIR/AppRun"
cp "$APPDIR/rail.svg" "$APPDIR/.DirIcon"

echo "==> appimagetool"
TOOL="$TOOLS/appimagetool-$ARCH.AppImage"
if [[ ! -x "$TOOL" ]]; then
  curl -fL -o "$TOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
  chmod +x "$TOOL"
fi
mkdir -p "$ROOT/dist"
# --appimage-extract-and-run: works without FUSE installed.
ARCH=$ARCH APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" "$APPDIR" "$ROOT/dist/rail-$ARCH.AppImage"
ls -lh "$ROOT/dist/rail-$ARCH.AppImage"
