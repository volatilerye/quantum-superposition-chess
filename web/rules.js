// Rust 製ルールコア + エンジン (chess_core.wasm) のラッパー。
// wasm-bindgen を使わないので、文字列は線形メモリ上のバッファ経由でやり取りする。

export async function loadRules(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`failed to load ${url} (${res.status})`);
  const imports = { env: { now_ms: () => performance.now() } };
  const { instance } = await WebAssembly.instantiate(await res.arrayBuffer(), imports);
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

  const STATUS = ["ongoing", "checkmate", "stalemate", "fifty", "threefold", "insufficient", "kingCaptured", "kingRevealed", "annihilation"];

  return {
    newGame: () => e.new_game(),
    /** 手 (e2e4 / 成り e7e8q / キャスリング e1g1c / アンパッサン e5d6e) を指す。成功なら表記、不正なら null */
    play: (code) => get(e.play(put(code))) || null,
    undo: () => e.undo() === 1,
    legalMoves: () => list(get(e.legal_moves())),
    history: () => list(get(e.move_history())),
    notations: () => list(get(e.notation_history())).map((s) => s.replaceAll("_", " ")),
    /** 64 要素 (a1..h8)。"." = 空き、それ以外は色 (w/b) + ありうる種類 (pnbrqk) */
    board: () => get(e.board()).split(" "),
    captured: () => list(get(e.captured())),
    /**
     * 直前の手を「重ね合わせを更新する前」の状態で見せるための情報 (初手前は null)。
     * board / captured は board() / captured() と同じ形式で、種類だけ指す前のもの。
     * changed は種類が減った盤上の駒のマス、changedCaptured は種類が減った取られた駒の captured() での位置。
     */
    preview: () => {
      const s = get(e.preview());
      if (!s) return null;
      const [board, captured, changed, changedCaptured] = s.split("\n");
      return { board: board.split(" "), captured: list(captured), changed: list(changed).map(Number), changedCaptured: list(changedCaptured).map(Number) };
    },
    /** 手番側の駒を from から to (0..64) へ動かせない理由。{ kind: "own" | "reach" | "castle" | "check", sq? } か null (指せる) */
    whyNot: (from, to) => {
      const s = get(e.why_not(from, to));
      if (!s) return null;
      const [kind, sq] = s.split(" ");
      return { kind, sq };
    },
    turn: () => (e.side_to_move() ? "b" : "w"),
    inCheck: () => e.in_check() === 1,
    /** "checkmate" と "kingCaptured" は手番側の負け、"kingRevealed" は直前に指した側の負け */
    status: () => STATUS[e.status()] ?? "ongoing",
    /** 現局面を ms ミリ秒まで、または深さ depth 手まで探索。{ move, score (手番側から見た centipawn), depth } か null */
    search: (ms, depth = 64) => {
      const s = get(e.search(ms, depth));
      if (!s) return null;
      const [move, score, reached] = s.split(" ");
      return { move, score: Number(score), depth: Number(reached) };
    },
  };
}
