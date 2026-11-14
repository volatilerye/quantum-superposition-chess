//! 量子重ね合わせチェス用の簡易エンジン (反復深化 + αβ + 静止探索)。

use crate::quantum::{Game, Move, Piece, Position, Special, OFF};

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

/// 手番側から見た評価値
pub fn evaluate(pos: &Position) -> i32 {
    let mut score = 0;
    for p in pos.pieces.iter().filter(|p| p.sq != OFF) {
        let (f, r) = ((p.sq % 8) as i32, (p.sq / 8) as i32);
        // 中央寄りを少し評価
        let center = 6 - ((2 * f - 7).abs() + (2 * r - 7).abs()) / 2;
        let v = piece_value(p) + center * 3;
        score += if p.color == pos.stm { v } else { -v };
    }
    score
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
/// 静止探索で読む駒取りの手数 (このゲームは遠くの駒も取れて取り合いが長いので、深く読むより本探索を深くする)
const QDEPTH: u32 = 2;

struct Searcher<'a> {
    deadline: f64,
    nodes: u64,
    stopped: bool,
    tt: &'a mut [Entry],
    qdepth: u32,
    killers: [[u32; 2]; MAX_PLY],
    history: [[i32; 64]; 64],
}

impl Searcher<'_> {
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
            return self.quiesce(pos, alpha, beta, ply, self.qdepth);
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
        if allow_null && !in_check && depth >= 3 && beta.abs() < MATE - 1000 && evaluate(pos) >= beta {
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
        let futile = depth <= 2 && !in_check && alpha.abs() < MATE - 1000 && evaluate(pos) + 200 * depth <= alpha;

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
        let stand = evaluate(pos);
        if stand >= beta || left == 0 {
            return stand;
        }
        alpha = alpha.max(stand);
        let mut moves = pos.pseudo_captures();
        let mut scores: Vec<i32> = moves.iter().map(|m| Self::capture_key(pos, m)).collect();
        for i in 0..moves.len() {
            let m = Self::pick(&mut moves, &mut scores, i);
            // デルタ枝刈り: 取る駒の価値を足しても α に届かない手は読まない
            let gain = pos.piece_at(m.to).map_or(100, |v| piece_value(&pos.pieces[v])) + if m.promo != 0 { 800 } else { 0 };
            if stand + gain + 200 < alpha {
                continue;
            }
            // 損な駒取り: 取る駒より価値の高い駒で、相手に利かされているマスへ取りに行く手は読まない
            if m.promo == 0 {
                let attacker = piece_value(&pos.pieces[pos.piece_at(m.from).unwrap()]);
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
        search_with(game, ms, max_depth, &mut tt, QDEPTH)
    })
}

fn search_with(game: &Game, ms: f64, max_depth: u32, tt: &mut [Entry], qdepth: u32) -> SearchResult {
    let pos = game.pos();
    let mut s = Searcher {
        deadline: clock::now() + ms,
        nodes: 0,
        stopped: false,
        tt,
        qdepth,
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
}

