// 自作エンジン (Rust → WASM) を Web Worker で動かすラッパー。
// WASM の探索は同期処理で途中停止できないので、取り消しは Worker の作り直しで行う。

export class Engine {
  constructor(workerUrl, wasmUrl) {
    this.workerUrl = workerUrl;
    this.wasmUrl = new URL(wasmUrl, location.href).href;
    this.seq = 0;
    this.pending = new Map();
    this.#spawn();
  }

  #spawn() {
    this.worker = new Worker(this.workerUrl, { type: "module" });
    this.worker.onmessage = (ev) => {
      const { id, ok, result, error, progress } = ev.data;
      const p = this.pending.get(id);
      if (!p) return;
      if (progress) return p.onProgress?.(progress); // 解析の途中経過
      this.pending.delete(id);
      if (ok) p.resolve(result);
      else p.reject(new Error(error));
    };
    this.worker.onerror = (e) => this.#rejectAll(new Error(e.message || "worker error"));
    this.ready = this.#call({ type: "init", wasm: this.wasmUrl });
    this.ready.catch(() => {});
  }

  #call(msg, onProgress) {
    const id = ++this.seq;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, onProgress });
      this.worker.postMessage({ ...msg, id });
    });
  }

  #rejectAll(err) {
    for (const p of this.pending.values()) p.reject(err);
    this.pending.clear();
  }

  init() {
    return this.ready;
  }

  /** 棋譜 (手のコードの配列) の局面を ms ミリ秒まで、または深さ depth 手まで探索する */
  async search(moves, ms, depth = 64) {
    await this.ready;
    return this.#call({ type: "search", moves, ms, depth });
  }

  /** 局面を cancel() されるまで 1 手ずつ深く読み続け、深さが進むたびに onProgress({ move, score, depth }) を呼ぶ */
  async analyze(moves, onProgress) {
    await this.ready;
    return this.#call({ type: "analyze", moves }, onProgress);
  }

  /** 探索中なら打ち切る */
  cancel() {
    if (this.pending.size === 0) return;
    this.worker.terminate();
    this.#rejectAll(new Error("cancelled"));
    this.#spawn();
  }
}
