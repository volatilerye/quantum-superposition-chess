import { loadRules } from "./rules.js";
import { Engine } from "./engine.js";
import { Link, inviteIdOf } from "./online.js";
import { t, setLang, applyStatic, DEFAULT_LANG } from "./i18n.js";

const WASM_URL = "pkg/chess_core.wasm";
const WORKER_URL = "engine-worker.js";
const MIN_ENGINE_DELAY = 350; // 即指しだと手が見えにくいので少し待つ (ms)
const STAGE_MS = 450; // 手を見せる各段階 (移動 → 重ね合わせの更新 → 手番・判定) の間隔 (ms)
const sleep = (ms) => new Promise((res) => setTimeout(res, ms));

const GLYPH = { k: "♚", q: "♛", r: "♜", b: "♝", n: "♞", p: "♟" };
const kindName = (k) => t("kind_" + k);
const colorName = (c) => t(c === "w" ? "white" : "black");
const glyph = (k) => GLYPH[k] + "︎";

const sqName = (i) => "abcdefgh"[i % 8] + (Math.floor(i / 8) + 1);
const sqIndex = (name) => (name.charCodeAt(1) - 49) * 8 + (name.charCodeAt(0) - 97);

const $ = (id) => document.getElementById(id);
const boardEl = $("board");
const statusEl = $("status");

const state = {
  rules: null,
  engine: null,
  opponent: "engine", // "engine" | "human" | "online"
  link: null, // オンライン対戦の接続 (online.js の Link)
  role: null, // オンライン対戦で "host" (招待した側) | "guest" (招待された側)
  netUnstable: false, // オンライン対戦の接続が一時的に不安定 (相手のブラウザが裏に回ったときなど)
  inviteUrl: "", // 作った招待リンク
  replyUrl: "", // 作った返信リンク
  setup: 0, // 初期配置 (ビット 0 = 白を通常のチェスの配置に、ビット 1 = 黒を通常のチェスの配置に)
  player: "w", // エンジン対局でのこちらの色
  flipped: false,
  selected: null,
  inspect: null, // 相手の駒や取られた駒をタップして見ているとき { sq } | { cap }、ply はその時点の手数
  hover: null, // マウスを乗せている (またはドラッグ中の指の下の) 駒 { sq } | { cap }
  thinking: false,
  token: 0, // 対局や待ったで増やし、古い探索結果を捨てる
  debug: false, // 隠しコマンドで評価・最善手を表示する
  analyzer: null, // 評価・最善手用のエンジン (対局用とは別の Worker)
  animating: false, // 直前の手を段階的に見せている間は true (入力を受け付けない)
  error: null, // { key, vars } (言語を切り替えても訳し直せるようにキーで持つ)
  cpuDepth: null, // CPU が直前の手で読んだ深さ { depth, ply } (ply はその手を指した後の手数)
};

// ---- 設定の保存 ---------------------------------------------------------
const SETTINGS = ["lang", "opponent", "setup", "color", "level", "guard", "overlay"];

/**
 * 強さレベル n (1〜10): 「思考 n 秒以内」か「深さ n 手以内」の先に達した方で止める。
 * レベル 11 は思考 10 秒以内で、深さの制限なし
 */
/** 隠しコマンドの解析は、レベル 11 と同じく思考 10 秒以内・深さの制限なし */
const ANALYSIS_LIMITS = [10000, 64];

const levelLimits = () => {
  const n = Number($("level").value);
  return n >= 11 ? ANALYSIS_LIMITS : [n * 1000, n];
};

function loadSettings() {
  try {
    const s = JSON.parse(localStorage.getItem("qchess-settings") || "{}");
    for (const id of SETTINGS) {
      if ($(id).type === "checkbox") $(id).checked = s[id] === true;
      else if (s[id] && $(id).querySelector(`option[value="${s[id]}"]`)) $(id).value = s[id];
    }
  } catch {}
}
function saveSettings() {
  try {
    localStorage.setItem("qchess-settings", JSON.stringify(Object.fromEntries(SETTINGS.map((id) => [id, $(id).type === "checkbox" ? $(id).checked : $(id).value]))));
  } catch {}
}

function updateSettingsUI() {
  // 手番はエンジン対局と、オンライン対戦で招待する側が選ぶ
  $("color-box").hidden = !["engine", "online"].includes($("opponent").value);
  // CPU の強さは「対CPU」を選んでいるときだけ
  $("cpu-box").hidden = $("opponent").value !== "engine";
  saveSettings();
}

/** 言語を切り替えて画面全体を訳し直す */
function applyLang() {
  setLang($("lang").value || DEFAULT_LANG);
  applyStatic();
  analysisKey = null;
  if (state.rules) render();
  saveSettings();
}

// ---- 盤面の解釈 -----------------------------------------------------------
/** "wpnb" → { color: "w", kinds: ["p","n","b"] } */
const parseToken = (tok) => ({ color: tok[0], kinds: [...tok.slice(1)] });

const engineTurn = () => state.opponent === "engine" && state.rules.turn() !== state.player;

function displayToSq(row, col) {
  return state.flipped ? row * 8 + (7 - col) : (7 - row) * 8 + col;
}

/**
 * こちらが指せる手。オプションが ON なら、自分の盤上の駒がどれもキングでありえなくなる
 * (指した瞬間に負けになる) 手を除く。全部の手がそうなら、何も指せなくならないよう全部残す
 */
let playableCache = { key: null, moves: [] };
function playableMoves() {
  const { rules } = state;
  const all = rules.legalMoves();
  if (!$("guard").checked) return all;
  const key = state.setup + "|" + rules.history().join(" ");
  if (playableCache.key === key) return playableCache.moves;
  const safe = all.filter((code) => {
    if (!rules.play(code)) return true;
    const lost = rules.status() === "kingRevealed";
    rules.undo();
    return !lost;
  });
  playableCache = { key, moves: safe.length ? safe : all };
  return playableCache.moves;
}

function legalTargets(from) {
  const name = sqName(from);
  return [...new Set(playableMoves().filter((m) => m.startsWith(name)).map((m) => sqIndex(m.slice(2, 4))))];
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
  return t("describe", { color: colorName(color), kinds: kinds.map(kindName).join(t("kindSep")) });
};

/** { sq } | { cap } の駒のトークン (空きマスなら null) */
function tokenOf(ref) {
  const tok = ref.sq != null ? state.rules.board()[ref.sq] : state.rules.captured()[ref.cap];
  return tok && tok !== "." ? tok : null;
}

/**
 * 状態欄の 2 行目に出す駒: 選択中の自分の駒 > マウスを乗せている駒 > タップした相手の駒・取った駒。
 * 取った駒なら captured = true
 */
function lookedAt() {
  const { rules } = state;
  const ins = state.inspect?.ply === rules.history().length ? state.inspect : null;
  for (const ref of [state.selected != null ? { sq: state.selected } : null, state.hover, ins]) {
    const tok = ref && tokenOf(ref);
    if (tok) return { tok, captured: ref.cap != null };
  }
  return null;
}

/**
 * 選択中の駒をマウス (または指) の下のマスへ動かせないときの理由。動かせる・対象外なら ""。
 * 自分の駒があるマスは (クリックするとその駒を選び直すだけなので) 説明しない
 */
function whyNotHere() {
  const { rules } = state;
  const from = state.selected;
  const to = state.hover?.sq;
  if (from == null || to == null || to === from || !canMove()) return "";
  const prefix = sqName(from) + sqName(to);
  if (playableMoves().some((m) => m.startsWith(prefix))) return "";
  if (rules.legalMoves().some((m) => m.startsWith(prefix))) return t("whyGuard");
  const r = rules.whyNot(from, to);
  if (!r || r.kind === "own") return "";
  if (r.kind === "check") return t("whyCheck", { sq: r.sq });
  if (r.kind === "castle") return t("whyCastle");
  const { kinds } = parseToken(rules.board()[from]);
  return t("whyReach", { kinds: kinds.map(kindName).join(t("kindSep")) });
}

/** 選択中の駒をマウス (または指) の下のマスへ動かした後の種類。候補が複数 (成り等) なら全部 */
function afterMove() {
  const { rules } = state;
  const from = state.selected;
  const to = state.hover?.sq;
  if (from == null || to == null || to === from || !canMove()) return [];
  const prefix = sqName(from) + sqName(to);
  const out = [];
  for (const code of playableMoves().filter((m) => m.slice(0, 4) === prefix)) {
    if (!rules.play(code)) continue;
    const tok = rules.board()[to];
    rules.undo();
    if (!out.includes(tok)) out.push(tok);
  }
  return out;
}

const inspecting = (key, v) => state.inspect?.[key] === v && state.inspect.ply === state.rules.history().length;

/** 相手の駒・取られた駒をタップしたら、その重ね合わせを状態欄に出す (もう一度タップで解除) */
function inspectAt(key, v, occupied) {
  state.selected = null;
  state.inspect = occupied && !inspecting(key, v) ? { [key]: v, ply: state.rules.history().length } : null;
  render();
}

/**
 * 盤面を描く。stage を渡すと直前の手を段階的に見せる途中の状態を描く:
 * - { kind: "move", pv }: 駒が動いた直後 (重ね合わせは指す前の種類のまま)
 * - { kind: "collapse", pv }: 重ね合わせを更新し、種類が減った駒を光らせる
 * どちらの段階でも、手番・チェック・決着はまだ表示しない。
 */
function render(stage = null) {
  const { rules } = state;
  const pv = stage?.pv;
  const board = stage?.kind === "move" ? pv.board : rules.board();
  const changed = new Set(stage?.kind === "collapse" ? pv.changed : []);
  const hist = rules.history();
  const last = hist.length ? hist[hist.length - 1] : null;
  const lastSqs = last ? [sqIndex(last.slice(0, 2)), sqIndex(last.slice(2, 4))] : [];
  const targets = state.selected != null ? legalTargets(state.selected) : [];
  const turn = rules.turn();
  // チェックされている駒 (盤上で最後のキング候補。キングに確定しているとは限らない)
  const checkSq = !stage && rules.inCheck() ? board.findIndex((tk) => tk[0] === turn && tk.includes("k")) : -1;

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
      if (inspecting("sq", sq)) el.classList.add("inspect");
      if (sq === checkSq) el.classList.add("check");
      if (targets.includes(sq)) el.classList.add("target");
      if (changed.has(sq)) el.classList.add("collapsed");

      const tok = board[sq];
      if (tok !== ".") {
        el.classList.add("occupied");
        el.title = describe(tok);
        el.append(pieceEl(tok));
      }
      if (col === 0) el.append(coord("rank", sqName(sq)[1]));
      if (row === 7) el.append(coord("file", sqName(sq)[0]));
      boardEl.append(el);
    }
  }
  renderCaptured(stage);
  renderMoves(!!stage);
  if (stage) renderStaging();
  else {
    renderStatus();
    analyze();
  }
}

function coord(cls, text) {
  const s = document.createElement("span");
  s.className = `coord ${cls}`;
  s.textContent = text;
  return s;
}

/** 取られた駒。取った側の近くに置く (盤の下側には上側の色の取られた駒、上側には下側の色の駒) */
function renderCaptured(stage = null) {
  const caps = stage?.kind === "move" ? stage.pv.captured : state.rules.captured();
  const changed = new Set(stage?.kind === "collapse" ? stage.pv.changedCaptured : []);
  const bottom = state.flipped ? "b" : "w";
  const top = bottom === "w" ? "b" : "w";
  for (const [id, color] of [["captured-bottom", top], ["captured-top", bottom]]) {
    const box = $(id);
    box.replaceChildren();
    // ポーン > ナイト > ビショップ > ルーク > クイーン > キング > 不確定な駒 の順
    const rank = (tok) => (tok.length === 2 ? "pnbrqk".indexOf(tok[1]) : 6);
    const list = [...caps.entries()].filter(([, tok]) => tok[0] === color).sort((a, b) => rank(a[1]) - rank(b[1]));
    for (const [i, tok] of list) {
      const c = document.createElement("span");
      c.className = "cap" + (changed.has(i) ? " collapsed" : "") + (inspecting("cap", i) ? " inspect" : "");
      c.dataset.i = i;
      c.title = t("captured", { desc: describe(tok) });
      c.append(pieceEl(tok));
      box.append(c);
    }
  }
}

/** staging = true のときは、最後の手の王手・決着の記号 (+ # †) をまだ見せない */
function renderMoves(staging = false) {
  const notes = state.rules.notations();
  if (staging && notes.length) notes[notes.length - 1] = notes[notes.length - 1].replace(/[+#†∅]$/, "");
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
}


/** 勝者の色 (決着していなければ null) */
function winnerOf(st, turn) {
  const other = turn === "w" ? "b" : "w";
  if (st === "checkmate" || st === "kingCaptured") return other; // 手番側の負け
  if (st === "kingRevealed") return turn; // 直前に指した側の負け
  return null;
}

function resultText(st, turn) {
  const winner = winnerOf(st, turn);
  const loser = winner === "w" ? "b" : "w";
  const how = t(st, { loser: colorName(loser) });
  if (state.opponent === "human") return t("wins", { how, winner: colorName(winner) });
  return t(winner === state.player ? "youWin" : "youLose", { how });
}

/**
 * 状態欄: 1 行目に対局状況 (手番・チェック・決着など)、2 行目に見ている駒の重ね合わせ、
 * 3 行目に選択中の駒を動かした後の重ね合わせ (→ …)
 */
function setStatus(main, thinking = false) {
  const line = (cls, text) => {
    const el = document.createElement(cls === "st-piece" ? "strong" : "span");
    el.className = cls;
    el.textContent = text;
    return el;
  };
  const looked = state.rules && !state.error ? lookedAt() : null;
  const piece = looked ? (looked.captured ? t("captured", { desc: describe(looked.tok) }) : describe(looked.tok)) : "";
  const after = state.rules && !state.error && !thinking ? afterMove() : [];
  const why = state.rules && !state.error && !thinking && !after.length ? whyNotHere() : "";
  if (onlineGame() && state.netUnstable && !state.error) main += t("netUnstable");
  statusEl.classList.toggle("thinking", thinking);
  statusEl.replaceChildren(line("st-main", main));
  if (piece) statusEl.append(line("st-piece", piece));
  if (after.length) statusEl.append(line("st-after", "→ " + after.map(describe).join(" / ")));
  else if (why) statusEl.append(line("st-reason", why));
}

function renderStaging() {
  setStatus(t("staging"), true);
  $("undo").disabled = onlineGame();
}

/**
 * 直前の手を段階的に見せる: 駒の移動 → 重ね合わせの更新 → 手番・チェック・決着。
 * 途中で待った・新規対局が入ったら false を返す (その後の処理はしない)。
 */
async function showLastMove() {
  const pv = state.rules.preview();
  const token = state.token;
  if (pv && (pv.changed.length || pv.changedCaptured.length)) {
    state.animating = true;
    render({ kind: "move", pv });
    await sleep(STAGE_MS);
    if (token !== state.token) return false;
    render({ kind: "collapse", pv });
    await sleep(STAGE_MS);
    if (token !== state.token) return false;
  }
  state.animating = false;
  render();
  return true;
}

function renderStatus() {
  const { rules } = state;
  const st = rules.status();
  const turn = rules.turn();
  if (state.error) {
    setStatus(t(state.error.key, state.error.vars));
  } else if (winnerOf(st, turn)) {
    setStatus(resultText(st, turn));
  } else if (st !== "ongoing") {
    setStatus(t(st));
  } else if (state.thinking) {
    setStatus(t("thinking"), true);
  } else if (engineTurn()) {
    setStatus(t("engineLoading"));
  } else if (onlineGame() && !connected()) {
    setStatus(t("notConnected"));
  } else if (remoteTurn()) {
    setStatus(t("opponentTurn", { color: colorName(turn) }) + (rules.inCheck() ? t("check") : ""));
  } else {
    const key = state.opponent === "human" ? "toMove" : "toMoveYou";
    // CPU の直前の手が何手先まで読んで指したものか
    const d = state.opponent === "engine" && state.cpuDepth?.ply === rules.history().length ? t("cpuDepth", state.cpuDepth) : "";
    setStatus(t(key, { color: colorName(turn) }) + (rules.inCheck() ? t("check") : "") + d);
  }
  $("undo").disabled = rules.history().length === 0 || onlineGame();
  $("new-game").disabled = onlineGame() && connected() && state.role === "guest";
  $("new-game").title = $("new-game").disabled ? t("hostOnly") : "";
}

// ---- 入力 ---------------------------------------------------------------
const onlineGame = () => state.opponent === "online";
const connected = () => !!state.link?.connected;
/** オンライン対戦で、相手の手番か未接続なら true */
const remoteTurn = () => onlineGame() && (!connected() || state.rules.turn() !== state.player);

function canMove() {
  return !state.thinking && !state.animating && state.rules.status() === "ongoing" && !engineTurn() && !remoteTurn();
}

function ownPiece(sq) {
  const tok = state.rules.board()[sq];
  return tok !== "." && tok[0] === state.rules.turn();
}

/** 同じ移動元・移動先で複数の解釈がある手 (成り・キャスリング・アンパッサン) の表示名 */
function choiceLabel(code, all) {
  const s = code.slice(4);
  if ("qrbn".includes(s) && s) return { glyph: s, text: t("promoteTo", { kind: kindName(s) }) };
  if (s === "c") return { text: t("castle") };
  if (s === "e") return { text: t("enPassant") };
  const promo = all.some((c) => "qrbn".includes(c.slice(4)) && c.length === 5);
  if (promo) return { text: t("noPromote") };
  // キャスリングと並ぶ「横に 2 マス以上の移動」はルーク・クイーンとしての手
  if (all.some((c) => c.slice(4) === "c")) return { text: t("castleAlt") };
  return { text: t("normalMove") };
}

async function tryMove(from, to) {
  const prefix = sqName(from) + sqName(to);
  const cands = playableMoves().filter((m) => m.slice(0, 4) === prefix);
  if (cands.length === 0) return false;
  let code = cands[0];
  if (cands.length > 1) {
    code = await askChoice(t("whichMove"), cands.map((c) => ({ value: c, ...choiceLabel(c, cands) })));
    if (!code) return false;
  }
  if (!(await confirmLostPawn(from, to, code))) return false;
  state.selected = null;
  if (!state.rules.play(code)) return false;
  if (onlineGame()) state.link.send({ type: "move", code, ply: state.rules.history().length - 1 });
  if (await showLastMove()) maybeEngineMove();
  return true;
}

/**
 * ポーンでありうる駒を 2 マス前に動かすのに、ポーンとしては 2 マス進めない (既に動かしたことがある) とき、
 * ルーク・クイーンなどとしての移動になってポーンの可能性が消えるので、確認してから指す
 */
async function confirmLostPawn(from, to, code) {
  const { rules } = state;
  const tok = rules.board()[from];
  const dir = tok[0] === "w" ? 1 : -1;
  if (!tok.slice(1).includes("p") || to - from !== 16 * dir) return true;
  // 確認するのは 2 段目 (黒は 7 段目) からの手だけ (ほかの段からはポーンとして 2 マス進めないのが見れば分かる)
  if (Math.floor(from / 8) !== (dir === 1 ? 1 : 6)) return true;
  // 試しに指して、ポーンの可能性が残るかを見る
  if (!rules.play(code)) return true;
  const keepsPawn = rules.board()[to].slice(1).includes("p");
  rules.undo();
  if (keepsPawn) return true;
  const ok = await askChoice(
    t("lostPawnTitle"),
    [
      { value: "yes", text: t("lostPawnYes") },
      { value: "no", text: t("lostPawnNo") },
    ],
    false, // 「やめる」があるのでキャンセルボタンは出さない
  );
  return ok === "yes";
}

/** 選択肢のダイアログ。選んだ value (キャンセル・閉じたら null) を返す */
function askChoice(title, options, cancelButton = true) {
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
    if (cancelButton) {
      const c = document.createElement("button");
      c.className = "cancel";
      c.textContent = t("cancel");
      c.onclick = () => dlg.close();
      box.append(c);
    }
    dlg.onclose = () => resolve(picked);
    dlg.showModal();
  });
}

// クリックとドラッグの両方に対応
let drag = null;

boardEl.addEventListener("pointerdown", (ev) => {
  if (ev.button !== 0) return; // 右クリックは重ね合わせの一覧表示用
  const el = ev.target.closest(".sq");
  if (!el) return;
  const sq = Number(el.dataset.sq);
  const occupied = state.rules.board()[sq] !== ".";
  if (!canMove()) return inspectAt("sq", sq, occupied);

  if (state.selected != null && state.selected !== sq && !ownPiece(sq)) {
    tryMove(state.selected, sq).then((ok) => {
      if (!ok) inspectAt("sq", sq, occupied);
    });
    return;
  }
  if (!ownPiece(sq)) return inspectAt("sq", sq, occupied);

  state.inspect = null;
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

/** マウス (ドラッグ中は指) の下の駒を覚えて、状態欄の 2・3 行目を更新する */
function setHover(ref) {
  if (JSON.stringify(state.hover) === JSON.stringify(ref)) return;
  state.hover = ref;
  if (state.rules && !state.animating) renderStatus();
}

boardEl.addEventListener("pointermove", (ev) => {
  if (ev.pointerType === "mouse" || drag) {
    const el = document.elementFromPoint(ev.clientX, ev.clientY)?.closest(".sq");
    setHover(el ? { sq: Number(el.dataset.sq) } : null);
  }
  if (!drag) return;
  if (!drag.moved && Math.hypot(ev.clientX - drag.x, ev.clientY - drag.y) > 4) {
    drag.moved = true;
    drag.ghost.hidden = false;
    drag.piece.classList.add("dragging");
  }
  drag.ghost.style.left = ev.clientX + "px";
  drag.ghost.style.top = ev.clientY + "px";
});

boardEl.addEventListener("pointerleave", (ev) => {
  if (!drag) setHover(null);
});

function endDrag(ev) {
  if (ev.pointerType !== "mouse") setHover(null);
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
for (const id of ["captured-top", "captured-bottom"]) {
  $(id).addEventListener("click", (ev) => {
    const c = ev.target.closest(".cap");
    if (c) inspectAt("cap", Number(c.dataset.i), true);
  });
  $(id).addEventListener("pointerover", (ev) => {
    const c = ev.target.closest(".cap");
    if (ev.pointerType === "mouse") setHover(c ? { cap: Number(c.dataset.i) } : null);
  });
  $(id).addEventListener("pointerleave", () => setHover(null));
}

// 盤の外をタップ (クリック) したら選択を外す (スマホで選択を解除する手段)。
// 盤のクリックで盤が描き直されるとクリックしたマスが盤から外れて「盤の外」に見えるので、
// 盤の処理より先に (キャプチャ段階で) 判定する
document.addEventListener(
  "pointerdown",
  (ev) => {
    if (ev.button !== 0 || ev.target.closest("#board, .captured, dialog")) return;
    if (state.selected == null && state.inspect == null) return;
    state.selected = null;
    state.inspect = null;
    if (state.rules && !state.animating) render();
  },
  true,
);

// 右クリックを押している間は、全部の駒の重ね合わせを円形に並べて見せる
const boardWrap = document.querySelector(".board-wrap");
boardWrap.addEventListener("contextmenu", (ev) => ev.preventDefault());
boardWrap.addEventListener("pointerdown", (ev) => {
  if (ev.button === 2) document.body.classList.add("peek");
});
const endPeek = () => document.body.classList.remove("peek");
document.addEventListener("pointerup", (ev) => {
  if (ev.button === 2) endPeek();
});
window.addEventListener("blur", endPeek);
boardEl.addEventListener("pointercancel", endDrag);

// ---- エンジン -----------------------------------------------------------
function formatEval(r, engineColor) {
  const sign = engineColor === "w" ? 1 : -1; // 白から見た値にそろえる
  const v = r.score * sign;
  if (Math.abs(v) > 90000) return t("evalWin", { color: colorName(v > 0 ? "w" : "b"), depth: r.depth });
  return t("eval", { value: `${v >= 0 ? "+" : ""}${(v / 100).toFixed(2)}`, depth: r.depth });
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
    r = await engine.search(rules.history(), ...levelLimits(), state.setup);
  } catch (e) {
    if (token !== state.token) return; // 取り消し
    state.thinking = false;
    state.error = { key: "errEngine", vars: { msg: e.message } };
    renderStatus();
    return;
  }
  const wait = MIN_ENGINE_DELAY - (performance.now() - started);
  if (wait > 0) await new Promise((res) => setTimeout(res, wait));
  if (token !== state.token) return; // 待った・新規対局で無効になった

  state.thinking = false;
  if (r && !rules.play(r.move)) {
    state.error = { key: "errIllegal", vars: { move: r.move } };
  } else if (r) {
    state.cpuDepth = { depth: r.depth, ply: rules.history().length };
    await showLastMove();
    return;
  }
  render();
}

function cancelEngine() {
  state.token++;
  state.animating = false;
  if (state.thinking) state.engine?.cancel();
  state.thinking = false;
}

// ---- 隠しコマンド: 製作者名をタイプすると評価・最善手を左上に出す ----------------
const SECRET = "volatile rye";
let typed = "";
let analysisKey = null;

document.addEventListener("keydown", (ev) => {
  if (ev.key.length !== 1 || ev.ctrlKey || ev.metaKey || ev.altKey) return;
  typed = (typed + ev.key.toLowerCase()).slice(-SECRET.length);
  if (typed !== SECRET) return;
  typed = "";
  state.debug = !state.debug;
  $("debug").hidden = !state.debug;
  if (state.debug && !state.analyzer) state.analyzer = new Engine(WORKER_URL, WASM_URL);
  if (!state.debug) state.analyzer?.cancel();
  analysisKey = null;
  analyze();
});

/** 今の局面を (対局用とは別のエンジンで) 探索し、評価と最善手を表示する */
async function analyze() {
  if (!state.debug || !state.analyzer || !state.rules) return;
  const hist = state.rules.history();
  const key = state.setup + "|" + hist.join(" ") + "|" + state.rules.status();
  if (key === analysisKey) return;
  analysisKey = key;
  const box = $("debug");
  state.analyzer.cancel();
  if (state.rules.status() !== "ongoing") {
    box.textContent = "—";
    return;
  }
  box.textContent = t("analyzing");
  const color = state.rules.turn();
  try {
    const r = await state.analyzer.search(hist, ...ANALYSIS_LIMITS, state.setup);
    if (key !== analysisKey) return;
    const m = r.move;
    const move = `${m.slice(0, 2)}-${m.slice(2, 4)}` + ({ q: "=Q", r: "=R", b: "=B", n: "=N", c: " (O-O)", e: " e.p." }[m[4]] ?? "");
    box.textContent = `${formatEval(r, color)}\n${t("bestMove", { move })}`;
  } catch (e) {
    if (key === analysisKey && e.message !== "cancelled") box.textContent = t("errEngine", { msg: e.message });
  }
}

// ---- オンライン対戦 -------------------------------------------------------
const resolveColor = () => {
  const c = $("color").value;
  return c === "r" ? (Math.random() < 0.5 ? "w" : "b") : c;
};

/** オンライン対戦の対局を始める (自分の色 player、初期配置 setup) */
function startGame(setup, player) {
  cancelEngine();
  state.opponent = "online";
  state.player = player;
  state.flipped = player === "b";
  state.selected = null;
  state.inspect = null;
  state.error = null;
  state.setup = setup;
  state.rules.newGame(setup);
  render();
  saveSettings();
}

function onlineStatus(key, vars) {
  $("online-status").textContent = key ? t(key, vars) : "";
}

const pageUrl = () => location.href.split("#")[0];

function newLink(role) {
  state.link?.close();
  state.role = role;
  state.netUnstable = false;
  state.link = new Link(onRemoteMessage, (st) => {
    if (st === "open") {
      onlineStatus("netConnected");
      $("online-dialog").close();
      $("opponent").value = "online";
      updateSettingsUI();
      if (role === "host") newGame();
    } else if (st === "timeout") {
      onlineStatus("netTimeout");
    } else if (st === "unstable" || st === "stable") {
      // 相手のブラウザが裏に回ったときなど。相手が戻れば自動で直る
      state.netUnstable = st === "unstable";
      if (state.rules && !state.animating) renderStatus();
    } else {
      onlineStatus("netClosed");
      state.netUnstable = false;
      state.error = state.opponent === "online" ? { key: "netClosed" } : state.error;
      if (state.rules) render();
    }
  });
  return state.link;
}

async function onRemoteMessage(msg) {
  const { rules } = state;
  if (msg.type === "start" && state.role === "guest") {
    $("setup").value = String(msg.setup);
    startGame(Number(msg.setup), msg.hostColor === "w" ? "b" : "w");
  } else if (msg.type === "move" && onlineGame()) {
    if (msg.ply !== rules.history().length || rules.turn() === state.player || !rules.play(msg.code)) {
      state.error = { key: "errSync", vars: { move: msg.code } };
      render();
      return;
    }
    state.selected = null;
    await showLastMove();
  } else if (msg.type === "bye") {
    state.link?.close();
    state.error = { key: "netClosed" };
    render();
  }
}

/** 接続のダイアログ。mode: "host" (招待する) / "guest" (招待リンクから開いた) / "relay" (返信リンクを受け渡すだけ) */
function openOnline(mode = "host") {
  $("online-host").hidden = mode !== "host";
  $("online-guest").hidden = mode !== "guest";
  onlineStatus(null);
  if (!$("online-dialog").open) $("online-dialog").showModal();
}

async function copyText(text, button) {
  try {
    await navigator.clipboard.writeText(text);
    const label = button.textContent;
    button.textContent = t("copied");
    setTimeout(() => (button.textContent = label), 1500);
  } catch {
    prompt("", text);
  }
}

/** OS の共有メニュー (スマホなら LINE などに直接送れる)。使えないブラウザではボタンを出さない */
async function shareUrl(url) {
  try {
    await navigator.share({ title: t("pageTitle"), url });
  } catch {}
}
for (const id of ["share-invite", "share-reply"]) $(id).hidden = !navigator.share;

$("close-online").onclick = () => $("online-dialog").close();
$("make-invite").onclick = async () => {
  onlineStatus("netWorking");
  $("invite-actions").hidden = true;
  try {
    state.inviteUrl = pageUrl() + "#join=" + (await newLink("host").createInvite());
    $("invite-actions").hidden = false;
    onlineStatus("inviteReady");
  } catch (e) {
    onlineStatus("netFailed", { msg: e.message });
  }
};
$("copy-invite").onclick = (ev) => copyText(state.inviteUrl, ev.target);
$("share-invite").onclick = () => shareUrl(state.inviteUrl);
$("copy-reply").onclick = (ev) => copyText(state.replyUrl, ev.target);
$("share-reply").onclick = () => shareUrl(state.replyUrl);

/** 招待リンク (#join=…) から開いたとき: 返信リンクを作る */
async function joinFromLink(code) {
  $("opponent").value = "online";
  updateSettingsUI();
  openOnline("guest");
  $("reply-actions").hidden = true;
  onlineStatus("netWorking");
  try {
    state.replyUrl = pageUrl() + "#reply=" + (await newLink("guest").joinWithInvite(code));
    $("reply-actions").hidden = false;
    onlineStatus("netWaiting");
  } catch {
    onlineStatus("badLink");
  }
}

// 返信リンクは新しいタブで開かれることが多いので、同じブラウザの「招待を作ったタブ」へ受け渡す
const channel = "BroadcastChannel" in window ? new BroadcastChannel("qchess-online") : null;

async function useReply(code) {
  openOnline("host");
  try {
    await state.link.acceptReply(code);
    onlineStatus("netWaiting");
  } catch {
    onlineStatus("badLink");
  }
}

const waitingForReply = (id) => state.role === "host" && state.link && !connected() && state.link.id === id;

channel?.addEventListener("message", async (ev) => {
  const m = ev.data;
  if (m?.type === "reply" && waitingForReply(m.id)) {
    channel.postMessage({ type: "ack", nonce: m.nonce });
    await useReply(m.code);
  }
});

/** 返信リンク (#reply=…) から開いたとき */
async function replyFromLink(code) {
  const id = await inviteIdOf(code);
  if (waitingForReply(id)) return useReply(code); // このタブで作った招待への返信
  openOnline("relay");
  onlineStatus("relaying");
  const nonce = Math.random();
  const acked = await new Promise((resolve) => {
    if (!channel || !id) return resolve(false);
    const timer = setTimeout(() => resolve(false), 2000);
    channel.addEventListener("message", function onAck(ev) {
      if (ev.data?.type === "ack" && ev.data.nonce === nonce) {
        clearTimeout(timer);
        channel.removeEventListener("message", onAck);
        resolve(true);
      }
    });
    channel.postMessage({ type: "reply", id, code, nonce });
  });
  onlineStatus(acked ? "relayDone" : "relayNoTab");
  if (acked) setTimeout(() => window.close(), 500); // 開けた場合だけ閉じられる
}

/** URL の #join= / #reply= を処理する (処理したら URL から消す) */
function handleLinkHash() {
  const m = location.hash.match(/^#(join|reply)=(.+)$/);
  if (!m) return;
  history.replaceState(null, "", pageUrl());
  const code = decodeURIComponent(m[2]);
  if (m[1] === "join") joinFromLink(code);
  else replyFromLink(code);
}
window.addEventListener("hashchange", handleLinkHash);

// ---- ボタン -------------------------------------------------------------
function newGame() {
  if ($("opponent").value === "online") {
    // 未接続なら接続のダイアログを開く。接続済みなら招待した側だけが新しい対局を始める
    if (!connected()) return openOnline();
    if (state.role !== "host") return;
    startGame(Number($("setup").value), resolveColor());
    state.link.send({ type: "start", setup: state.setup, hostColor: state.player });
    return;
  }
  if (state.link) {
    state.link.send({ type: "bye" });
    state.link.close();
    state.link = null;
  }
  cancelEngine();
  state.opponent = $("opponent").value;
  const c = $("color").value;
  state.player = c === "r" ? (Math.random() < 0.5 ? "w" : "b") : c;
  state.flipped = state.opponent === "engine" && state.player === "b";
  state.selected = null;
  state.inspect = null;
  state.error = null;
  state.setup = Number($("setup").value);
  state.rules.newGame(state.setup);
  render();
  saveSettings();
  maybeEngineMove();
}

function undo() {
  const { rules } = state;
  if (rules.history().length === 0 || onlineGame()) return;
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
  state.inspect = null;
  state.error = null;
  render();
  maybeEngineMove(); // 後手で初手まで戻した場合はエンジンが指し直す
}

function kifu() {
  const notes = state.rules.notations();
  const st = state.rules.status();
  let result = "*";
  const winner = winnerOf(st, state.rules.turn());
  if (winner) result = winner === "w" ? "1-0" : "0-1";
  else if (st !== "ongoing") result = "1/2-1/2";
  const d = new Date();
  const date = `${d.getFullYear()}.${String(d.getMonth() + 1).padStart(2, "0")}.${String(d.getDate()).padStart(2, "0")}`;
  const other = onlineGame() ? "Opponent" : "CPU";
  const name = (c) => (state.opponent === "human" ? (c === "w" ? "White" : "Black") : c === state.player ? "You" : other);
  const headers = [
    `[Event "Casual game"]`,
    `[Variant "Quantum superposition chess"]`,
    `[Setup "${["both superposed", "white classical", "black classical", "both classical"][state.setup]}"]`,
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
// ルールのダイアログ (閉じるボタン・枠の外のクリック・Esc で閉じる)
$("show-rules").onclick = () => $("rules-dialog").showModal();
$("close-rules").onclick = () => $("rules-dialog").close();
$("rules-dialog").addEventListener("click", (ev) => {
  const r = $("rules-dialog").getBoundingClientRect();
  const outside = ev.clientX < r.left || ev.clientX > r.right || ev.clientY < r.top || ev.clientY > r.bottom;
  if (ev.target === $("rules-dialog") && outside) $("rules-dialog").close();
});
$("undo").onclick = undo;
$("flip").onclick = () => {
  state.flipped = !state.flipped;
  render();
};
$("copy-pgn").onclick = async () => {
  try {
    await navigator.clipboard.writeText(kifu());
    $("copy-pgn").textContent = t("copied");
    setTimeout(() => ($("copy-pgn").textContent = t("copy")), 1500);
  } catch {
    prompt(t("recordPrompt"), kifu());
  }
};
for (const id of SETTINGS) $(id).addEventListener("input", id === "lang" ? applyLang : updateSettingsUI);
// OFF (初期値) のときは、ホバー・右クリック時と同じく常に種類を円形に並べる
const applyOverlay = () => document.body.classList.toggle("overlay", $("overlay").checked);
$("overlay").addEventListener("change", applyOverlay);
$("guard").addEventListener("change", () => {
  playableCache.key = null;
  if (state.rules) render();
});

// ---- 起動 ---------------------------------------------------------------
async function main() {
  loadSettings();
  applyOverlay();
  applyLang();
  updateSettingsUI();
  try {
    state.rules = await loadRules(WASM_URL);
  } catch (e) {
    statusEl.textContent = t("errWasm", { msg: e.message });
    return;
  }
  state.opponent = $("opponent").value;
  state.setup = Number($("setup").value);
  state.rules.newGame(state.setup);
  const c = $("color").value;
  state.player = c === "r" ? (Math.random() < 0.5 ? "w" : "b") : c;
  state.flipped = state.opponent === "engine" && state.player === "b";
  render();

  const engine = new Engine(WORKER_URL, WASM_URL);
  try {
    await engine.init();
  } catch (e) {
    state.error = { key: "errEngineLoad", vars: { msg: e.message } };
    renderStatus();
    return;
  }
  state.engine = engine;
  maybeEngineMove();
}

main().then(handleLinkHash);
