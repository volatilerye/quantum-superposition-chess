// Rust 製ルールコア (chess_core.wasm) のラッパー。
// wasm-bindgen を使わないので、文字列は線形メモリ上のバッファ経由でやり取りする。

export async function loadRules(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url} の読み込みに失敗しました (${res.status})`);
  const { instance } = await WebAssembly.instantiate(await res.arrayBuffer(), {});
  const e = instance.exports;
  const enc = new TextEncoder();
  const dec = new TextDecoder();

  const put = (s) => {
    const bytes = enc.encode(s);
    if (bytes.length > e.input_capacity()) throw new Error("input too long");
    new Uint8Array(e.memory.buffer, e.input_ptr(), bytes.length).set(bytes);
    return bytes.length;
  };
  // 呼び出し後にポインタを取る (出力バッファは再確保されうる)
  const get = (len) => (len ? dec.decode(new Uint8Array(e.memory.buffer, e.output_ptr(), len)) : "");
  const list = (s) => (s ? s.split(" ") : []);

  const STATUS = ["ongoing", "checkmate", "stalemate", "fifty", "threefold", "insufficient"];

  return {
    newGame: () => e.new_game(),
    loadFen: (fen) => e.load_fen(put(fen)) === 1,
    /** UCI 手を指す。成功なら SAN、不正なら null */
    play: (uci) => get(e.play(put(uci))) || null,
    undo: () => e.undo() === 1,
    legalMoves: () => list(get(e.legal_moves())),
    fen: () => get(e.fen()),
    initialFen: () => get(e.initial_fen()),
    isStartPosition: () => e.is_start_position() === 1,
    history: () => list(get(e.move_history())),
    sanHistory: () => list(get(e.san_history())),
    /** 64 文字 (a1..h8)。大文字 = 白、小文字 = 黒、'.' = 空き */
    board: () => get(e.board()),
    turn: () => (e.side_to_move() ? "b" : "w"),
    inCheck: () => e.in_check() === 1,
    status: () => STATUS[e.status()] ?? "ongoing",
  };
}
