//! 依存ライブラリなしのチェスルール実装。
//!
//! マス番号は a1 = 0, b1 = 1, ..., h8 = 63。
//! 駒は下位 3bit が種類 (1..=6)、bit3 が色 (0 = 白, 8 = 黒)。

pub const EMPTY: u8 = 0;
pub const PAWN: u8 = 1;
pub const KNIGHT: u8 = 2;
pub const BISHOP: u8 = 3;
pub const ROOK: u8 = 4;
pub const QUEEN: u8 = 5;
pub const KING: u8 = 6;
pub const BLACK: u8 = 8;

pub const WK: u8 = 1;
pub const WQ: u8 = 2;
pub const BK: u8 = 4;
pub const BQ: u8 = 8;

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[inline]
pub fn kind(p: u8) -> u8 {
    p & 7
}
#[inline]
pub fn color(p: u8) -> u8 {
    p & 8
}
#[inline]
pub fn file(sq: u8) -> i8 {
    (sq % 8) as i8
}
#[inline]
pub fn rank(sq: u8) -> i8 {
    (sq / 8) as i8
}
#[inline]
pub fn at(f: i8, r: i8) -> Option<u8> {
    if (0..8).contains(&f) && (0..8).contains(&r) {
        Some((r * 8 + f) as u8)
    } else {
        None
    }
}

const KNIGHT_D: [(i8, i8); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
const KING_D: [(i8, i8); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
const ROOK_D: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP_D: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    /// 成る駒の種類 (KNIGHT..=QUEEN)、成らなければ 0
    pub promo: u8,
}

pub fn sq_name(sq: u8) -> String {
    let mut s = String::with_capacity(2);
    s.push((b'a' + sq % 8) as char);
    s.push((b'1' + sq / 8) as char);
    s
}

pub fn parse_sq(s: &[u8]) -> Option<u8> {
    if s.len() != 2 {
        return None;
    }
    let f = s[0].wrapping_sub(b'a');
    let r = s[1].wrapping_sub(b'1');
    if f < 8 && r < 8 {
        Some(r * 8 + f)
    } else {
        None
    }
}

impl Move {
    pub fn uci(&self) -> String {
        let mut s = sq_name(self.from);
        s.push_str(&sq_name(self.to));
        if self.promo != 0 {
            s.push(match self.promo {
                KNIGHT => 'n',
                BISHOP => 'b',
                ROOK => 'r',
                _ => 'q',
            });
        }
        s
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Position {
    pub board: [u8; 64],
    /// 0 = 白番, 8 = 黒番
    pub stm: u8,
    pub castling: u8,
    pub ep: Option<u8>,
    pub halfmove: u32,
    pub fullmove: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ongoing = 0,
    Checkmate = 1,
    Stalemate = 2,
    FiftyMove = 3,
    Threefold = 4,
    Insufficient = 5,
}

fn piece_char(p: u8) -> char {
    let c = match kind(p) {
        PAWN => 'p',
        KNIGHT => 'n',
        BISHOP => 'b',
        ROOK => 'r',
        QUEEN => 'q',
        KING => 'k',
        _ => '.',
    };
    if color(p) == 0 {
        c.to_ascii_uppercase()
    } else {
        c
    }
}

impl Position {
    pub fn start() -> Self {
        Self::from_fen(START_FEN).expect("valid start fen")
    }

    pub fn from_fen(fen: &str) -> Option<Self> {
        let mut parts = fen.split_whitespace();
        let placement = parts.next()?;
        let stm = parts.next().unwrap_or("w");
        let cast = parts.next().unwrap_or("-");
        let ep = parts.next().unwrap_or("-");
        let half = parts.next().unwrap_or("0");
        let full = parts.next().unwrap_or("1");

        let mut board = [EMPTY; 64];
        let rows: Vec<&str> = placement.split('/').collect();
        if rows.len() != 8 {
            return None;
        }
        for (i, row) in rows.iter().enumerate() {
            let r = 7 - i as i8;
            let mut f: i8 = 0;
            for ch in row.chars() {
                if let Some(d) = ch.to_digit(10) {
                    f += d as i8;
                } else {
                    let k = match ch.to_ascii_lowercase() {
                        'p' => PAWN,
                        'n' => KNIGHT,
                        'b' => BISHOP,
                        'r' => ROOK,
                        'q' => QUEEN,
                        'k' => KING,
                        _ => return None,
                    };
                    let c = if ch.is_ascii_uppercase() { 0 } else { BLACK };
                    let sq = at(f, r)?;
                    board[sq as usize] = k | c;
                    f += 1;
                }
                if f > 8 {
                    return None;
                }
            }
            if f != 8 {
                return None;
            }
        }
        // 各色キングはちょうど 1 枚
        for c in [0, BLACK] {
            if board.iter().filter(|&&p| p == KING | c).count() != 1 {
                return None;
            }
        }

        let stm = match stm {
            "w" => 0,
            "b" => BLACK,
            _ => return None,
        };
        let mut castling = 0;
        if cast != "-" {
            for ch in cast.chars() {
                castling |= match ch {
                    'K' => WK,
                    'Q' => WQ,
                    'k' => BK,
                    'q' => BQ,
                    _ => return None,
                };
            }
        }
        // 駒の配置と矛盾するキャスリング権は落とす
        if board[4] != KING || board[7] != ROOK {
            castling &= !WK;
        }
        if board[4] != KING || board[0] != ROOK {
            castling &= !WQ;
        }
        if board[60] != KING | BLACK || board[63] != ROOK | BLACK {
            castling &= !BK;
        }
        if board[60] != KING | BLACK || board[56] != ROOK | BLACK {
            castling &= !BQ;
        }
        let ep = if ep == "-" { None } else { Some(parse_sq(ep.as_bytes())?) };

        let pos = Position {
            board,
            stm,
            castling,
            ep,
            halfmove: half.parse().ok()?,
            fullmove: full.parse::<u32>().ok()?.max(1),
        };
        // 手番でない側のキングに王手がかかっている局面は不正
        if pos.attacked(pos.king_sq(pos.stm ^ BLACK), pos.stm) {
            return None;
        }
        Some(pos)
    }

    pub fn to_fen(&self) -> String {
        let mut s = String::new();
        for r in (0..8).rev() {
            let mut empty = 0;
            for f in 0..8 {
                let p = self.board[r * 8 + f];
                if p == EMPTY {
                    empty += 1;
                } else {
                    if empty > 0 {
                        s.push((b'0' + empty) as char);
                        empty = 0;
                    }
                    s.push(piece_char(p));
                }
            }
            if empty > 0 {
                s.push((b'0' + empty) as char);
            }
            if r > 0 {
                s.push('/');
            }
        }
        s.push(' ');
        s.push(if self.stm == 0 { 'w' } else { 'b' });
        s.push(' ');
        if self.castling == 0 {
            s.push('-');
        } else {
            for (bit, ch) in [(WK, 'K'), (WQ, 'Q'), (BK, 'k'), (BQ, 'q')] {
                if self.castling & bit != 0 {
                    s.push(ch);
                }
            }
        }
        s.push(' ');
        match self.ep {
            Some(sq) => s.push_str(&sq_name(sq)),
            None => s.push('-'),
        }
        s.push_str(&format!(" {} {}", self.halfmove, self.fullmove));
        s
    }

    /// 64 文字の盤面 (a1..h8 の順、空きは '.')
    pub fn board_string(&self) -> String {
        self.board.iter().map(|&p| piece_char(p)).collect()
    }

    pub fn king_sq(&self, c: u8) -> u8 {
        self.board.iter().position(|&p| p == KING | c).map(|i| i as u8).unwrap_or(0)
    }

    /// マス `sq` が色 `by` の駒に利かされているか
    pub fn attacked(&self, sq: u8, by: u8) -> bool {
        let (f, r) = (file(sq), rank(sq));
        // ポーン: 白ポーンは下から上へ利く
        let pr = if by == 0 { r - 1 } else { r + 1 };
        for df in [-1, 1] {
            if let Some(s) = at(f + df, pr) {
                if self.board[s as usize] == PAWN | by {
                    return true;
                }
            }
        }
        for (df, dr) in KNIGHT_D {
            if let Some(s) = at(f + df, r + dr) {
                if self.board[s as usize] == KNIGHT | by {
                    return true;
                }
            }
        }
        for (df, dr) in KING_D {
            if let Some(s) = at(f + df, r + dr) {
                if self.board[s as usize] == KING | by {
                    return true;
                }
            }
        }
        for (dirs, k) in [(ROOK_D, ROOK), (BISHOP_D, BISHOP)] {
            for (df, dr) in dirs {
                let (mut cf, mut cr) = (f + df, r + dr);
                while let Some(s) = at(cf, cr) {
                    let p = self.board[s as usize];
                    if p != EMPTY {
                        if color(p) == by && (kind(p) == k || kind(p) == QUEEN) {
                            return true;
                        }
                        break;
                    }
                    cf += df;
                    cr += dr;
                }
            }
        }
        false
    }

    pub fn in_check(&self) -> bool {
        self.attacked(self.king_sq(self.stm), self.stm ^ BLACK)
    }

    fn pseudo_moves(&self, out: &mut Vec<Move>) {
        let us = self.stm;
        let them = us ^ BLACK;
        for from in 0..64u8 {
            let p = self.board[from as usize];
            if p == EMPTY || color(p) != us {
                continue;
            }
            let (f, r) = (file(from), rank(from));
            match kind(p) {
                PAWN => {
                    let dir: i8 = if us == 0 { 1 } else { -1 };
                    let start_rank = if us == 0 { 1 } else { 6 };
                    let last_rank = if us == 0 { 7 } else { 0 };
                    let push = |to: u8, out: &mut Vec<Move>| {
                        if rank(to) == last_rank {
                            for pr in [QUEEN, ROOK, BISHOP, KNIGHT] {
                                out.push(Move { from, to, promo: pr });
                            }
                        } else {
                            out.push(Move { from, to, promo: 0 });
                        }
                    };
                    if let Some(to) = at(f, r + dir) {
                        if self.board[to as usize] == EMPTY {
                            push(to, out);
                            if r == start_rank {
                                let to2 = at(f, r + 2 * dir).unwrap();
                                if self.board[to2 as usize] == EMPTY {
                                    out.push(Move { from, to: to2, promo: 0 });
                                }
                            }
                        }
                    }
                    for df in [-1, 1] {
                        if let Some(to) = at(f + df, r + dir) {
                            let t = self.board[to as usize];
                            if (t != EMPTY && color(t) == them) || self.ep == Some(to) {
                                push(to, out);
                            }
                        }
                    }
                }
                KNIGHT | KING => {
                    let ds = if kind(p) == KNIGHT { &KNIGHT_D } else { &KING_D };
                    for &(df, dr) in ds {
                        if let Some(to) = at(f + df, r + dr) {
                            let t = self.board[to as usize];
                            if t == EMPTY || color(t) == them {
                                out.push(Move { from, to, promo: 0 });
                            }
                        }
                    }
                }
                k => {
                    let dirs: &[(i8, i8)] = match k {
                        BISHOP => &BISHOP_D,
                        ROOK => &ROOK_D,
                        _ => &KING_D, // クイーン = 8 方向
                    };
                    for &(df, dr) in dirs {
                        let (mut cf, mut cr) = (f + df, r + dr);
                        while let Some(to) = at(cf, cr) {
                            let t = self.board[to as usize];
                            if t == EMPTY {
                                out.push(Move { from, to, promo: 0 });
                            } else {
                                if color(t) == them {
                                    out.push(Move { from, to, promo: 0 });
                                }
                                break;
                            }
                            cf += df;
                            cr += dr;
                        }
                    }
                }
            }
        }
        // キャスリング
        let (rights_k, rights_q, base) = if us == 0 { (WK, WQ, 0u8) } else { (BK, BQ, 56u8) };
        let king = base + 4;
        if self.board[king as usize] == KING | us && self.castling & (rights_k | rights_q) != 0 && !self.attacked(king, them) {
            let empty = |s: u8| self.board[s as usize] == EMPTY;
            if self.castling & rights_k != 0
                && self.board[(base + 7) as usize] == ROOK | us
                && empty(base + 5)
                && empty(base + 6)
                && !self.attacked(base + 5, them)
                && !self.attacked(base + 6, them)
            {
                out.push(Move { from: king, to: base + 6, promo: 0 });
            }
            if self.castling & rights_q != 0
                && self.board[base as usize] == ROOK | us
                && empty(base + 1)
                && empty(base + 2)
                && empty(base + 3)
                && !self.attacked(base + 3, them)
                && !self.attacked(base + 2, them)
            {
                out.push(Move { from: king, to: base + 2, promo: 0 });
            }
        }
    }

    pub fn is_capture(&self, m: Move) -> bool {
        self.board[m.to as usize] != EMPTY
            || (kind(self.board[m.from as usize]) == PAWN && Some(m.to) == self.ep)
    }

    /// 合法性チェックなしで指す (pseudo-legal な手を前提)
    pub fn apply(&self, m: Move) -> Position {
        let mut n = self.clone();
        let p = n.board[m.from as usize];
        let us = self.stm;
        let capture = self.is_capture(m);
        n.board[m.from as usize] = EMPTY;

        if kind(p) == PAWN && Some(m.to) == self.ep {
            // アンパッサン: 取られるポーンは to の後ろ
            let cap = if us == 0 { m.to - 8 } else { m.to + 8 };
            n.board[cap as usize] = EMPTY;
        }
        n.board[m.to as usize] = if m.promo != 0 { m.promo | us } else { p };

        if kind(p) == KING && (m.to as i8 - m.from as i8).abs() == 2 {
            let (rf, rt) = if m.to > m.from { (m.from + 3, m.from + 1) } else { (m.from - 4, m.from - 1) };
            n.board[rt as usize] = n.board[rf as usize];
            n.board[rf as usize] = EMPTY;
        }

        for sq in [m.from, m.to] {
            n.castling &= match sq {
                0 => !WQ,
                7 => !WK,
                4 => !(WK | WQ),
                56 => !BQ,
                63 => !BK,
                60 => !(BK | BQ),
                _ => 0xff,
            };
        }

        n.ep = None;
        if kind(p) == PAWN && (m.to as i8 - m.from as i8).abs() == 16 {
            let ep_sq = (m.from + m.to) / 2;
            // 実際に取れる相手ポーンがいるときだけ ep を立てる (繰り返し判定のため)
            let them = us ^ BLACK;
            let (f, r) = (file(m.to), rank(m.to));
            let can = [-1i8, 1].iter().any(|&df| at(f + df, r).map(|s| n.board[s as usize] == PAWN | them).unwrap_or(false));
            if can {
                n.ep = Some(ep_sq);
            }
        }

        n.halfmove = if kind(p) == PAWN || capture { 0 } else { self.halfmove + 1 };
        if us == BLACK {
            n.fullmove += 1;
        }
        n.stm = us ^ BLACK;
        n
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        let mut pseudo = Vec::with_capacity(64);
        self.pseudo_moves(&mut pseudo);
        let us = self.stm;
        pseudo
            .into_iter()
            .filter(|&m| {
                let n = self.apply(m);
                !n.attacked(n.king_sq(us), us ^ BLACK)
            })
            .collect()
    }

    pub fn parse_uci(&self, s: &str) -> Option<Move> {
        let b = s.trim().as_bytes();
        if b.len() < 4 || b.len() > 5 {
            return None;
        }
        let from = parse_sq(&b[0..2])?;
        let to = parse_sq(&b[2..4])?;
        let promo = if b.len() == 5 {
            match b[4] {
                b'n' => KNIGHT,
                b'b' => BISHOP,
                b'r' => ROOK,
                b'q' => QUEEN,
                _ => return None,
            }
        } else {
            0
        };
        let m = Move { from, to, promo };
        self.legal_moves().into_iter().find(|&x| x == m)
    }

    pub fn san(&self, m: Move) -> String {
        let p = self.board[m.from as usize];
        let k = kind(p);
        let mut s = String::new();
        if k == KING && (m.to as i8 - m.from as i8).abs() == 2 {
            s.push_str(if m.to > m.from { "O-O" } else { "O-O-O" });
        } else {
            let capture = self.is_capture(m);
            if k == PAWN {
                if capture {
                    s.push((b'a' + m.from % 8) as char);
                }
            } else {
                s.push(piece_char(k).to_ascii_uppercase());
                let others: Vec<Move> = self
                    .legal_moves()
                    .into_iter()
                    .filter(|o| o.to == m.to && o.from != m.from && self.board[o.from as usize] == p)
                    .collect();
                if !others.is_empty() {
                    let same_file = others.iter().any(|o| o.from % 8 == m.from % 8);
                    let same_rank = others.iter().any(|o| o.from / 8 == m.from / 8);
                    if !same_file {
                        s.push((b'a' + m.from % 8) as char);
                    } else if !same_rank {
                        s.push((b'1' + m.from / 8) as char);
                    } else {
                        s.push_str(&sq_name(m.from));
                    }
                }
            }
            if capture {
                s.push('x');
            }
            s.push_str(&sq_name(m.to));
            if m.promo != 0 {
                s.push('=');
                s.push(piece_char(m.promo));
            }
        }
        let n = self.apply(m);
        if n.in_check() {
            s.push(if n.legal_moves().is_empty() { '#' } else { '+' });
        }
        s
    }

    pub fn insufficient_material(&self) -> bool {
        let mut minors = Vec::new();
        for (sq, &p) in self.board.iter().enumerate() {
            match kind(p) {
                EMPTY | KING => {}
                KNIGHT | BISHOP => minors.push((kind(p), color(p), (sq % 8 + sq / 8) % 2)),
                _ => return false,
            }
        }
        match minors.len() {
            0 | 1 => true,
            _ => {
                // 全部ビショップで同じ色マスなら駒不足
                minors.iter().all(|m| m.0 == BISHOP) && minors.iter().all(|m| m.2 == minors[0].2)
            }
        }
    }

    /// 繰り返し判定用のキー (手数カウンタを除いた FEN 相当)
    fn rep_key(&self) -> ([u8; 64], u8, u8, Option<u8>) {
        (self.board, self.stm, self.castling, self.ep)
    }
}

/// 対局全体 (履歴つき)
pub struct Game {
    pub initial: Position,
    pub history: Vec<Position>,
    pub moves: Vec<Move>,
    pub sans: Vec<String>,
}

impl Default for Game {
    fn default() -> Self {
        Self::new(Position::start())
    }
}

impl Game {
    pub fn new(pos: Position) -> Self {
        Game { initial: pos.clone(), history: vec![pos], moves: vec![], sans: vec![] }
    }

    pub fn pos(&self) -> &Position {
        self.history.last().unwrap()
    }

    /// UCI 形式で指す。成功時は SAN を返す
    pub fn play_uci(&mut self, s: &str) -> Option<String> {
        if self.status() != Status::Ongoing {
            return None;
        }
        let m = self.pos().parse_uci(s)?;
        let san = self.pos().san(m);
        let next = self.pos().apply(m);
        self.history.push(next);
        self.moves.push(m);
        self.sans.push(san.clone());
        Some(san)
    }

    pub fn undo(&mut self) -> bool {
        if self.moves.is_empty() {
            return false;
        }
        self.history.pop();
        self.moves.pop();
        self.sans.pop();
        true
    }

    pub fn status(&self) -> Status {
        let pos = self.pos();
        if pos.legal_moves().is_empty() {
            return if pos.in_check() { Status::Checkmate } else { Status::Stalemate };
        }
        if pos.insufficient_material() {
            return Status::Insufficient;
        }
        if pos.halfmove >= 100 {
            return Status::FiftyMove;
        }
        let key = pos.rep_key();
        let n = pos.halfmove as usize;
        let reps = self.history.iter().rev().take(n + 1).filter(|p| p.rep_key() == key).count();
        if reps >= 3 {
            return Status::Threefold;
        }
        Status::Ongoing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perft(p: &Position, d: u32) -> u64 {
        if d == 0 {
            return 1;
        }
        let ms = p.legal_moves();
        if d == 1 {
            return ms.len() as u64;
        }
        ms.iter().map(|&m| perft(&p.apply(m), d - 1)).sum()
    }

    fn check(fen: &str, d: u32, n: u64) {
        let p = Position::from_fen(fen).unwrap();
        assert_eq!(perft(&p, d), n, "perft({fen}, {d})");
    }

    #[test]
    fn perft_start() {
        check(START_FEN, 4, 197_281);
    }
    #[test]
    fn perft_kiwipete() {
        check("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 3, 97_862);
    }
    #[test]
    fn perft_pos3() {
        check("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 5, 674_624);
    }
    #[test]
    fn perft_pos4() {
        check("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 4, 422_333);
    }
    #[test]
    fn perft_pos5() {
        check("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 3, 62_379);
    }

    #[test]
    fn fen_roundtrip() {
        let f = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        assert_eq!(Position::from_fen(f).unwrap().to_fen(), f);
    }

    #[test]
    fn san_and_mate() {
        let mut g = Game::default();
        for (u, s) in [("f2f3", "f3"), ("e7e5", "e5"), ("g2g4", "g4"), ("d8h4", "Qh4#")] {
            assert_eq!(g.play_uci(u).as_deref(), Some(s));
        }
        assert_eq!(g.status(), Status::Checkmate);
        assert!(g.play_uci("a2a3").is_none());
    }

    #[test]
    fn san_disambiguation_and_castle() {
        let p = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        assert_eq!(p.san(p.parse_uci("e1g1").unwrap()), "O-O");
        assert_eq!(p.san(p.parse_uci("a1d1").unwrap()), "Rd1");
        let p = Position::from_fen("4k3/8/8/8/8/8/4K3/R6R w - - 0 1").unwrap();
        assert_eq!(p.san(p.parse_uci("a1d1").unwrap()), "Rad1");
        let p = Position::from_fen("4k3/8/N7/8/8/8/N7/4K3 w - - 0 1").unwrap();
        assert_eq!(p.san(p.parse_uci("a2b4").unwrap()), "N2b4");
        let p = Position::from_fen("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert_eq!(p.san(p.parse_uci("b7b8q").unwrap()), "b8=Q+");
    }

    #[test]
    fn threefold() {
        let mut g = Game::default();
        for _ in 0..2 {
            for u in ["g1f3", "g8f6", "f3g1", "f6g8"] {
                g.play_uci(u).unwrap();
            }
        }
        assert_eq!(g.status(), Status::Threefold);
    }

    #[test]
    fn insufficient() {
        let p = Position::from_fen("4k3/8/8/8/8/8/8/4KB2 w - - 0 1").unwrap();
        assert!(p.insufficient_material());
        let p = Position::from_fen("4k3/8/8/8/8/8/8/4KN1N w - - 0 1").unwrap();
        assert!(!p.insufficient_material());
    }
}
