// 探索を UI スレッドから切り離すための Worker。chess_core.wasm を別インスタンスで読み込み、
// 受け取った棋譜を再生してから探索する。
import { loadRules } from "./rules.js";

let rules = null;

self.onmessage = async (ev) => {
  const { id, type } = ev.data;
  try {
    if (type === "init") {
      rules = await loadRules(ev.data.wasm);
      self.postMessage({ id, ok: true });
    } else if (type === "search" || type === "analyze") {
      rules.newGame();
      for (const m of ev.data.moves) {
        if (!rules.play(m)) throw new Error(`failed to replay move: ${m}`);
      }
      if (type === "search") {
        self.postMessage({ id, ok: true, result: rules.search(ev.data.ms, ev.data.depth) });
        return;
      }
      // 時間・深さの制限なしで 1 手ずつ深くする (浅い深さの結果は置換表に残るので読み直しは速い)。
      // 途中で止めるときは Worker ごと破棄する。詰みが見えるかエンジンの上限に達すると、
      // 指定より浅い深さで返るので、そこで終わる
      for (let depth = 1; ; depth++) {
        const r = rules.search(Infinity, depth);
        if (!r || r.depth < depth) break;
        self.postMessage({ id, progress: r });
      }
      self.postMessage({ id, ok: true });
    }
  } catch (e) {
    self.postMessage({ id, ok: false, error: String(e?.message ?? e) });
  }
};
