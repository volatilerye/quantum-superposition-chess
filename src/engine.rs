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
}

struct Searcher {
    deadline: f64,
    nodes: u64,
    stopped: bool,
}

impl Searcher {
    fn time_up(&mut self) -> bool {
        self.nodes += 1;
        if self.nodes & 255 == 0 && clock::now() > self.deadline {
            self.stopped = true;
        }
        self.stopped
    }

    fn order(pos: &Position, children: &mut [(Move, Position)]) {
        let key = |m: &Move| -> i32 {
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
        };
        children.sort_by_key(|(m, _)| -key(m));
    }

    fn negamax(&mut self, pos: &Position, depth: u32, mut alpha: i32, beta: i32, ply: i32) -> i32 {
        if self.time_up() {
            return 0;
        }
        if pos.king_candidates(pos.stm ^ 1) == 0 {
            return MATE - ply; // 直前に指した側が自分のキングの喪失を確定させた
        }
        if pos.king_candidates(pos.stm) == 0 {
            return -MATE + ply;
        }
        if depth == 0 {
            return self.quiesce(pos, alpha, beta, ply, 4);
        }
        let mut children = pos.legal_children();
        if children.is_empty() {
            return if pos.in_check() { -MATE + ply } else { 0 };
        }
        if pos.halfmove >= 100 {
            return 0;
        }
        Self::order(pos, &mut children);
        let mut best = -MATE;
        for (_, child) in &children {
            let v = -self.negamax(child, depth - 1, -beta, -alpha, ply + 1);
            if self.stopped {
                return 0;
            }
            if v > best {
                best = v;
            }
            if v > alpha {
                alpha = v;
            }
            if alpha >= beta {
                break;
            }
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
        let mut children = pos.legal_children_where(|m| pos.is_capture(*m) || m.promo != 0);
        Self::order(pos, &mut children);
        for (_, child) in &children {
            let v = -self.quiesce(child, -beta, -alpha, ply + 1, left - 1);
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

/// 現在の局面を `ms` ミリ秒まで探索する
pub fn search(game: &Game, ms: f64) -> SearchResult {
    let pos = game.pos();
    let mut s = Searcher { deadline: clock::now() + ms, nodes: 0, stopped: false };
    let mut root = pos.legal_children();
    if root.is_empty() {
        return SearchResult { best: None, score: 0, depth: 0 };
    }
    Searcher::order(pos, &mut root);
    let mut result = SearchResult { best: Some(root[0].0), score: 0, depth: 0 };

    for depth in 1..=64 {
        let mut alpha = -MATE - 1;
        let mut best_i = 0;
        for (i, (_, child)) in root.iter().enumerate() {
            let v = -s.negamax(child, depth - 1, -MATE - 1, -alpha, 1);
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
        result = SearchResult { best: Some(root[0].0), score: alpha, depth };
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
        let r = search(&g, 300.0);
        assert!(r.best.is_some());
        assert!(r.depth >= 1);
        assert!(clock::now() - t < 2000.0);
    }

    #[test]
    fn plays_whole_game_without_panicking() {
        let mut g = Game::default();
        for _ in 0..40 {
            if g.status() != crate::quantum::Status::Ongoing {
                break;
            }
            let r = search(&g, 20.0);
            g.push(r.best.unwrap());
        }
    }

    #[test]
    fn avoids_revealing_own_captured_king() {
        let mut g = Game::default();
        for m in ["e2e4", "b7e4", "d2e4", "a7f2", "f1f2", "f7f2", "g1f2", "f8f5", "f2f5", "e7f5", "c2c7", "a8c7"] {
            g.play(m).unwrap();
        }
        let r = search(&g, 300.0);
        let m = r.best.unwrap();
        assert_ne!(m.code(), "b2b8");
        let mut g2 = Game { history: g.history.clone(), moves: g.moves.clone(), notes: g.notes.clone() };
        g2.push(m);
        assert_ne!(g2.status(), crate::quantum::Status::KingRevealed, "{}", m.code());
    }
}
