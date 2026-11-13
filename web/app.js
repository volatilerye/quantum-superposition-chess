import { loadRules } from "./rules.js";
import { Engine } from "./engine.js";

// ---- 強さ ---------------------------------------------------------------
const MIN_ENGINE_DELAY = 350; // 即指しだと手が見えにくいので少し待つ (ms)

const GLYPH = { k: "♚", q: "♛", r: "♜", b: "♝", n: "♞", p: "♟" };
const glyph = (ch) => GLYPH[ch.toLowerCase()] + "︎";
const sqName = (i) => "abcdefgh"[i % 8] + (Math.floor(i / 8) + 1);
const sqIndex = (name) => (name.charCodeAt(1) - 49) * 8 + (name.charCodeAt(0) - 97);

const $ = (id) => document.getElementById(id);
const boardEl = $("board");
const statusEl = $("status");

const state = {
  rules: null,
  engine: null,
  player: "w",
  flipped: false,
  selected: null,
  thinking: false,
  token: 0, // 対局や待ったで増やし、古い探索結果を捨てる
  evalText: "",
};

// ---- 設定の保存 ---------------------------------------------------------
function loadSettings() {
  try {
    const s = JSON.parse(localStorage.getItem("chess-settings") || "{}");
    if (s.color) $("color").value = s.color;
    if (s.elo) $("elo").value = s.elo;
    if (s.movetime && $("movetime").querySelector(`option[value="${s.movetime}"]`)) $("movetime").value = s.movetime;
  } catch {}
}
function saveSettings() {
  try {
    localStorage.setItem(
      "chess-settings",
      JSON.stringify({
        color: $("color").value,
        elo: $("elo").value,
        movetime: $("movetime").value,
      }),
    );
  } catch {}
}
function strength() {
  return { elo: Number($("elo").value), go: `movetime ${$("movetime").value}` };
}

function updateStrengthUI() {
  $("elo-val").textContent = $("elo").value;
  saveSettings();
}

// ---- 描画 ---------------------------------------------------------------
function displayToSq(row, col) {
  return state.flipped ? row * 8 + (7 - col) : (7 - row) * 8 + col;
}

function legalTargets(from) {
  const name = sqName(from);
  return state.rules
    .legalMoves()
    .filter((m) => m.startsWith(name))
    .map((m) => sqIndex(m.slice(2, 4)));
}

function render() {
  const { rules } = state;
  const board = rules.board();
  const hist = rules.history();
  const last = hist.length ? hist[hist.length - 1] : null;
  const lastSqs = last ? [sqIndex(last.slice(0, 2)), sqIndex(last.slice(2, 4))] : [];
  const targets = state.selected != null ? legalTargets(state.selected) : [];
  const turn = rules.turn();
  const checkSq = rules.inCheck() ? board.indexOf(turn === "w" ? "K" : "k") : -1;

  boardEl.replaceChildren();
  for (let row = 0; row < 8; row++) {
    for (let col = 0; col < 8; col++) {
      const sq = displayToSq(row, col);
      const el = document.createElement("div");
      const light = (sq % 8 + Math.floor(sq / 8)) % 2 === 1;
      el.className = `sq ${light ? "light" : "dark"}`;
      el.dataset.sq = sq;
      if (lastSqs.includes(sq)) el.classList.add("last");
      if (sq === state.selected) el.classList.add("sel");
      if (sq === checkSq) el.classList.add("check");
      if (targets.includes(sq)) el.classList.add("target");

      const p = board[sq];
      if (p !== ".") {
        el.classList.add("occupied");
        const span = document.createElement("span");
        span.className = `piece ${p === p.toUpperCase() ? "w" : "b"}`;
        span.textContent = glyph(p);
        el.append(span);
      }
      if (col === 0) el.append(coord("rank", sqName(sq)[1]));
      if (row === 7) el.append(coord("file", sqName(sq)[0]));
      boardEl.append(el);
    }
  }
  renderMoves();
  renderStatus();
}

function coord(cls, text) {
  const s = document.createElement("span");
  s.className = `coord ${cls}`;
  s.textContent = text;
  return s;
}

function renderMoves() {
  const sans = state.rules.sanHistory();
  const startBlack = state.rules.initialFen().split(" ")[1] === "b";
  const list = startBlack ? ["…", ...sans] : sans;
  const ol = $("moves");
  ol.replaceChildren();
  for (let i = 0; i < list.length; i += 2) {
    const li = document.createElement("li");
    for (const m of [list[i], list[i + 1]]) {
      const span = document.createElement("span");
      span.textContent = m ?? "";
      li.append(span);
    }
    ol.append(li);
  }
  ol.scrollTop = ol.scrollHeight;
  $("eval").textContent = state.evalText;
}

const RESULT_TEXT = {
  stalemate: "ステイルメイトで引き分け",
  fifty: "50 手ルールで引き分け",
  threefold: "同一局面 3 回で引き分け",
  insufficient: "駒不足で引き分け",
};

function renderStatus() {
  const { rules } = state;
  const st = rules.status();
  const turn = rules.turn();
  statusEl.classList.remove("thinking");
  if (st === "checkmate") {
    statusEl.textContent = turn === state.player ? "チェックメイト — あなたの負けです" : "チェックメイト — あなたの勝ちです！";
  } else if (st !== "ongoing") {
    statusEl.textContent = RESULT_TEXT[st];
  } else if (state.thinking) {
    statusEl.textContent = "Stockfish が考えています";
    statusEl.classList.add("thinking");
  } else {
    statusEl.textContent = (turn === "w" ? "白" : "黒") + "番（あなた）" + (rules.inCheck() ? " — チェック！" : "");
  }
  $("undo").disabled = rules.history().length === 0;
}

// ---- 入力 ---------------------------------------------------------------
function canMove() {
  return !state.thinking && state.rules.status() === "ongoing" && state.rules.turn() === state.player;
}

function ownPiece(sq) {
  const p = state.rules.board()[sq];
  if (p === ".") return false;
  return (p === p.toUpperCase() ? "w" : "b") === state.player;
}

async function tryMove(from, to) {
  const prefix = sqName(from) + sqName(to);
  const cands = state.rules.legalMoves().filter((m) => m.startsWith(prefix));
  if (cands.length === 0) return false;
  let uci = cands[0];
  if (cands.length > 1) {
    const promo = await askPromotion();
    if (!promo) return false;
    uci = prefix + promo;
  }
  state.selected = null;
  if (!state.rules.play(uci)) return false;
  render();
  maybeEngineMove();
  return true;
}

function askPromotion() {
  return new Promise((resolve) => {
    const dlg = $("promo");
    const box = $("promo-choices");
    box.replaceChildren();
    for (const k of ["q", "r", "b", "n"]) {
      const b = document.createElement("button");
      const span = document.createElement("span");
      span.className = `piece ${state.player}`;
      span.textContent = glyph(k);
      b.append(span);
      b.onclick = () => {
        dlg.close();
        resolve(k);
      };
      box.append(b);
    }
    dlg.onclose = () => resolve(null);
    dlg.showModal();
  });
}

// クリックとドラッグの両方に対応
let drag = null;

boardEl.addEventListener("pointerdown", (ev) => {
  const el = ev.target.closest(".sq");
  if (!el || !canMove()) return;
  const sq = Number(el.dataset.sq);

  if (state.selected != null && state.selected !== sq && !ownPiece(sq)) {
    tryMove(state.selected, sq).then((ok) => {
      if (!ok) {
        state.selected = null;
        render();
      }
    });
    return;
  }
  if (!ownPiece(sq)) return;

  state.selected = sq;
  render();
  const piece = boardEl.querySelector(`[data-sq="${sq}"] .piece`);
  const ghost = document.createElement("div");
  ghost.className = `ghost ${piece.className}`;
  ghost.textContent = piece.textContent;
  ghost.style.left = ev.clientX + "px";
  ghost.style.top = ev.clientY + "px";
  document.body.append(ghost);
  piece.classList.add("dragging");
  drag = { from: sq, ghost, piece, moved: false, x: ev.clientX, y: ev.clientY };
  boardEl.setPointerCapture(ev.pointerId);
});

boardEl.addEventListener("pointermove", (ev) => {
  if (!drag) return;
  if (Math.hypot(ev.clientX - drag.x, ev.clientY - drag.y) > 4) drag.moved = true;
  drag.ghost.style.left = ev.clientX + "px";
  drag.ghost.style.top = ev.clientY + "px";
});

function endDrag(ev) {
  if (!drag) return;
  const d = drag;
  drag = null;
  d.ghost.remove();
  d.piece.classList.remove("dragging");
  const el = document.elementFromPoint(ev.clientX, ev.clientY)?.closest(".sq");
  if (d.moved && el && Number(el.dataset.sq) !== d.from) {
    tryMove(d.from, Number(el.dataset.sq)).then((ok) => {
      if (!ok) render();
    });
  }
}
boardEl.addEventListener("pointerup", endDrag);
boardEl.addEventListener("pointercancel", endDrag);

// ---- エンジン -----------------------------------------------------------
function positionCommand() {
  const moves = state.rules.history();
  const base = state.rules.isStartPosition() ? "startpos" : `fen ${state.rules.initialFen()}`;
  return moves.length ? `${base} moves ${moves.join(" ")}` : base;
}

function formatEval(info, engineColor) {
  const sign = engineColor === "w" ? 1 : -1; // 白から見た値にそろえる
  if (info.mate !== undefined) {
    const m = info.mate * sign;
    return `評価: ${m > 0 ? "白" : "黒"}のメイト ${Math.abs(m)} 手（深さ ${info.depth}）`;
  }
  if (info.cp !== undefined) {
    const v = (info.cp * sign) / 100;
    return `評価: ${v >= 0 ? "+" : ""}${v.toFixed(2)}（深さ ${info.depth}）`;
  }
  return "";
}

async function maybeEngineMove() {
  const { rules, engine } = state;
  if (!engine || rules.status() !== "ongoing" || rules.turn() === state.player) return;

  const token = ++state.token;
  const engineColor = rules.turn();
  state.thinking = true;
  renderStatus();

  const s = strength();
  await engine.configure(s);
  if (token !== state.token) return;

  const started = performance.now();
  const best = await engine.search(positionCommand(), s.go, (info) => {
    if (token === state.token) state.evalText = formatEval(info, engineColor);
  });
  const wait = MIN_ENGINE_DELAY - (performance.now() - started);
  if (wait > 0) await new Promise((r) => setTimeout(r, wait));
  if (token !== state.token) return; // 待った・新規対局で無効になった

  state.thinking = false;
  if (best) rules.play(best);
  render();
}

function cancelEngine() {
  state.token++;
  state.thinking = false;
  state.engine?.stop();
}

// ---- ボタン -------------------------------------------------------------
async function newGame() {
  cancelEngine();
  const c = $("color").value;
  state.player = c === "r" ? (Math.random() < 0.5 ? "w" : "b") : c;
  state.flipped = state.player === "b";
  state.selected = null;
  state.evalText = "";
  state.rules.newGame();
  render();
  saveSettings();
  if (state.engine) {
    await state.engine.newGame();
    maybeEngineMove();
  }
}

function undo() {
  const { rules } = state;
  if (rules.history().length === 0) return;
  cancelEngine();
  // 自分の手番に戻るまで戻す (最低 1 手)
  do {
    rules.undo();
  } while (rules.turn() !== state.player && rules.history().length > 0);
  state.selected = null;
  state.evalText = "";
  render();
  maybeEngineMove(); // 後手で初手まで戻した場合はエンジンが指し直す
}

function pgn() {
  const sans = state.rules.sanHistory();
  const st = state.rules.status();
  let result = "*";
  if (st === "checkmate") result = state.rules.turn() === "w" ? "0-1" : "1-0";
  else if (st !== "ongoing") result = "1/2-1/2";
  const d = new Date();
  const date = `${d.getFullYear()}.${String(d.getMonth() + 1).padStart(2, "0")}.${String(d.getDate()).padStart(2, "0")}`;
  const you = "You";
  const sf = `Stockfish (Elo ${$("elo").value})`;
  const headers = [
    `[Event "Casual game"]`,
    `[Date "${date}"]`,
    `[White "${state.player === "w" ? you : sf}"]`,
    `[Black "${state.player === "b" ? you : sf}"]`,
    `[Result "${result}"]`,
  ];
  if (!state.rules.isStartPosition()) headers.push(`[SetUp "1"]`, `[FEN "${state.rules.initialFen()}"]`);
  const startBlack = state.rules.initialFen().split(" ")[1] === "b";
  let n = Number(state.rules.initialFen().split(" ")[5] || 1);
  const body = [];
  sans.forEach((m, i) => {
    const white = startBlack ? i % 2 === 1 : i % 2 === 0;
    if (white) body.push(`${n}. ${m}`);
    else {
      if (i === 0) body.push(`${n}... ${m}`);
      else body.push(m);
      n++;
    }
  });
  return `${headers.join("\n")}\n\n${body.join(" ")} ${result}\n`;
}

$("new-game").onclick = newGame;
$("undo").onclick = undo;
$("flip").onclick = () => {
  state.flipped = !state.flipped;
  render();
};
$("copy-pgn").onclick = async () => {
  try {
    await navigator.clipboard.writeText(pgn());
    $("copy-pgn").textContent = "コピーしました";
    setTimeout(() => ($("copy-pgn").textContent = "PGN をコピー"), 1500);
  } catch {
    prompt("PGN", pgn());
  }
};
for (const id of ["elo", "movetime"]) $(id).addEventListener("input", updateStrengthUI);

// ---- 起動 ---------------------------------------------------------------
async function main() {
  loadSettings();
  updateStrengthUI();
  try {
    state.rules = await loadRules("pkg/chess_core.wasm");
  } catch (e) {
    statusEl.textContent = "ルールエンジン (WASM) を読み込めませんでした: " + e.message;
    return;
  }
  const c = $("color").value;
  state.player = c === "r" ? (Math.random() < 0.5 ? "w" : "b") : c;
  state.flipped = state.player === "b";
  render();
  statusEl.textContent = "Stockfish を読み込み中";

  try {
    let file = "stockfish.js";
    try {
      file = (await (await fetch("engine/engine.json")).json()).file || file;
    } catch {}
    const engine = new Engine(`engine/${file}`);
    await engine.init();
    await engine.newGame();
    state.engine = engine;
  } catch (e) {
    statusEl.textContent = "Stockfish を読み込めませんでした: " + e.message;
    return;
  }
  render();
  maybeEngineMove();
}

main();
