import { loadRules } from "./rules.js?v=a2f37a99bc";
import { Engine } from "./engine.js?v=a2f37a99bc";

const WASM_URL = "pkg/chess_core.wasm?v=a2f37a99bc";
const WORKER_URL = "engine-worker.js?v=a2f37a99bc";
const MIN_ENGINE_DELAY = 350; // 即指しだと手が見えにくいので少し待つ (ms)

const GLYPH = { k: "♚", q: "♛", r: "♜", b: "♝", n: "♞", p: "♟" };
const KIND_NAME = { p: "ポーン", n: "ナイト", b: "ビショップ", r: "ルーク", q: "クイーン", k: "キング" };
const COLOR_NAME = { w: "白", b: "黒" };
const glyph = (k) => GLYPH[k] + "︎";

const sqName = (i) => "abcdefgh"[i % 8] + (Math.floor(i / 8) + 1);
const sqIndex = (name) => (name.charCodeAt(1) - 49) * 8 + (name.charCodeAt(0) - 97);

const $ = (id) => document.getElementById(id);
const boardEl = $("board");
const statusEl = $("status");

const state = {
  rules: null,
  engine: null,
  opponent: "engine", // "engine" | "human"
  player: "w", // エンジン対局でのこちらの色
  flipped: false,
  selected: null,
  thinking: false,
  token: 0, // 対局や待ったで増やし、古い探索結果を捨てる
  evalText: "",
  error: "",
};

// ---- 設定の保存 ---------------------------------------------------------
const SETTINGS = ["opponent", "color", "movetime"];

function loadSettings() {
  try {
    const s = JSON.parse(localStorage.getItem("qchess-settings") || "{}");
    for (const id of SETTINGS) {
      if (s[id] && $(id).querySelector(`option[value="${s[id]}"]`)) $(id).value = s[id];
    }
  } catch {}
}
function saveSettings() {
  try {
    localStorage.setItem("qchess-settings", JSON.stringify(Object.fromEntries(SETTINGS.map((id) => [id, $(id).value]))));
  } catch {}
}

function updateSettingsUI() {
  $("color-box").hidden = $("opponent").value !== "engine";
  saveSettings();
}

// ---- 盤面の解釈 -----------------------------------------------------------
/** "wpnb" → { color: "w", kinds: ["p","n","b"] } */
const parseToken = (t) => ({ color: t[0], kinds: [...t.slice(1)] });

const engineTurn = () => state.opponent === "engine" && state.rules.turn() !== state.player;

function displayToSq(row, col) {
  return state.flipped ? row * 8 + (7 - col) : (7 - row) * 8 + col;
}

function legalTargets(from) {
  const name = sqName(from);
  return [...new Set(state.rules.legalMoves().filter((m) => m.startsWith(name)).map((m) => sqIndex(m.slice(2, 4))))];
}

// ---- 描画 ---------------------------------------------------------------
/** 重ね合わせの駒。通常は半透明で重ね、ホバー (または選択) で円形に並べる */
function pieceEl(token) {
  const { color, kinds } = parseToken(token);
  const el = document.createElement("div");
  el.className = `piece ${color}` + (kinds.length > 1 ? " super" : "");
  el.style.setProperty("--n", kinds.length);
  kinds.forEach((k, i) => {
    const g = document.createElement("span");
    g.className = "g";
    g.textContent = glyph(k);
    g.style.setProperty("--i", i);
    el.append(g);
  });
  return el;
}

const describe = (token) => {
  const { color, kinds } = parseToken(token);
  return `${COLOR_NAME[color]}: ${kinds.map((k) => KIND_NAME[k]).join("・")}`;
};

function render() {
  const { rules } = state;
  const board = rules.board();
  const hist = rules.history();
  const last = hist.length ? hist[hist.length - 1] : null;
  const lastSqs = last ? [sqIndex(last.slice(0, 2)), sqIndex(last.slice(2, 4))] : [];
  const targets = state.selected != null ? legalTargets(state.selected) : [];
  const turn = rules.turn();
  const checkSq = rules.inCheck() ? board.indexOf(turn + "k") : -1;

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

      const t = board[sq];
      if (t !== ".") {
        el.classList.add("occupied");
        el.title = describe(t);
        el.append(pieceEl(t));
      }
      if (col === 0) el.append(coord("rank", sqName(sq)[1]));
      if (row === 7) el.append(coord("file", sqName(sq)[0]));
      boardEl.append(el);
    }
  }
  renderCaptured();
  renderMoves();
  renderStatus();
}

function coord(cls, text) {
  const s = document.createElement("span");
  s.className = `coord ${cls}`;
  s.textContent = text;
  return s;
}

/** 取られた駒。盤の上側に上側の色、下側に下側の色の駒を並べる */
function renderCaptured() {
  const caps = state.rules.captured();
  const bottom = state.flipped ? "b" : "w";
  for (const [id, color] of [["captured-bottom", bottom], ["captured-top", bottom === "w" ? "b" : "w"]]) {
    const box = $(id);
    box.replaceChildren();
    for (const t of caps.filter((t) => t[0] === color)) {
      const c = document.createElement("span");
      c.className = "cap";
      c.title = "取られた駒 — " + describe(t);
      c.append(pieceEl(t));
      box.append(c);
    }
  }
}

function renderMoves() {
  const notes = state.rules.notations();
  const ol = $("moves");
  ol.replaceChildren();
  for (let i = 0; i < notes.length; i += 2) {
    const li = document.createElement("li");
    for (const m of [notes[i], notes[i + 1]]) {
      const span = document.createElement("span");
      span.textContent = m ?? "";
      li.append(span);
    }
    ol.append(li);
  }
  ol.scrollTop = ol.scrollHeight;
  $("eval").textContent = state.evalText;
}

const DRAW_TEXT = {
  stalemate: "ステイルメイトで引き分け",
  fifty: "50 手ルールで引き分け",
  threefold: "同一局面 3 回で引き分け",
  insufficient: "駒不足で引き分け",
};

function resultText(st, turn) {
  // checkmate / kingCaptured は手番側の負け
  const how = st === "checkmate" ? "チェックメイト" : "キングを取りました";
  const winner = turn === "w" ? "b" : "w";
  if (state.opponent === "human") return `${how} — ${COLOR_NAME[winner]}の勝ち`;
  return `${how} — ${winner === state.player ? "あなたの勝ちです！" : "あなたの負けです"}`;
}

function renderStatus() {
  const { rules } = state;
  const st = rules.status();
  const turn = rules.turn();
  statusEl.classList.remove("thinking");
  if (state.error) {
    statusEl.textContent = state.error;
  } else if (st === "checkmate" || st === "kingCaptured") {
    statusEl.textContent = resultText(st, turn);
  } else if (st !== "ongoing") {
    statusEl.textContent = DRAW_TEXT[st];
  } else if (state.thinking) {
    statusEl.textContent = "エンジンが考えています";
    statusEl.classList.add("thinking");
  } else if (engineTurn()) {
    statusEl.textContent = "エンジンを読み込み中";
  } else {
    const who = state.opponent === "engine" ? "（あなた）" : "";
    statusEl.textContent = `${COLOR_NAME[turn]}番${who}` + (rules.inCheck() ? " — チェック！" : "");
  }
  $("undo").disabled = rules.history().length === 0;
}

// ---- 入力 ---------------------------------------------------------------
function canMove() {
  return !state.thinking && state.rules.status() === "ongoing" && !engineTurn();
}

function ownPiece(sq) {
  const t = state.rules.board()[sq];
  return t !== "." && t[0] === state.rules.turn();
}

/** 同じ移動元・移動先で複数の解釈がある手 (成り・キャスリング・アンパッサン) の表示名 */
function choiceLabel(code, all) {
  const s = code.slice(4);
  if ("qrbn".includes(s) && s) return { glyph: s, text: `${KIND_NAME[s]}に成る` };
  if (s === "c") return { text: "キャスリング" };
  if (s === "e") return { text: "アンパッサン（ポーンとして取る）" };
  const promo = all.some((c) => "qrbn".includes(c.slice(4)) && c.length === 5);
  return { text: promo ? "成らない（ポーン以外として動く）" : "通常の移動（ポーン以外として動く）" };
}

async function tryMove(from, to) {
  const prefix = sqName(from) + sqName(to);
  const cands = state.rules.legalMoves().filter((m) => m.slice(0, 4) === prefix);
  if (cands.length === 0) return false;
  let code = cands[0];
  if (cands.length > 1) {
    code = await askChoice("どの動きにしますか？", cands.map((c) => ({ value: c, ...choiceLabel(c, cands) })));
    if (!code) return false;
  }
  state.selected = null;
  if (!state.rules.play(code)) return false;
  render();
  maybeEngineMove();
  return true;
}

function askChoice(title, options) {
  return new Promise((resolve) => {
    const dlg = $("choice");
    $("choice-title").textContent = title;
    const box = $("choice-buttons");
    box.replaceChildren();
    let picked = null;
    for (const o of options) {
      const b = document.createElement("button");
      if (o.glyph) {
        const g = document.createElement("span");
        g.className = `glyph ${state.rules.turn()}`;
        g.textContent = glyph(o.glyph);
        b.append(g);
      }
      b.append(o.text);
      b.onclick = () => {
        picked = o.value;
        dlg.close();
      };
      box.append(b);
    }
    dlg.onclose = () => resolve(picked);
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
  ghost.className = "ghost";
  ghost.append(piece.cloneNode(true));
  ghost.style.left = ev.clientX + "px";
  ghost.style.top = ev.clientY + "px";
  ghost.hidden = true; // 動かし始めるまで出さない
  document.body.append(ghost);
  drag = { from: sq, ghost, piece, moved: false, x: ev.clientX, y: ev.clientY };
  boardEl.setPointerCapture(ev.pointerId);
});

boardEl.addEventListener("pointermove", (ev) => {
  if (!drag) return;
  if (!drag.moved && Math.hypot(ev.clientX - drag.x, ev.clientY - drag.y) > 4) {
    drag.moved = true;
    drag.ghost.hidden = false;
    drag.piece.classList.add("dragging");
  }
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
function formatEval(r, engineColor) {
  const sign = engineColor === "w" ? 1 : -1; // 白から見た値にそろえる
  const v = r.score * sign;
  if (Math.abs(v) > 90000) return `評価: ${v > 0 ? "白" : "黒"}の勝ちが見えています（深さ ${r.depth}）`;
  return `評価: ${v >= 0 ? "+" : ""}${(v / 100).toFixed(2)}（深さ ${r.depth}）`;
}

async function maybeEngineMove() {
  const { rules, engine } = state;
  if (!engine || rules.status() !== "ongoing" || !engineTurn()) return;

  const token = ++state.token;
  const engineColor = rules.turn();
  state.thinking = true;
  renderStatus();

  const started = performance.now();
  let r;
  try {
    r = await engine.search(rules.history(), Number($("movetime").value));
  } catch (e) {
    if (token !== state.token) return; // 取り消し
    state.thinking = false;
    state.error = "エンジンでエラーが起きました: " + e.message;
    renderStatus();
    return;
  }
  const wait = MIN_ENGINE_DELAY - (performance.now() - started);
  if (wait > 0) await new Promise((res) => setTimeout(res, wait));
  if (token !== state.token) return; // 待った・新規対局で無効になった

  state.thinking = false;
  if (r && !rules.play(r.move)) {
    state.error = `エンジンが不正な手を返しました: ${r.move}`;
  } else if (r) {
    state.evalText = formatEval(r, engineColor);
  }
  render();
}

function cancelEngine() {
  state.token++;
  if (state.thinking) state.engine?.cancel();
  state.thinking = false;
}

// ---- ボタン -------------------------------------------------------------
function newGame() {
  cancelEngine();
  state.opponent = $("opponent").value;
  const c = $("color").value;
  state.player = c === "r" ? (Math.random() < 0.5 ? "w" : "b") : c;
  state.flipped = state.opponent === "engine" && state.player === "b";
  state.selected = null;
  state.evalText = "";
  state.error = "";
  state.rules.newGame();
  render();
  saveSettings();
  maybeEngineMove();
}

function undo() {
  const { rules } = state;
  if (rules.history().length === 0) return;
  cancelEngine();
  if (state.opponent === "human") {
    rules.undo();
  } else {
    // 自分の手番に戻るまで戻す (最低 1 手)
    do {
      rules.undo();
    } while (rules.turn() !== state.player && rules.history().length > 0);
  }
  state.selected = null;
  state.evalText = "";
  state.error = "";
  render();
  maybeEngineMove(); // 後手で初手まで戻した場合はエンジンが指し直す
}

function kifu() {
  const notes = state.rules.notations();
  const st = state.rules.status();
  let result = "*";
  if (st === "checkmate" || st === "kingCaptured") result = state.rules.turn() === "w" ? "0-1" : "1-0";
  else if (st !== "ongoing") result = "1/2-1/2";
  const d = new Date();
  const date = `${d.getFullYear()}.${String(d.getMonth() + 1).padStart(2, "0")}.${String(d.getDate()).padStart(2, "0")}`;
  const name = (c) => (state.opponent === "human" ? (c === "w" ? "White" : "Black") : c === state.player ? "You" : "Engine");
  const headers = [
    `[Event "Casual game"]`,
    `[Variant "Quantum superposition chess"]`,
    `[Date "${date}"]`,
    `[White "${name("w")}"]`,
    `[Black "${name("b")}"]`,
    `[Result "${result}"]`,
  ];
  const body = [];
  notes.forEach((m, i) => body.push(i % 2 === 0 ? `${i / 2 + 1}. ${m}` : m));
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
    await navigator.clipboard.writeText(kifu());
    $("copy-pgn").textContent = "コピーしました";
    setTimeout(() => ($("copy-pgn").textContent = "棋譜をコピー"), 1500);
  } catch {
    prompt("棋譜", kifu());
  }
};
for (const id of SETTINGS) $(id).addEventListener("input", updateSettingsUI);

// ---- 起動 ---------------------------------------------------------------
async function main() {
  loadSettings();
  updateSettingsUI();
  try {
    state.rules = await loadRules(WASM_URL);
  } catch (e) {
    statusEl.textContent = "ルールエンジン (WASM) を読み込めませんでした: " + e.message;
    return;
  }
  state.opponent = $("opponent").value;
  const c = $("color").value;
  state.player = c === "r" ? (Math.random() < 0.5 ? "w" : "b") : c;
  state.flipped = state.opponent === "engine" && state.player === "b";
  render();

  const engine = new Engine(WORKER_URL, WASM_URL);
  try {
    await engine.init();
  } catch (e) {
    state.error = "エンジンを読み込めませんでした: " + e.message;
    renderStatus();
    return;
  }
  state.engine = engine;
  maybeEngineMove();
}

main();
