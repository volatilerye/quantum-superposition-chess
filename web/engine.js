// Stockfish.js を Web Worker として動かし、UCI でやり取りする薄いラッパー。
// GitHub Pages では COOP/COEP ヘッダを付けられないため、シングルスレッド版を使う。

export class Engine {
  constructor(path) {
    this.worker = new Worker(path);
    this.listeners = new Set();
    this.options = {}; // UCI option name -> { min, max, default }
    this.idle = Promise.resolve();
    this.searching = false;
    this.worker.onmessage = (ev) => {
      const text = typeof ev.data === "string" ? ev.data : String(ev.data);
      for (const line of text.split("\n")) {
        if (line) for (const fn of [...this.listeners]) fn(line);
      }
    };
  }

  send(cmd) {
    this.worker.postMessage(cmd);
  }

  waitFor(pred) {
    return new Promise((resolve) => {
      const fn = (line) => {
        if (pred(line)) {
          this.listeners.delete(fn);
          resolve(line);
        }
      };
      this.listeners.add(fn);
    });
  }

  async init(timeoutMs = 30000) {
    let timer;
    const failed = new Promise((_, reject) => {
      this.worker.onerror = (e) => reject(new Error(e.message || "worker error"));
      timer = setTimeout(() => reject(new Error("timeout")), timeoutMs);
    });
    failed.catch(() => {}); // 成功後に reject しても未処理扱いにしない
    try {
      await Promise.race([this.handshake(), failed]);
    } finally {
      clearTimeout(timer);
    }
  }

  async handshake() {
    const collect = (line) => {
      const m = line.match(/^option name (.+?) type (\w+)(.*)$/);
      if (!m) return;
      const rest = m[3];
      const num = (k) => {
        const r = rest.match(new RegExp(`${k} (-?\\d+)`));
        return r ? Number(r[1]) : undefined;
      };
      this.options[m[1]] = { type: m[2], min: num("min"), max: num("max"), default: num("default") };
    };
    this.listeners.add(collect);
    this.send("uci");
    await this.waitFor((l) => l === "uciok");
    this.listeners.delete(collect);
    await this.ready();
  }

  async ready() {
    this.send("isready");
    await this.waitFor((l) => l === "readyok");
  }

  setOption(name, value) {
    if (name in this.options) this.send(`setoption name ${name} value ${value}`);
  }

  /** エンジンへの一連の操作を直列に実行する (bestmove の取り違え防止) */
  exclusive(fn) {
    const run = this.idle.then(fn);
    this.idle = run.catch(() => {});
    return run;
  }

  /** 強さの設定 (実行中の探索が終わってから反映) */
  configure({ elo }) {
    return this.exclusive(async () => {
      this.setOption("UCI_LimitStrength", "true");
      this.setOption("UCI_Elo", elo);
      await this.ready();
    });
  }

  newGame() {
    this.stop();
    return this.exclusive(async () => {
      this.send("ucinewgame");
      await this.ready();
    });
  }

  /**
   * 探索して最善手 (UCI) を返す。
   * @param {string} position  "startpos moves ..." / "fen ... moves ..."
   * @param {string} go        "depth 8" / "movetime 1000" など
   * @param {(info: {depth:number, cp?:number, mate?:number}) => void} onInfo
   */
  search(position, go, onInfo) {
    return this.exclusive(() => this.#search(position, go, onInfo));
  }

  async #search(position, go, onInfo) {
    this.searching = true;

    const info = (line) => {
      if (!onInfo || !line.startsWith("info ") || !line.includes(" score ")) return;
      if (/ multipv (?!1 )\d+/.test(line)) return;
      const depth = Number(line.match(/ depth (\d+)/)?.[1] ?? 0);
      const cp = line.match(/ score cp (-?\d+)/);
      const mate = line.match(/ score mate (-?\d+)/);
      onInfo({ depth, cp: cp ? Number(cp[1]) : undefined, mate: mate ? Number(mate[1]) : undefined });
    };
    this.listeners.add(info);
    this.send(`position ${position}`);
    this.send(`go ${go}`);
    const line = await this.waitFor((l) => l.startsWith("bestmove"));
    this.listeners.delete(info);
    this.searching = false;
    const mv = line.split(/\s+/)[1];
    return mv && mv !== "(none)" ? mv : null;
  }

  stop() {
    if (this.searching) this.send("stop");
  }
}
