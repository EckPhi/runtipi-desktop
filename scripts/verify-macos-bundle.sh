#!/usr/bin/env bash
set -euo pipefail

target="${1:?Pass the macOS target triple}"
case "$target" in
  aarch64-apple-darwin) expected_arch=arm64 ;;
  x86_64-apple-darwin) expected_arch=x86_64 ;;
  *) echo "Unsupported macOS target: $target" >&2; exit 1 ;;
esac

images=("src-tauri/target/$target/release/bundle/dmg/"*.dmg)
if [[ ${#images[@]} -ne 1 || ! -f "${images[0]}" ]]; then
  echo 'Expected exactly one packaged DMG' >&2
  exit 1
fi
mount_dir="$(mktemp -d)"
cleanup() {
  hdiutil detach "$mount_dir" >/dev/null 2>&1 || true
  rmdir "$mount_dir" 2>/dev/null || true
}
trap cleanup EXIT
hdiutil verify "${images[0]}"
hdiutil attach -readonly -nobrowse -mountpoint "$mount_dir" "${images[0]}"
app="$mount_dir/Runtipi Desktop.app"
codesign --verify --deep --strict --verbose=2 "$app"
codesign --display --verbose=2 "$app"
executable="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app/Contents/Info.plist")"
archs="$(lipo -archs "$app/Contents/MacOS/$executable")"
if [[ "$archs" != "$expected_arch" ]]; then
  echo "Expected architecture $expected_arch, found $archs" >&2
  exit 1
fi
printf 'Packaged app signature verified for %s (%s)\n' "$target" "$archs"
