#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if ! command -v cargo >/dev/null 2>&1; then
  if command -v rustup >/dev/null 2>&1; then
    task_rust_bin=$(dirname "$(rustup which rustc)")
    PATH="$task_rust_bin:$PATH"
    export PATH
  else
    echo 'Install Rust from https://rustup.rs, then run this script again.' >&2
    exit 1
  fi
fi
cargo build --release --locked --bin amex-together
task_app='dist/Amex Together.app'
mkdir -p "$task_app/Contents/MacOS" "$task_app/Contents/Resources"
cp target/release/amex-together "$task_app/Contents/MacOS/amex-together"
cp packaging/Info.plist "$task_app/Contents/Info.plist"
task_icon_dir=${AMEX_ICON_DIR:-packaging}
cp "$task_icon_dir/AmexTogether.icns" "$task_app/Contents/Resources/AmexTogether.icns"
cp "$task_icon_dir/amex-icon.png" "$task_app/Contents/Resources/AppIcon.png"
codesign --force --deep --sign - "$task_app"
echo "Built $task_app"
