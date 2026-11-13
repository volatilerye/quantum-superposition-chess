#!/usr/bin/env bash
# 公開用の静的サイトを dist/ に組み立てる。
#   1. Rust のルールコア + エンジンを wasm32-unknown-unknown でビルド
#   2. web/ と合わせて dist/ にコピー
#
# 使い方: scripts/build.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"

cd "$ROOT"
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build --release --target wasm32-unknown-unknown

rm -rf "$DIST"
mkdir -p "$DIST/pkg"
cp web/* "$DIST/"
cp target/wasm32-unknown-unknown/release/chess_core.wasm "$DIST/pkg/"

# キャッシュ対策: 古いファイルと新しいファイルが混ざらないよう、参照に版を付ける
VER="$(cat web/* "$DIST/pkg/chess_core.wasm" | sha1sum | cut -c1-10)"
sed -i "s|\"app.js\"|\"app.js?v=$VER\"|; s|\"style.css\"|\"style.css?v=$VER\"|" "$DIST/index.html"
for f in app.js engine-worker.js; do
  sed -i -E "s#\"(\./)?(rules|engine|engine-worker|i18n)\.js\"#\"\1\2.js?v=$VER\"#g; s#\"pkg/chess_core\.wasm\"#\"pkg/chess_core.wasm?v=$VER\"#" "$DIST/$f"
done

touch "$DIST/.nojekyll"
echo "built: $DIST"
ls -la "$DIST" "$DIST/pkg"
