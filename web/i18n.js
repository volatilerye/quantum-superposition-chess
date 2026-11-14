// 画面の文言 (英語 / 日本語)。
// HTML 側は data-i18n="キー" (textContent)、data-i18n-aria="キー" (aria-label) で参照する。
// 文言中の {name} は t() に渡した値、または要素の data-* 属性の値で置き換える。

const en = {
  pageTitle: "Quantum Superposition Chess",
  language: "Language",
  boardLabel: "Chess board",
  loading: "Loading…",

  newGame: "New game",
  opponent: "Opponent",
  optEngine: "Engine",
  optHuman: "Human (same screen, taking turns)",
  side: "Play as",
  optWhite: "White (moves first)",
  optBlack: "Black (moves second)",
  optRandom: "Random",
  start: "Start",
  engine: "Engine",
  strength: "Strength",
  levelOpt: "Level {n}",
  changeNote: "Changes take effect from the next move.",
  undo: "Undo",
  flip: "Flip board",
  copy: "Copy game record",
  guard: "Block moves that leave none of your pieces on the board able to be the king",
  copied: "Copied",
  recordPrompt: "Game record",

  rules: "Rules",
  rule1: "Every piece in the starting position is a superposition of pawn, knight, bishop, rook, queen and king.",
  rule2:
    "A piece moves as one of the kinds in its superposition. After the move, every kind inconsistent with that move is removed (e.g. a knight's jump collapses the piece to a knight).",
  rule3:
    "Each side's pieces must be assignable to the standard starting set (8 pawns, 2 knights, 2 bishops, 2 rooks, 1 queen, 1 king). Kinds that become impossible are removed even from unmoved and captured pieces.",
  rule4:
    "Check applies only to a piece confirmed as the king. You win when none of your opponent's pieces on the board can be the king (i.e. you have captured it).",
  rule5:
    "If your own move leaves none of your pieces on the board able to be the king (i.e. it confirms that a captured piece was your king), you lose on that move (marked “†” in the record).",
  rule6:
    "Castling: possible with an unmoved king candidate on e1 (e8 for Black) and an unmoved rook candidate on a1 or h1 (a8 or h8). The squares between them must be empty, and the king's square, the square it passes and its destination must not be attacked (a superposed enemy piece attacks a square if any of its possible kinds does). The two pieces collapse to a king and a rook. When the move can't be told apart from a rook or queen moving sideways (e.g. e1→g1), you choose which one you mean.",
  rule7:
    "A piece that has never moved may advance two squares as a pawn from its first rank as well as its second (e.g. b1→b3 for White, b8→b6 for Black). It can be captured en passant as usual.",
  hoverNote: "Hover over a piece to see its possible kinds arranged in a circle.",

  white: "White",
  black: "Black",
  kind_p: "Pawn",
  kind_n: "Knight",
  kind_b: "Bishop",
  kind_r: "Rook",
  kind_q: "Queen",
  kind_k: "King",
  kindSep: ", ",
  describe: "{color}: {kinds}",
  captured: "Captured — {desc}",

  stalemate: "Draw by stalemate",
  fifty: "Draw by the 50-move rule",
  threefold: "Draw by threefold repetition",
  insufficient: "Draw by insufficient material",
  checkmate: "Checkmate",
  kingCaptured: "King captured",
  kingRevealed: "{loser}'s king turns out to have been captured already",
  wins: "{how} — {winner} wins",
  youWin: "{how} — You win!",
  youLose: "{how} — You lose",
  staging: "Updating superpositions",
  thinking: "Engine is thinking",
  engineLoading: "Loading engine",
  toMove: "{color} to move",
  toMoveYou: "{color} to move (you)",
  check: " — Check!",

  whichMove: "Which move do you mean?",
  lostPawnTitle:
    "This piece has already moved, so it can't advance two squares as a pawn!\nIf you move it, it will no longer be able to be a pawn. Move it anyway?",
  lostPawnYes: "Move it",
  lostPawnNo: "Cancel",
  promoteTo: "Promote to {kind}",
  castle: "Castle",
  enPassant: "En passant (capture as a pawn)",
  noPromote: "Don't promote (move as a non-pawn)",
  normalMove: "Normal move (as a non-pawn)",

  evalWin: "Eval: {color} is winning (depth {depth})",
  eval: "Eval: {value} (depth {depth})",
  analyzing: "Analyzing…",
  bestMove: "Best move: {move}",

  errEngine: "Engine error: {msg}",
  errIllegal: "The engine returned an illegal move: {move}",
  errWasm: "Could not load the rules engine (WASM): {msg}",
  errEngineLoad: "Could not load the engine: {msg}",
};

const ja = {
  pageTitle: "量子重ね合わせチェス",
  language: "言語",
  boardLabel: "チェス盤",
  loading: "読み込み中…",

  newGame: "新しい対局",
  opponent: "相手",
  optEngine: "エンジン",
  optHuman: "人（同じ画面で交互に指す）",
  side: "手番",
  optWhite: "白（先手）",
  optBlack: "黒（後手）",
  optRandom: "ランダム",
  start: "対局開始",
  engine: "エンジン",
  strength: "強さ",
  levelOpt: "レベル {n}",
  changeNote: "変更は次の手から反映されます。",
  undo: "待った",
  flip: "盤を反転",
  copy: "棋譜をコピー",
  guard: "自分の盤上の駒がどれもキングでありえなくなる手を指せなくする",
  copied: "コピーしました",
  recordPrompt: "棋譜",

  rules: "ルール",
  rule1: "初期配置の駒はすべて、ポーン・ナイト・ビショップ・ルーク・クイーン・キングの重ね合わせです。",
  rule2: "駒は重ね合わせのどれか 1 種類として動きます。動いた後は、その動きと矛盾する種類が除外されます（例: ナイトの跳び方をしたらナイトに確定）。",
  rule3:
    "各色の駒は通常の初期配置の駒数（ポーン 8・ナイト 2・ビショップ 2・ルーク 2・クイーン 1・キング 1）に割り当てられなければならず、ありえなくなった種類は動いていない駒や取られた駒からも除外されます。",
  rule4: "チェックはキングに確定した駒にだけ適用されます。相手の盤上の駒のどれもキングでありえなくなったら（＝キングを取ったら）勝ちです。",
  rule5:
    "自分の手で、自分の盤上の駒のどれもキングでありえなくなった場合（＝取られた駒がキングだったと確定した場合）は、その手を指した時点で負けです（棋譜では「†」）。",
  rule6:
    "キャスリング: e1（黒は e8）にいる一度も動いていないキング候補の駒と、a1・h1（黒は a8・h8）にいる一度も動いていないルーク候補の駒で指せます。間のマスが空いていて、キングのいるマス・通過するマス・行き先のマスのどれも相手の駒に利かされていないことが条件です（相手の駒が重ね合わせのときは、ありうる種類のどれか 1 つでも利いていれば利かされているとみなします）。指すと、動いた 2 枚はそれぞれキングとルークに確定します。e1→g1 のようにルーク・クイーンとしての横移動と区別できない場合は、どちらとして指すかを選べます。",
  rule7: "一度も動いていない駒は、2 段目だけでなく 1 段目からもポーンとして 2 マス前進できます（白なら b1→b3、黒なら b8→b6 など）。通常どおりアンパッサンで取られることがあります。",
  hoverNote: "駒にマウスを乗せると、ありうる種類を円形に並べて表示します。",

  white: "白",
  black: "黒",
  kind_p: "ポーン",
  kind_n: "ナイト",
  kind_b: "ビショップ",
  kind_r: "ルーク",
  kind_q: "クイーン",
  kind_k: "キング",
  kindSep: "・",
  describe: "{color}: {kinds}",
  captured: "取った駒 — {desc}",

  stalemate: "ステイルメイトで引き分け",
  fifty: "50 手ルールで引き分け",
  threefold: "同一局面 3 回で引き分け",
  insufficient: "駒不足で引き分け",
  checkmate: "チェックメイト",
  kingCaptured: "キングを取りました",
  kingRevealed: "{loser}のキングはすでに取られていたことが確定しました",
  wins: "{how} — {winner}の勝ち",
  youWin: "{how} — あなたの勝ちです！",
  youLose: "{how} — あなたの負けです",
  staging: "重ね合わせを更新しています",
  thinking: "エンジンが考えています",
  engineLoading: "エンジンを読み込み中",
  toMove: "{color}番",
  toMoveYou: "{color}番（あなた）",
  check: " — チェック！",

  whichMove: "どの動きにしますか？",
  lostPawnTitle: "既に動かしたことのある駒なので、ポーンとしては 2 マス進めません！\n動かすとポーンの可能性がなくなりますが、動かしますか？",
  lostPawnYes: "動かす",
  lostPawnNo: "やめる",
  promoteTo: "{kind}に成る",
  castle: "キャスリング",
  enPassant: "アンパッサン（ポーンとして取る）",
  noPromote: "成らない（ポーン以外として動く）",
  normalMove: "通常の移動（ポーン以外として動く）",

  evalWin: "評価: {color}の勝ちが見えています（深さ {depth}）",
  eval: "評価: {value}（深さ {depth}）",
  analyzing: "解析中…",
  bestMove: "最善手: {move}",

  errEngine: "エンジンでエラーが起きました: {msg}",
  errIllegal: "エンジンが不正な手を返しました: {move}",
  errWasm: "ルールエンジン (WASM) を読み込めませんでした: {msg}",
  errEngineLoad: "エンジンを読み込めませんでした: {msg}",
};

export const LANGS = { en, ja };
export const DEFAULT_LANG = "en";

let lang = DEFAULT_LANG;

export function setLang(l) {
  lang = l in LANGS ? l : DEFAULT_LANG;
  document.documentElement.lang = lang;
}

export function getLang() {
  return lang;
}

/** 文言を取り出し、{name} を vars の値で置き換える */
export function t(key, vars = {}) {
  const s = LANGS[lang][key] ?? en[key] ?? key;
  return s.replace(/\{(\w+)\}/g, (_, k) => vars[k] ?? "");
}

/** HTML の静的な文言を今の言語で書き直す */
export function applyStatic(root = document) {
  for (const el of root.querySelectorAll("[data-i18n]")) el.textContent = t(el.dataset.i18n, el.dataset);
  for (const el of root.querySelectorAll("[data-i18n-aria]")) el.setAttribute("aria-label", t(el.dataset.i18nAria));
  document.title = t("pageTitle");
}
