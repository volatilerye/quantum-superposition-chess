//! 量子重ね合わせチェス用の簡易エンジン (反復深化 + αβ + 静止探索)。

use crate::quantum::{Game, Move, Piece, Position, Special, K_, OFF};

pub const MATE: i32 = 100_000;

/// P N B R Q K。重ね合わせの駒はありうる種類の平均で評価する
const VALUE: [i32; 6] = [100, 310, 330, 500, 900, 400];

mod clock {
    #[cfg(target_arch = "wasm32")]
    pub fn now() -> f64 {
        #[link(wasm_import_module = "env")]
        extern "C" {
            fn now_ms() -> f64;
        }
        unsafe { now_ms() }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn now() -> f64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() * 1000.0
    }
}

fn piece_value(p: &Piece) -> i32 {
    if p.promoted != 0 {
        return VALUE[p.promoted as usize - 1];
    }
    let d = p.domain;
    let n = d.count_ones() as i32;
    if n == 0 {
        return 0;
    }
    (0..6).filter(|b| d & (1 << b) != 0).map(|b| VALUE[b]).sum::<i32>() / n
}

// ---- 期待駒価値 (駒数制約つきの割り当てで周辺確率を取る) ----------------------------
//
// 種類の平均をそのまま取ると、駒が動いて種類が絞られるだけで評価値が上下する
// (例: 未移動の駒 423 点が縦に長く動いて R/Q に絞られると 700 点になる)。
// そこで「駒数制約 P8 N2 B2 R2 Q1 K1 を満たす割り当てが一様にありうる」として、
// 各駒が各種類である確率 (周辺確率) を求め、期待値で評価する。
// 取られた駒も含めた 16 枚の期待値の合計は常に一定なので、種類が絞られても
// 情報が増えただけで駒得・駒損にはならない。

const COUNT: [usize; 6] = [8, 2, 2, 2, 1, 1];
/// 使った枚数 (P0..8, N0..2, B0..2, R0..2, Q0..1, K0..1) の混合基数表現
#[cfg(test)]
const STRIDE: [usize; 6] = [1, 9, 27, 81, 243, 486];
#[cfg(test)]
const STATES: usize = 972;
#[cfg(test)]
const FULL: usize = 8 + 2 * 9 + 2 * 27 + 2 * 81 + 243 + 486;

/// 状態 s で種類 k をあと 1 枚使えるか
#[cfg(test)]
fn room(s: usize, k: usize) -> bool {
    (s / STRIDE[k]) % (COUNT[k] + 1) < COUNT[k]
}

/// 1 色分の周辺確率から出した値
#[derive(Clone, Copy)]
struct Marg {
    key: u128,
    /// 駒の期待価値 (キングは 0 として)
    value: [i16; 16],
    /// 駒がキングである確率 × 1024
    king: [u16; 16],
}

const MCACHE: usize = 1 << 14;

std::thread_local! {
    static MARG: std::cell::RefCell<Vec<Option<Marg>>> = std::cell::RefCell::new(Vec::new());
}

fn marg_key(pos: &Position, c: u8) -> u128 {
    let mut k = 0u128;
    for i in 0..16 {
        let p = &pos.pieces[c as usize * 16 + i];
        let d = if p.promoted != 0 { 0x40 | p.promoted } else { p.domain };
        k |= (d as u128) << (7 * i);
    }
    k
}

/// 厳密な数え上げ (近似の検証用)。駒を 1 枚ずつ置く前向き・後ろ向きの DP
#[cfg(test)]
fn compute_marg(pos: &Position, c: u8, f: &mut Vec<f64>, b: &mut Vec<f64>) -> Marg {
    let base = c as usize * 16;
    f.clear();
    f.resize(17 * STATES, 0.0);
    b.clear();
    b.resize(17 * STATES, 0.0);
    // 成った駒は元の役割がポーン
    let dom = |i: usize| {
        let p = &pos.pieces[base + i];
        if p.promoted != 0 { 1u8 } else { p.domain }
    };
    f[0] = 1.0;
    for i in 0..16 {
        let d = dom(i);
        for s in 0..STATES {
            let v = f[i * STATES + s];
            if v == 0.0 {
                continue;
            }
            for k in 0..6 {
                if d & (1 << k) != 0 && room(s, k) {
                    f[(i + 1) * STATES + s + STRIDE[k]] += v;
                }
            }
        }
    }
    b[16 * STATES + FULL] = 1.0;
    for i in (0..16).rev() {
        let d = dom(i);
        for s in 0..STATES {
            let mut v = 0.0;
            for k in 0..6 {
                if d & (1 << k) != 0 && room(s, k) {
                    v += b[(i + 1) * STATES + s + STRIDE[k]];
                }
            }
            b[i * STATES + s] = v;
        }
    }
    let total = b[0];
    let mut m = Marg { key: marg_key(pos, c), value: [0; 16], king: [0; 16] };
    for i in 0..16 {
        let p = &pos.pieces[base + i];
        if p.promoted != 0 {
            m.value[i] = VALUE[p.promoted as usize - 1] as i16;
            continue;
        }
        let d = p.domain;
        if total == 0.0 || d.count_ones() == 1 {
            // 割り当てがない (起こらないはず) か確定している
            let k = d.trailing_zeros() as usize;
            if k < 5 {
                m.value[i] = VALUE[k] as i16;
            } else if k == 5 {
                m.king[i] = 1024;
            }
            continue;
        }
        let mut prob = [0.0f64; 6];
        for s in 0..STATES {
            let v = f[i * STATES + s];
            if v == 0.0 {
                continue;
            }
            for k in 0..6 {
                if d & (1 << k) != 0 && room(s, k) {
                    prob[k] += v * b[(i + 1) * STATES + s + STRIDE[k]];
                }
            }
        }
        let ev: f64 = (0..5).map(|k| prob[k] * VALUE[k] as f64).sum::<f64>() / total;
        m.value[i] = ev.round() as i16;
        m.king[i] = (prob[5] / total * 1024.0).round() as u16;
    }
    m
}

/// 周辺確率の近似 (Sinkhorn 法): 行和 1 (各駒はどれか 1 種類)、列和が初期枚数になるよう
/// 交互に正規化する。駒数制約つきの最大エントロピー分布で、厳密な数え上げより桁違いに速い。
/// 列和が初期枚数に一致するので、16 枚の期待値の合計が一定になる性質は保たれる。
fn approx_marg(pos: &Position, c: u8) -> Marg {
    let base = c as usize * 16;
    let mut w = [[0f32; 6]; 16];
    for i in 0..16 {
        let p = &pos.pieces[base + i];
        let d = if p.promoted != 0 { 1u8 } else { p.domain };
        for k in 0..6 {
            if d & (1 << k) != 0 {
                w[i][k] = 1.0;
            }
        }
    }
    for _ in 0..24 {
        for k in 0..6 {
            let col: f32 = (0..16).map(|i| w[i][k]).sum();
            if col > 0.0 {
                let f = COUNT[k] as f32 / col;
                for row in w.iter_mut() {
                    row[k] *= f;
                }
            }
        }
        for row in w.iter_mut() {
            let r: f32 = row.iter().sum();
            if r > 0.0 {
                row.iter_mut().for_each(|x| *x /= r);
            }
        }
    }
    let mut m = Marg { key: marg_key(pos, c), value: [0; 16], king: [0; 16] };
    for i in 0..16 {
        let p = &pos.pieces[base + i];
        if p.promoted != 0 {
            m.value[i] = VALUE[p.promoted as usize - 1] as i16;
            continue;
        }
        let ev: f32 = (0..5).map(|k| w[i][k] * VALUE[k] as f32).sum();
        m.value[i] = ev.round() as i16;
        m.king[i] = (w[i][5] * 1024.0).round() as u16;
    }
    m
}

/// 色 `c` の 16 枚の周辺確率 (キャッシュつき。種類の集合が同じなら盤上の位置によらない)
fn marginals(pos: &Position, c: u8) -> Marg {
    let key = marg_key(pos, c);
    MARG.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.is_empty() {
            cache.resize(MCACHE, None);
        }
        let h = (key ^ key >> 64) as u64;
        let slot = (h.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 50) as usize & (MCACHE - 1);
        if let Some(m) = cache[slot] {
            if m.key == key {
                return m;
            }
        }
        let m = approx_marg(pos, c);
        cache[slot] = Some(m);
        m
    })
}

#[cfg(test)]
fn exact_marg(pos: &Position, c: u8) -> Marg {
    let (mut f, mut b) = (Vec::new(), Vec::new());
    compute_marg(pos, c, &mut f, &mut b)
}

/// 評価関数の設定 (自己対局で比べるため)
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// true なら周辺確率による期待価値、false なら旧来の種類の単純平均
    pub expected: bool,
    /// 盤上のキング候補が n 枚のときの減点 king_loss / n²
    pub king_loss: i32,
    pub qdepth: u32,
}

pub const DEFAULT: Params = Params { expected: true, king_loss: 400, qdepth: 2 };
pub const LEGACY: Params = Params { expected: false, king_loss: 0, qdepth: 2 };

/// 各駒 (32 枚) の価値。期待価値モードなら周辺確率から
fn values(pos: &Position, prm: &Params) -> [i32; 32] {
    let mut v = [0; 32];
    if prm.expected {
        for c in 0..2u8 {
            let m = marginals(pos, c);
            for i in 0..16 {
                v[c as usize * 16 + i] = m.value[i] as i32;
            }
        }
    } else {
        for i in 0..32 {
            v[i] = piece_value(&pos.pieces[i]);
        }
    }
    v
}

/// 手番側から見た評価値
pub fn evaluate(pos: &Position) -> i32 {
    evaluate_with(pos, &DEFAULT)
}

fn evaluate_with(pos: &Position, prm: &Params) -> i32 {
    let mut score = 0;
    if prm.expected {
        for c in 0..2u8 {
            let m = marginals(pos, c);
            let mut s = 0;
            let mut cands = 0;
            for i in 0..16 {
                let p = &pos.pieces[c as usize * 16 + i];
                if p.sq == OFF {
                    continue;
                }
                s += m.value[i] as i32 + center(p.sq) * 3;
                if p.promoted == 0 && p.domain & K_ != 0 {
                    cands += 1;
                }
            }
            // キング候補が残り少ないほど (全部取られたら負けなので) 減点
            if cands > 0 {
                s -= prm.king_loss / (cands * cands);
            }
            score += if c == pos.stm { s } else { -s };
        }
        return score;
    }
    for p in pos.pieces.iter().filter(|p| p.sq != OFF) {
        let v = piece_value(p) + center(p.sq) * 3;
        score += if p.color == pos.stm { v } else { -v };
    }
    score
}

/// 中央寄りを少し評価
fn center(sq: u8) -> i32 {
    let (f, r) = ((sq % 8) as i32, (sq / 8) as i32);
    6 - ((2 * f - 7).abs() + (2 * r - 7).abs()) / 2
}

pub struct SearchResult {
    pub best: Option<Move>,
    pub score: i32,
    pub depth: u32,
    pub nodes: u64,
}

// ---- 置換表 -------------------------------------------------------------------

const TT_SIZE: usize = 1 << 18;
const EXACT: u8 = 0;
const LOWER: u8 = 1; // score 以上 (β カット)
const UPPER: u8 = 2; // score 以下 (α を超えなかった)

#[derive(Clone, Copy, Default)]
struct Entry {
    key: u64,
    score: i32,
    mv: u32,
    depth: i8,
    flag: u8,
}

std::thread_local! {
    // 探索をまたいで使い回す (同じ対局の続きなら前の結果が役に立つ)
    static TT: std::cell::RefCell<Vec<Entry>> = std::cell::RefCell::new(Vec::new());
}

fn pack(m: &Move) -> u32 {
    let sp = match m.special {
        Special::Normal => 0,
        Special::Castle => 1,
        Special::EnPassant => 2,
    };
    // from == to の手はないので 0 は「手なし」に使える
    m.from as u32 | (m.to as u32) << 6 | (m.promo as u32) << 12 | sp << 16
}

/// 詰み (キング喪失) の値は「根からの手数」込みなので、置換表には局面からの手数で入れる
fn to_tt(v: i32, ply: i32) -> i32 {
    if v > MATE - 1000 {
        v + ply
    } else if v < -MATE + 1000 {
        v - ply
    } else {
        v
    }
}
fn from_tt(v: i32, ply: i32) -> i32 {
    if v > MATE - 1000 {
        v - ply
    } else if v < -MATE + 1000 {
        v + ply
    } else {
        v
    }
}

// ---- 探索 ---------------------------------------------------------------------

const MAX_PLY: usize = 128;

struct Searcher<'a> {
    deadline: f64,
    nodes: u64,
    stopped: bool,
    tt: &'a mut [Entry],
    prm: Params,
    killers: [[u32; 2]; MAX_PLY],
    history: [[i32; 64]; 64],
}

impl Searcher<'_> {
    fn eval(&self, pos: &Position) -> i32 {
        evaluate_with(pos, &self.prm)
    }

    fn time_up(&mut self) -> bool {
        self.nodes += 1;
        if self.nodes & 255 == 0 && clock::now() > self.deadline {
            self.stopped = true;
        }
        self.stopped
    }

    fn capture_key(pos: &Position, m: &Move) -> i32 {
        let mut k = 0;
        if let Some(v) = pos.piece_at(m.to) {
            k += 10 * piece_value(&pos.pieces[v]) - piece_value(&pos.pieces[pos.piece_at(m.from).unwrap()]) / 10;
        } else if m.special == Special::EnPassant {
            k += 1000;
        }
        if m.promo != 0 {
            k += 8000;
        }
        k
    }

    /// 並べ替えの優先度: 置換表の手 > 駒取り・成り (MVV-LVA) > キラー手 > ヒストリー
    fn score_moves(&self, pos: &Position, moves: &[Move], tt_move: u32, ply: usize) -> Vec<i32> {
        moves
            .iter()
            .map(|m| {
                let p = pack(m);
                if p == tt_move {
                    return 10_000_000;
                }
                if pos.is_capture(*m) || m.promo != 0 {
                    return 1_000_000 + Self::capture_key(pos, m);
                }
                if ply < MAX_PLY {
                    if self.killers[ply][0] == p {
                        return 900_000;
                    }
                    if self.killers[ply][1] == p {
                        return 800_000;
                    }
                }
                self.history[m.from as usize][m.to as usize]
            })
            .collect()
    }

    /// 残りの手から優先度が最大のものを取り出す (カットが起きれば残りは並べ替えずに済む)
    fn pick(moves: &mut [Move], scores: &mut [i32], i: usize) -> Move {
        let mut b = i;
        for j in i + 1..moves.len() {
            if scores[j] > scores[b] {
                b = j;
            }
        }
        moves.swap(i, b);
        scores.swap(i, b);
        moves[i]
    }

    fn negamax(&mut self, pos: &Position, depth: i32, mut alpha: i32, beta: i32, ply: i32, allow_null: bool) -> i32 {
        if self.time_up() {
            return 0;
        }
        if pos.king_candidates(pos.stm ^ 1) == 0 {
            return MATE - ply; // 直前に指した側が自分のキングの喪失を確定させた
        }
        if pos.king_candidates(pos.stm) == 0 {
            return -MATE + ply;
        }
        if depth <= 0 {
            return self.quiesce(pos, alpha, beta, ply, self.prm.qdepth);
        }
        if pos.halfmove >= 100 {
            return 0;
        }

        let key = pos.hash();
        let slot = key as usize & (TT_SIZE - 1);
        let e = self.tt[slot];
        let mut tt_move = 0;
        if e.key == key {
            tt_move = e.mv;
            if e.depth as i32 >= depth {
                let v = from_tt(e.score, ply);
                let cut = match e.flag {
                    EXACT => true,
                    LOWER => v >= beta,
                    _ => v <= alpha,
                };
                if cut {
                    return v;
                }
            }
        }

        let in_check = pos.in_check();

        // ヌルムーブ: 1 手パスしても β を超えるなら、普通に指せばまず超える
        if allow_null && !in_check && depth >= 3 && beta.abs() < MATE - 1000 && self.eval(pos) >= beta {
            let mut n = pos.clone();
            n.stm ^= 1;
            n.ep = None;
            let r = if depth >= 6 { 3 } else { 2 };
            let v = -self.negamax(&n, depth - 1 - r, -beta, -beta + 1, ply + 1, false);
            if self.stopped {
                return 0;
            }
            if v >= beta {
                return beta;
            }
        }

        // 浅いところでは、評価値に余裕を足しても α に届かない静かな手を読まない (futility)
        let futile = depth <= 2 && !in_check && alpha.abs() < MATE - 1000 && self.eval(pos) + 200 * depth <= alpha;

        let mut moves = pos.pseudo_legal();
        let mut scores = self.score_moves(pos, &moves, tt_move, ply as usize);
        let alpha0 = alpha;
        let mut best = -MATE - 1;
        let mut best_move = 0;
        let mut legal = 0;
        for i in 0..moves.len() {
            let m = Self::pick(&mut moves, &mut scores, i);
            let quiet = !pos.is_capture(m) && m.promo == 0;
            if futile && quiet && legal > 0 {
                continue;
            }
            let Some(child) = pos.play_legal(m) else { continue };
            legal += 1;

            let v = if legal == 1 {
                -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1, true)
            } else {
                // 後ろの方の静かな手は浅く読み (LMR)、α を超えそうなら読み直す (PVS)
                let mut r = 0;
                if depth >= 3 && legal > 3 && quiet && !in_check {
                    r = if legal > 10 { 2 } else { 1 };
                    r = r.min(depth - 2);
                }
                let mut v = -self.negamax(&child, depth - 1 - r, -alpha - 1, -alpha, ply + 1, true);
                if v > alpha && r > 0 {
                    v = -self.negamax(&child, depth - 1, -alpha - 1, -alpha, ply + 1, true);
                }
                if v > alpha && v < beta {
                    v = -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1, true);
                }
                v
            };
            if self.stopped {
                return 0;
            }
            if v > best {
                best = v;
                best_move = pack(&m);
            }
            if v > alpha {
                alpha = v;
            }
            if alpha >= beta {
                if quiet {
                    let p = ply as usize;
                    if p < MAX_PLY && self.killers[p][0] != best_move {
                        self.killers[p][1] = self.killers[p][0];
                        self.killers[p][0] = best_move;
                    }
                    let h = &mut self.history[m.from as usize][m.to as usize];
                    *h = (*h + depth * depth).min(700_000);
                }
                break;
            }
        }
        if legal == 0 {
            return if in_check { -MATE + ply } else { 0 };
        }

        let flag = if best <= alpha0 {
            UPPER
        } else if best >= beta {
            LOWER
        } else {
            EXACT
        };
        if e.key != key || depth >= e.depth as i32 || flag == EXACT {
            self.tt[slot] = Entry { key, score: to_tt(best, ply), mv: best_move, depth: depth as i8, flag };
        }
        best
    }

    fn quiesce(&mut self, pos: &Position, mut alpha: i32, beta: i32, ply: i32, left: u32) -> i32 {
        if self.time_up() {
            return 0;
        }
        if pos.king_candidates(pos.stm ^ 1) == 0 {
            return MATE - ply; // 直前に指した側が自分のキングの喪失を確定させた
        }
        if pos.king_candidates(pos.stm) == 0 {
            return -MATE + ply;
        }
        let stand = self.eval(pos);
        if stand >= beta || left == 0 {
            return stand;
        }
        alpha = alpha.max(stand);
        let vals = values(pos, &self.prm);
        let mut moves = pos.pseudo_captures();
        let mut scores: Vec<i32> = moves.iter().map(|m| Self::capture_key(pos, m)).collect();
        for i in 0..moves.len() {
            let m = Self::pick(&mut moves, &mut scores, i);
            // デルタ枝刈り: 取る駒の価値を足しても α に届かない手は読まない
            let gain = pos.piece_at(m.to).map_or(100, |v| vals[v]) + if m.promo != 0 { 800 } else { 0 };
            if stand + gain + 200 < alpha {
                continue;
            }
            // 損な駒取り: 取る駒より価値の高い駒で、相手に利かされているマスへ取りに行く手は読まない
            if m.promo == 0 {
                let attacker = vals[pos.piece_at(m.from).unwrap()];
                if attacker > gain + 50 && pos.attacked(m.to, pos.stm ^ 1) {
                    continue;
                }
            }
            let Some(child) = pos.play_legal(m) else { continue };
            let v = -self.quiesce(&child, -beta, -alpha, ply + 1, left - 1);
            if self.stopped {
                return 0;
            }
            if v >= beta {
                return v;
            }
            alpha = alpha.max(v);
        }
        alpha
    }
}

/// 置換表を空にする (ベンチマーク用)
pub fn clear_tt() {
    TT.with(|tt| tt.borrow_mut().iter_mut().for_each(|e| *e = Entry::default()));
}

/// 現在の局面を `ms` ミリ秒まで、または深さ `max_depth` 手まで探索する (どちらか先に達した方で止める)
pub fn search(game: &Game, ms: f64, max_depth: u32) -> SearchResult {
    TT.with(|tt| {
        let mut tt = tt.borrow_mut();
        if tt.is_empty() {
            tt.resize(TT_SIZE, Entry::default());
        }
        search_with(game, ms, max_depth, &mut tt, DEFAULT)
    })
}

pub fn search_params(game: &Game, ms: f64, max_depth: u32, prm: Params) -> SearchResult {
    TT.with(|tt| {
        let mut tt = tt.borrow_mut();
        if tt.is_empty() {
            tt.resize(TT_SIZE, Entry::default());
        }
        search_with(game, ms, max_depth, &mut tt, prm)
    })
}

fn search_with(game: &Game, ms: f64, max_depth: u32, tt: &mut [Entry], prm: Params) -> SearchResult {
    let pos = game.pos();
    let mut s = Searcher {
        deadline: clock::now() + ms,
        nodes: 0,
        stopped: false,
        tt,
        prm,
        killers: [[0; 2]; MAX_PLY],
        history: [[0; 64]; 64],
    };
    let mut root = pos.legal_children();
    if root.is_empty() {
        return SearchResult { best: None, score: 0, depth: 0, nodes: 0 };
    }
    root.sort_by_key(|(m, _)| -Searcher::capture_key(pos, m));
    let mut result = SearchResult { best: Some(root[0].0), score: 0, depth: 0, nodes: 0 };

    for depth in 1..=(max_depth.max(1) as i32).min(MAX_PLY as i32 - 8) {
        let mut alpha = -MATE - 1;
        let mut best_i = 0;
        for (i, (_, child)) in root.iter().enumerate() {
            let v = if i == 0 {
                -s.negamax(child, depth - 1, -MATE - 1, MATE + 1, 1, true)
            } else {
                let v = -s.negamax(child, depth - 1, -alpha - 1, -alpha, 1, true);
                if v > alpha && !s.stopped {
                    -s.negamax(child, depth - 1, -MATE - 1, -alpha, 1, true)
                } else {
                    v
                }
            };
            if s.stopped {
                break;
            }
            if v > alpha {
                alpha = v;
                best_i = i;
            }
        }
        if s.stopped {
            break;
        }
        // 最善手を先頭に置いて次の深さへ
        root[..=best_i].rotate_right(1);
        result = SearchResult { best: Some(root[0].0), score: alpha, depth: depth as u32, nodes: s.nodes };
        if alpha.abs() > MATE - 100 {
            break; // 詰み (キング捕獲) が見えたら終了
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_move_quickly() {
        let g = Game::default();
        let t = clock::now();
        let r = search(&g, 300.0, 64);
        assert!(r.best.is_some());
        assert!(r.depth >= 1);
        assert!(clock::now() - t < 2000.0);
    }

    /// cargo test --release bench -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench() {
        let lines: [&[&str]; 3] = [&[], &["e2e4", "e7e5"], &["g1f3", "b8c6", "e2e4", "e7e5", "f1c4"]];
        for ms in [1000.0, 3000.0] {
            for l in lines {
                let mut g = Game::default();
                for m in l {
                    g.play(m).unwrap();
                }
                clear_tt();
                let r = search(&g, ms, 64);
                println!("{ms}ms {:?}: depth {} score {} best {} nodes {}", l, r.depth, r.score, r.best.unwrap().code(), r.nodes);
            }
        }
    }

    #[test]
    fn depth_limit_stops_early() {
        let g = Game::default();
        let t = clock::now();
        let r = search(&g, 10_000.0, 2);
        assert_eq!(r.depth, 2);
        assert!(clock::now() - t < 5000.0);
    }

    #[test]
    fn plays_whole_game_without_panicking() {
        let mut g = Game::default();
        for _ in 0..40 {
            if g.status() != crate::quantum::Status::Ongoing {
                break;
            }
            let r = search(&g, 20.0, 64);
            g.push(r.best.unwrap());
        }
    }

    #[test]
    fn avoids_revealing_own_captured_king() {
        let mut g = Game::default();
        for m in ["e2e4", "b7e4", "d2e4", "a7f2", "f1f2", "f7f2", "g1f2", "f8f5", "f2f5", "e7f5", "c2c7", "a8c7"] {
            g.play(m).unwrap();
        }
        let r = search(&g, 300.0, 64);
        let m = r.best.unwrap();
        assert_ne!(m.code(), "b2b8");
        let mut g2 = Game { history: g.history.clone(), moves: g.moves.clone(), notes: g.notes.clone() };
        g2.push(m);
        assert_ne!(g2.status(), crate::quantum::Status::KingRevealed, "{}", m.code());
    }

    /// 自己対局で 2 つの設定を比べる。
    /// QC_MS=100 QC_GAMES=40 cargo test --release self_play -- --ignored --nocapture
    #[test]
    #[ignore]
    fn self_play() {
        use crate::quantum::Status;
        let env = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let ms = env("QC_MS", 100.0);
        let games = env("QC_GAMES", 40.0) as usize;
        let threads = env("QC_THREADS", 2.0) as usize;
        let depth = env("QC_DEPTH", 64.0) as u32;
        let a = Params {
            expected: env("QC_A_EXP", 1.0) != 0.0,
            king_loss: env("QC_A_KING", DEFAULT.king_loss as f64) as i32,
            qdepth: env("QC_A_Q", DEFAULT.qdepth as f64) as u32,
        };
        let b = Params {
            expected: env("QC_B_EXP", 0.0) != 0.0,
            king_loss: env("QC_B_KING", 0.0) as i32,
            qdepth: env("QC_B_Q", 2.0) as u32,
        };
        println!("A = {a:?}\nB = {b:?}");
        // (A の勝ち, 引き分け, B の勝ち)
        let res = std::sync::Mutex::new((0, 0, 0));
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..threads {
                sc.spawn(|| loop {
                    let g = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if g >= games {
                        break;
                    }
                    // 序盤 4 手はランダム (同じ序盤を先後入れ替えて 2 局)
                    let mut seed = (g / 2) as u64 * 0x9E37_79B9_7F4A_7C15 + 12345;
                    let mut game = Game::default();
                    for _ in 0..4 {
                        let ms = game.pos().legal_moves();
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                        game.push(ms[(seed >> 33) as usize % ms.len()]);
                    }
                    let a_white = g % 2 == 0;
                    let mut plies = 0;
                    let outcome = loop {
                        let st = game.status();
                        if st != Status::Ongoing || plies > 300 {
                            break st;
                        }
                        let white = game.pos().stm == crate::quantum::WHITE;
                        let prm = if white == a_white { a } else { b };
                        clear_tt();
                        let r = search_params(&game, ms, depth, prm);
                        game.push(r.best.unwrap());
                        plies += 1;
                    };
                    // 負けた側: Checkmate / KingCaptured は手番側、KingRevealed は直前に指した側
                    let stm_white = game.pos().stm == crate::quantum::WHITE;
                    let loser_white = match outcome {
                        Status::Checkmate | Status::KingCaptured => Some(stm_white),
                        Status::KingRevealed => Some(!stm_white),
                        _ => None,
                    };
                    let mut r = res.lock().unwrap();
                    match loser_white {
                        Some(lw) if lw != a_white => r.0 += 1,
                        Some(_) => r.2 += 1,
                        None => r.1 += 1,
                    }
                    println!("game {g}: A {} {:?} after {} plies -> {:?}", if a_white { "white" } else { "black" }, outcome, game.moves.len(), *r);
                });
            }
        });
        let r = res.into_inner().unwrap();
        println!("A wins {} / draws {} / B wins {}", r.0, r.1, r.2);
    }

    /// 近似の周辺確率が厳密な数え上げとどれだけずれるか
    #[test]
    fn approx_marginals_close_to_exact() {
        let mut g = Game::default();
        let mut worst = (0i32, 0i32);
        for _ in 0..60 {
            if g.status() != crate::quantum::Status::Ongoing {
                break;
            }
            for c in 0..2u8 {
                let (a, e) = (approx_marg(g.pos(), c), exact_marg(g.pos(), c));
                for i in 0..16 {
                    worst.0 = worst.0.max((a.value[i] as i32 - e.value[i] as i32).abs());
                    worst.1 = worst.1.max((a.king[i] as i32 - e.king[i] as i32).abs());
                }
            }
            let r = search_params(&g, 5.0, 2, DEFAULT);
            g.push(r.best.unwrap());
        }
        println!("worst value diff {} / king diff {}/1024", worst.0, worst.1);
        assert!(worst.0 < 60 && worst.1 < 100, "{worst:?}");
    }
}
