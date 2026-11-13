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
    } else if (type === "search") {
      rules.newGame();
      for (const m of ev.data.moves) {
        if (!rules.play(m)) throw new Error(`failed to replay move: ${m}`);
      }
      self.postMessage({ id, ok: true, result: rules.search(ev.data.ms) });
    }
  } catch (e) {
    self.postMessage({ id, ok: false, error: String(e?.message ?? e) });
  }
};
