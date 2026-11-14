#!/usr/bin/env bash
# itch.io の HTML ゲーム用 zip を作る (index.html が zip の直下に来る形)。
set -euo pipefail
cd "$(dirname "$0")/.."

bash scripts/build.sh
mkdir -p itch
OUT="itch/quantum-superposition-chess.zip"
rm -f "$OUT"
# itch.io はアップロードごとに別の URL で配信するので、キャッシュ対策の ?v= は外す
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -r dist/. "$STAGE"
rm -f "$STAGE/.nojekyll"
sed -i -E 's#\?v=[0-9a-f]+##g' "$STAGE"/*.html "$STAGE"/*.js
(cd "$STAGE" && zip -qr -X - .) > "$OUT"
echo "wrote $OUT"
unzip -l "$OUT"
