# Rust × Stockfish チェス (GitHub Pages)

ブラウザだけで Stockfish と対局できる静的サイトです。

- **ルール判定**: Rust で書いた依存ゼロのチェスコア (`src/chess.rs`) を `wasm32-unknown-unknown` にビルド
  - 合法手生成 / キャスリング / アンパッサン / 成り / SAN 表記
  - チェックメイト・ステイルメイト・50 手・千日手 (3 回)・駒不足
  - perft で検証済み (初期局面 d4, Kiwipete d3, CPW position 3–5)
- **思考エンジン**: [Stockfish.js](https://github.com/nmrugg/stockfish.js) のシングルスレッド lite 版を Web Worker で実行
- **強さ調整**
  - Elo 1320–3190 を指定 (`UCI_LimitStrength` + `UCI_Elo`)
  - 思考時間は 1〜5 秒から選択

## 公開手順

1. このディレクトリを GitHub リポジトリに push (`main` ブランチ)
2. リポジトリの **Settings → Pages → Build and deployment → Source** を **GitHub Actions** に変更
3. `main` に push するたびに `.github/workflows/pages.yml` が
   テスト → WASM ビルド → Stockfish 取得 → Pages へデプロイ を行います

## ローカルで動かす

```sh
scripts/build.sh                # dist/ に組み立て (Rust, Node/npm が必要)
python3 -m http.server -d dist 8000
# → http://localhost:8000
```

`file://` では Worker と fetch が動かないので、必ず HTTP サーバー経由で開いてください。

## 構成

```
src/chess.rs     ルール本体 + perft テスト
src/lib.rs       WASM 向け FFI (extern "C" + 入出力バッファ、wasm-bindgen 不要)
web/rules.js     WASM のラッパー
web/engine.js    Stockfish Worker の UCI ラッパー (探索の直列化・停止)
web/app.js       盤面 UI (クリック / ドラッグ、成り選択、待った、PGN コピー)
scripts/build.sh dist/ の組み立て
```

## メモ

- GitHub Pages は COOP/COEP ヘッダを付けられず `SharedArrayBuffer` が使えないため、
  マルチスレッド版の Stockfish は動きません。`STOCKFISH_FLAVOR=single` にすると
  大きいネットワークのシングルスレッド版 (約 100MB、GitHub Pages のファイル上限に注意) になります。
- Stockfish のバージョンは `STOCKFISH_VERSION` (既定 19.0.1) で変えられます。
  `UCI_Elo` は長めの持ち時間で較正された値なので、短い思考時間では表示より弱めになります。

## ライセンス

Stockfish は GPLv3 です。これを同梱して配布するので、このプロジェクトも
GPL-3.0-or-later としています (`Cargo.toml`)。リポジトリには GPLv3 の全文を `LICENSE` として置いてください。
ビルド時に Stockfish.js のライセンス文を `engine/STOCKFISH-LICENSE.txt` として同梱します。
