#!/bin/sh
set -eu
cd "$(dirname "$0")"
if [ ! -x 'dist/Amex Together.app/Contents/MacOS/amex-together' ]; then
  sh scripts/package-macos.sh
fi
open 'dist/Amex Together.app'
