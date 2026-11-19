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
  setup: "Setup",
  setup0: "Both superposed",
  setup1: "Only White classical",
  setup2: "Only Black classical",
  setup3: "Both classical",
  start: "Start",
  engine: "Engine",
  strength: "Strength",
  levelOpt: "Level {n}",
  changeNote: "Changes take effect from the next move.",
  undo: "Undo",
  flip: "Flip board",
  copy: "Copy game record",
  guard: "Block moves that leave none of your pieces on the board able to be the king",
  overlay: "Show possible kinds stacked on top of each other",
  copied: "Copied",
  recordPrompt: "Game record",

  rules: "Rules",
  rs_setup: "Starting position",
  r_setup1:
    "Every piece is a superposition of pawn, knight, bishop, rook, queen and king.",
  r_setup2:
    "With the “Setup” option, either side (or both) can start from the classical chess position instead.",
  rs_super: "Moves and superposition",
  r_super1:
    "A piece moves as one of the kinds in its superposition.",
  r_super2:
    "After the move, every kind inconsistent with it is removed (e.g. a knight's jump collapses the piece to a knight).",
  r_super3:
    "Each side's pieces must fit the standard starting set: 8 pawns, 2 knights, 2 bishops, 2 rooks, 1 queen, 1 king (a promoted piece counts as a pawn).",
  r_super4:
    "Kinds that become impossible are removed even from unmoved and captured pieces.",
  rs_check: "Check and result",
  r_check1:
    "Check: when only one of your pieces on the board can still be the king, you are in check if that piece is attacked (even if it isn't confirmed as the king).",
  r_check2:
    "A superposed enemy piece attacks a square if any of its possible kinds does.",
  r_check3:
    "You may not leave yourself in check or move into check. If you can't get out of check, you are checkmated and lose.",
  r_check4:
    "If your own move leaves none of your pieces on the board able to be the king (a captured piece turns out to have been your king), you lose on that move (“†”).",
  r_check5:
    "Annihilation: if a single move leaves neither side with a piece on the board that can be the king, the game is a draw (“∅”).",
  rs_castle: "Castling",
  r_castle1:
    "Played with an unmoved king candidate and an unmoved rook candidate on the same rank.",
  r_castle2:
    "There must be no pieces and at least two empty squares between them.",
  r_castle3:
    "The king candidate must not be in check, and the square it passes and its destination must not be attacked.",
  r_castle4:
    "The king moves two squares toward the rook, and the rook moves to the square the king passed. They collapse to a king and a rook.",
  r_castle5:
    "If the move can't be told apart from a rook or queen moving sideways (e.g. e1→g1), you choose which one you mean.",
  rs_pawn: "Pawn moves",
  r_pawn1:
    "An unmoved piece may advance two squares as a pawn from its first rank as well as its second (e.g. b1→b3 for White).",
  r_pawn2:
    "En passant: capturing a piece that just advanced two squares as a pawn confirms both pieces as pawns.",
  r_pawn3:
    "A move into the last rank as a pawn is a promotion. You can also choose to move there as a non-pawn instead.",
  hoverNote:
    "With “Show possible kinds stacked” on, hover over a piece to see its possible kinds arranged in a circle, or hold the right mouse button to see every piece that way.",

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
  annihilation: "Draw by annihilation (neither side has a king left)",
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
  castleAlt: "Normal move (as a rook or queen)",
  whyReach: "Can't move there as any of its possible kinds ({kinds})",
  whyCheck: "Not allowed: your last king candidate on the board ({sq}) would be in check",
  whyCastle: "Can't castle there: the king and rook can't both be confirmed",
  whyGuard: "Blocked by the option: none of your pieces on the board could be the king after this move (you'd lose)",

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
  setup: "初期配置",
  setup0: "両方とも重ね合わせ",
  setup1: "白だけ通常配置",
  setup2: "黒だけ通常配置",
  setup3: "両方とも通常配置",
  start: "対局開始",
  engine: "エンジン",
  strength: "強さ",
  levelOpt: "レベル {n}",
  changeNote: "変更は次の手から反映されます。",
  undo: "待った",
  flip: "盤を反転",
  copy: "棋譜をコピー",
  guard: "自分の盤上の駒がどれもキングでありえなくなる手を指せなくする",
  overlay: "駒のありうる種類を半透明で重ねて表示する",
  copied: "コピーしました",
  recordPrompt: "棋譜",

  rules: "ルール",
  rs_setup: "初期配置",
  r_setup1:
    "すべての駒が、ポーン・ナイト・ビショップ・ルーク・クイーン・キングの重ね合わせです。",
  r_setup2:
    "「初期配置」の設定で、どちらかの色（または両方）を通常のチェスの配置から始めることもできます。",
  rs_super: "動きと重ね合わせ",
  r_super1:
    "駒は、重ね合わせのうちどれか 1 種類として動きます。",
  r_super2:
    "動いた後は、その動きと矛盾する種類が除外されます（例: ナイトの跳び方をしたらナイトに確定）。",
  r_super3:
    "各色の駒は、通常の駒数（ポーン 8・ナイト 2・ビショップ 2・ルーク 2・クイーン 1・キング 1）に割り当てられなければなりません（成った駒はポーンとして数えます）。",
  r_super4:
    "ありえなくなった種類は、動いていない駒や取られた駒からも除外されます。",
  rs_check: "チェックと勝敗",
  r_check1:
    "チェック: 自分の盤上のキング候補が 1 枚だけのとき、その駒が利かされていればチェックです（キングに確定していなくても）。",
  r_check2:
    "相手の駒が重ね合わせのときは、ありうる種類のどれか 1 つでも利いていれば、利かされているとみなします。",
  r_check3:
    "チェックを受けたままにする手や、チェックされにいく手は指せません。チェックを外せなければチェックメイトで負けです。",
  r_check4:
    "自分の手で、自分の盤上の駒のどれもキングでありえなくなったら（＝取られた駒がキングだったと確定したら）、その手を指した時点で負けです（「†」）。",
  r_check5:
    "対消滅: 1 手で両者とも盤上の駒のどれもキングでありえなくなったら、引き分けです（「∅」）。",
  rs_castle: "キャスリング",
  r_castle1:
    "同じ段にある、一度も動いていないキング候補の駒とルーク候補の駒で指せます。",
  r_castle2:
    "間に駒がなく、2 マス以上空いている必要があります。",
  r_castle3:
    "キング候補がチェックされておらず、またぐマスと行き先のマスが利かされていないことが条件です。",
  r_castle4:
    "キングはルークの方へ 2 マス動き、ルークはキングがまたいだマスへ移ります。2 枚はそれぞれキングとルークに確定します。",
  r_castle5:
    "e1→g1 のようにルーク・クイーンとしての横移動と区別できない場合は、どちらとして指すかを選べます。",
  rs_pawn: "ポーンとしての動き",
  r_pawn1:
    "一度も動いていない駒は、2 段目だけでなく 1 段目からもポーンとして 2 マス前進できます（白なら b1→b3 など）。",
  r_pawn2:
    "アンパッサン: 直前にポーンとして 2 マス進んだ駒を取れます。取った駒・取られた駒ともポーンに確定します。",
  r_pawn3:
    "ポーンとして最終段に入る手は成りです。同じマスへ「ポーン以外として動く」手と選べます。",
  hoverNote: "「半透明で重ねて表示」を ON にしているときは、駒にマウスを乗せると、ありうる種類を円形に並べて表示します。右クリックを押している間は、全部の駒をそう表示します。",

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
  annihilation: "対消滅で引き分け（両者ともキングがいなくなりました）",
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
  castleAlt: "通常の移動（ルーク・クイーンとして動く）",
  whyReach: "ありうる種類（{kinds}）のどれとしても、そのマスには動けません",
  whyCheck: "指すと盤上で最後のキング候補（{sq}）がチェックされるため、指せません",
  whyCastle: "キングとルークの両方に確定させられないため、キャスリングできません",
  whyGuard: "指すと自分の盤上の駒がどれもキングでありえなくなる（負けになる）ため、オプションで禁止しています",

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
