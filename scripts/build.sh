#!/usr/bin/env bash
# 公開用の静的サイトを dist/ に組み立てる。
#   1. Rust のルールコアを wasm32-unknown-unknown でビルド
#   2. npm の stockfish パッケージからシングルスレッド lite 版を取り出す
#   3. web/ と合わせて dist/ にコピー
#
# 使い方: scripts/build.sh            (STOCKFISH_VERSION で版を変更可)
set -euo pipefail

STOCKFISH_VERSION="${STOCKFISH_VERSION:-19.0.1}"
FLAVOR="${STOCKFISH_FLAVOR:-lite-single}"   # GitHub Pages ではシングルスレッド版のみ動く
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"

cd "$ROOT"
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build --release --target wasm32-unknown-unknown

rm -rf "$DIST"
mkdir -p "$DIST/pkg" "$DIST/engine"
cp web/* "$DIST/"
cp target/wasm32-unknown-unknown/release/chess_core.wasm "$DIST/pkg/"

# キャッシュ対策: 古い app.js と新しい index.html が混ざらないよう、参照に版を付ける
VER="$(cat web/* "$DIST/pkg/chess_core.wasm" | sha1sum | cut -c1-10)"
sed -i "s|\"app.js\"|\"app.js?v=$VER\"|; s|\"style.css\"|\"style.css?v=$VER\"|" "$DIST/index.html"
sed -i "s|\"\./rules\.js\"|\"./rules.js?v=$VER\"|; s|\"\./engine\.js\"|\"./engine.js?v=$VER\"|; s|\"pkg/chess_core\.wasm\"|\"pkg/chess_core.wasm?v=$VER\"|" "$DIST/app.js"

# Stockfish.js を npm から取得 (展開は専用の一時ディレクトリで行う)
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
( cd "$TMP" && npm pack "stockfish@${STOCKFISH_VERSION}" --silent >/dev/null && tar xzf stockfish-*.tgz )

JS="$(find "$TMP/package" -type f -name "stockfish-*-${FLAVOR}.js" | head -n1)"
if [[ -z "$JS" ]]; then
  echo "stockfish-*-${FLAVOR}.js が見つかりません。パッケージ内容:" >&2
  find "$TMP/package" -name 'stockfish-*' >&2
  exit 1
fi
BASE="$(basename "$JS" .js)"
# .js と対応する .wasm (分割版なら -part-N.wasm も) をコピー
cp "$(dirname "$JS")/${BASE}".* "$DIST/engine/"
find "$(dirname "$JS")" -maxdepth 1 -name "${BASE}-part-*.wasm" -exec cp {} "$DIST/engine/" \;
printf '{ "file": "%s.js", "version": "%s" }\n' "$BASE" "$STOCKFISH_VERSION" > "$DIST/engine/engine.json"

# GPL のライセンス文を同梱
for f in Copying.txt COPYING license.txt LICENSE; do
  if [[ -f "$TMP/package/$f" ]]; then cp "$TMP/package/$f" "$DIST/engine/STOCKFISH-LICENSE.txt"; break; fi
done

touch "$DIST/.nojekyll"
echo "built: $DIST"
ls -la "$DIST" "$DIST/engine" "$DIST/pkg"
