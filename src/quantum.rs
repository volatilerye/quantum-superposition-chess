//! 量子重ね合わせチェスのルール。
//!
//! 通常のチェスとの違い:
//! - 初期配置の 32 枚はすべて「ポーン・ナイト・ビショップ・ルーク・クイーン・キング」の重ね合わせ
//! - 駒は重ね合わせのどれか 1 種類として動く。動いた後は、その動きと矛盾する種類が除外される
//! - 各色の駒は初期配置の駒数 (P8 N2 B2 R2 Q1 K1) に割り当てられなければならず、
//!   どの割り当てでもありえない種類は (動いていない駒・取られた駒からも) 除外される
//! - チェックはキングに確定した駒にだけ適用する。キングが未確定の間は、
//!   相手の生きている駒のどれもキングでありえなくなったら (= キングを取ったら) 勝ち
//!
//! 種類の集合は 6bit のマスクで表す (bit `kind - 1`、P N B R Q K の順)。

use crate::chess::{at, file, parse_sq, rank, sq_name, BISHOP, KING, KNIGHT, PAWN, QUEEN, ROOK};

pub const WHITE: u8 = 0;
pub const BLACK: u8 = 1;

/// 種類 (PAWN..=KING) のビット
pub const fn bit(kind: u8) -> u8 {
    1 << (kind - 1)
}
pub const P_: u8 = bit(PAWN);
pub const N_: u8 = bit(KNIGHT);
pub const B_: u8 = bit(BISHOP);
pub const R_: u8 = bit(ROOK);
pub const Q_: u8 = bit(QUEEN);
pub const K_: u8 = bit(KING);
pub const ALL: u8 = 0b11_1111;

/// 各色の初期配置の駒数 (P N B R Q K)
const INITIAL_COUNT: [u8; 6] = [8, 2, 2, 2, 1, 1];

/// 種類の集合 S (6 ビット) に割り当てられる駒の数 (S に含まれる種類の初期枚数の和)
const CAPACITY: [u8; 64] = {
    let mut t = [0u8; 64];
    let mut s = 0;
    while s < 64 {
        let mut b = 0;
        while b < 6 {
            if s & (1 << b) != 0 {
                t[s] += INITIAL_COUNT[b];
            }
            b += 1;
        }
        s += 1;
    }
    t
};
const LETTERS: [u8; 6] = *b"PNBRQK";

/// 盤外 (取られた駒) を表すマス
pub const OFF: u8 = 64;

const KNIGHT_D: [(i8, i8); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
const KING_D: [(i8, i8); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
const ROOK_D: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP_D: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

/// 種類マスクを "PNB" のような文字列にする
pub fn letters(mask: u8) -> String {
    (0..6).filter(|i| mask & (1 << i) != 0).map(|i| LETTERS[i] as char).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub color: u8,
    /// 0..64、取られていれば OFF
    pub sq: u8,
    /// 初期配置での役割としてありうる種類 (成った駒は P のみ)
    pub domain: u8,
    /// 成った後の種類 (成っていなければ 0)
    pub promoted: u8,
    pub moved: bool,
}

impl Piece {
    pub fn alive(&self) -> bool {
        self.sq != OFF
    }
    /// 今この駒が動ける種類
    pub fn kinds(&self) -> u8 {
        if self.promoted != 0 {
            bit(self.promoted)
        } else {
            self.domain
        }
    }
    pub fn is_definite_king(&self) -> bool {
        self.alive() && self.promoted == 0 && self.domain == K_
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    Normal,
    Castle,
    EnPassant,
}

#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    /// 成る種類 (成らなければ 0)。成る手はポーンとしての手
    pub promo: u8,
    pub special: Special,
    /// この手と矛盾しない種類
    pub mask: u8,
}

impl PartialEq for Move {
    fn eq(&self, o: &Self) -> bool {
        self.from == o.from && self.to == o.to && self.promo == o.promo && self.special == o.special
    }
}

impl Move {
    /// "e2e4" / 成り "e7e8q" / キャスリング "e1g1c" / アンパッサン "e5d6e"
    pub fn code(&self) -> String {
        let mut s = sq_name(self.from);
        s.push_str(&sq_name(self.to));
        match self.special {
            Special::Castle => s.push('c'),
            Special::EnPassant => s.push('e'),
            Special::Normal => {
                if self.promo != 0 {
                    s.push(LETTERS[self.promo as usize - 1].to_ascii_lowercase() as char);
                }
            }
        }
        s
    }

    fn parse(s: &str) -> Option<(u8, u8, u8, Special)> {
        let b = s.trim().as_bytes();
        if b.len() < 4 || b.len() > 5 {
            return None;
        }
        let from = parse_sq(&b[0..2])?;
        let to = parse_sq(&b[2..4])?;
        let (promo, special) = match b.get(4) {
            None => (0, Special::Normal),
            Some(b'c') => (0, Special::Castle),
            Some(b'e') => (0, Special::EnPassant),
            Some(b'n') => (KNIGHT, Special::Normal),
            Some(b'b') => (BISHOP, Special::Normal),
            Some(b'r') => (ROOK, Special::Normal),
            Some(b'q') => (QUEEN, Special::Normal),
            _ => return None,
        };
        Some((from, to, promo, special))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ongoing = 0,
    Checkmate = 1,
    Stalemate = 2,
    FiftyMove = 3,
    Threefold = 4,
    Insufficient = 5,
    /// 手番側の駒のどれもキングでありえない (= キングを取られた)。手番側の負け
    KingCaptured = 6,
    /// 直前に指した側が、自分の手で「自分のキングはすでに取られていた」と確定させた。指した側の負け
    KingRevealed = 7,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Position {
    /// 白 0..16、黒 16..32
    pub pieces: [Piece; 32],
    /// マスにいる駒の id + 1 (空きは 0)
    pub board: [u8; 64],
    pub stm: u8,
    /// (通過したマス, 2 マス進んだ駒の id)
    pub ep: Option<(u8, u8)>,
    pub halfmove: u32,
    pub fullmove: u32,
}

impl Default for Position {
    fn default() -> Self {
        Self::start()
    }
}

impl Position {
    pub fn start() -> Self {
        let mut pieces = [Piece { color: WHITE, sq: OFF, domain: ALL, promoted: 0, moved: false }; 32];
        let mut board = [0u8; 64];
        for (id, p) in pieces.iter_mut().enumerate() {
            let sq = if id < 16 { id as u8 } else { 48 + (id as u8 - 16) };
            p.color = if id < 16 { WHITE } else { BLACK };
            p.sq = sq;
            board[sq as usize] = id as u8 + 1;
        }
        Position { pieces, board, stm: WHITE, ep: None, halfmove: 0, fullmove: 1 }
    }

    pub fn piece_at(&self, sq: u8) -> Option<usize> {
        match self.board[sq as usize] {
            0 => None,
            v => Some(v as usize - 1),
        }
    }

    fn ids(c: u8) -> std::ops::Range<usize> {
        let base = c as usize * 16;
        base..base + 16
    }

    /// 生きている駒のうちキングでありうる駒の数
    pub fn king_candidates(&self, c: u8) -> usize {
        Self::ids(c).filter(|&i| {
            let p = &self.pieces[i];
            p.alive() && p.promoted == 0 && p.domain & K_ != 0
        }).count()
    }

    /// キングに確定した駒のマス
    pub fn definite_king(&self, c: u8) -> Option<u8> {
        Self::ids(c).map(|i| &self.pieces[i]).find(|p| p.is_definite_king()).map(|p| p.sq)
    }

    // ---- 割り当て (駒数制約) ------------------------------------------------

    /// SOS DP で f[S] = (domain ⊆ S の駒数) を求める
    fn subset_counts(&self, c: u8) -> [u8; 64] {
        let mut f = [0u8; 64];
        for i in Self::ids(c) {
            f[self.pieces[i].domain as usize] += 1;
        }
        for b in 0..6 {
            for s in 0..64 {
                if s & (1 << b) != 0 {
                    f[s] += f[s ^ (1 << b)];
                }
            }
        }
        f
    }

    fn capacity(s: usize) -> u8 {
        CAPACITY[s]
    }

    /// 駒数制約を満たす割り当てが存在するか (Hall の条件)
    pub fn feasible(&self, c: u8) -> bool {
        let f = self.subset_counts(c);
        (0..64).all(|s| f[s] <= Self::capacity(s))
    }

    /// どの割り当てでもありえない種類を除外する (割り当てが存在する前提)。
    ///
    /// 駒 i を種類 k に固定して実行不能になるのは、ちょうど埋まっている集合 S
    /// (f[S] = cap[S]) で k ∈ S かつ domain_i ⊄ S のものがあるとき。
    /// 支えのない値を消しても他の値の支えは崩れないので 1 回で収束する。
    fn prune(&mut self, c: u8) {
        let f = self.subset_counts(c);
        let mut tight = [0u8; 64];
        let mut nt = 0;
        for s in 1..63 {
            if f[s] >= Self::capacity(s) {
                tight[nt] = s as u8;
                nt += 1;
            }
        }
        if nt == 0 {
            return;
        }
        for i in Self::ids(c) {
            let d = self.pieces[i].domain;
            if d.count_ones() <= 1 {
                continue;
            }
            let mut nd = d;
            for b in 0..6 {
                let kb = 1u8 << b;
                if d & kb != 0 && tight[..nt].iter().any(|&t| t & kb != 0 && d & !t != 0) {
                    nd &= !kb;
                }
            }
            self.pieces[i].domain = nd;
        }
    }

    // ---- 利き ---------------------------------------------------------------

    fn clear_line(&self, from: u8, to: u8, df: i8, dr: i8) -> bool {
        let (mut f, mut r) = (file(from) + df, rank(from) + dr);
        while let Some(s) = at(f, r) {
            if s == to {
                return true;
            }
            if self.board[s as usize] != 0 {
                return false;
            }
            f += df;
            r += dr;
        }
        false
    }

    /// 駒 `id` が (いずれかの種類として) マス `sq` の駒を取れるか
    fn can_hit(&self, id: usize, sq: u8) -> bool {
        let p = &self.pieces[id];
        let (df, dr) = (file(sq) - file(p.sq), rank(sq) - rank(p.sq));
        if df == 0 && dr == 0 {
            return false;
        }
        let ks = p.kinds();
        let dir = if p.color == WHITE { 1 } else { -1 };
        if ks & P_ != 0 && dr == dir && df.abs() == 1 {
            return true;
        }
        if ks & N_ != 0 && KNIGHT_D.contains(&(df, dr)) {
            return true;
        }
        if ks & K_ != 0 && df.abs() <= 1 && dr.abs() <= 1 {
            return true;
        }
        let straight = df == 0 || dr == 0;
        let diagonal = df.abs() == dr.abs();
        if (straight && ks & (R_ | Q_) != 0) || (diagonal && ks & (B_ | Q_) != 0) {
            return self.clear_line(p.sq, sq, df.signum(), dr.signum());
        }
        false
    }

    /// マス `sq` が色 `by` の駒に (可能性として) 利かされているか
    pub fn attacked(&self, sq: u8, by: u8) -> bool {
        Self::ids(by).any(|i| self.pieces[i].alive() && self.can_hit(i, sq))
    }

    /// 手番側のキング (確定している場合のみ) に王手がかかっているか
    pub fn in_check(&self) -> bool {
        match self.definite_king(self.stm) {
            Some(k) => self.attacked(k, self.stm ^ 1),
            None => false,
        }
    }

    // ---- 指し手生成 -----------------------------------------------------------

    fn pseudo_moves(&self, out: &mut Vec<Move>) {
        self.gen_moves(out, false);
    }

    /// 指し手生成。`captures_only` なら駒取り・アンパッサン・成りだけ (静止探索用)
    fn gen_moves(&self, out: &mut Vec<Move>, captures_only: bool) {
        let us = self.stm;
        let them = us ^ 1;
        let enemy = |s: u8| self.piece_at(s).map(|i| self.pieces[i].color == them).unwrap_or(false);
        for id in Self::ids(us) {
            let p = self.pieces[id];
            if !p.alive() {
                continue;
            }
            let from = p.sq;
            let (f, r) = (file(from), rank(from));
            let ks = p.kinds();
            let mut masks = [0u8; 64];
            let mut touched = 0u64; // masks が 0 でないマス

            if ks & P_ != 0 {
                let dir: i8 = if us == WHITE { 1 } else { -1 };
                let last = if us == WHITE { 7 } else { 0 };
                // 2 マス前進は、まだ動いていない駒が 2 段目 (黒は 7 段目) か 1 段目 (黒は 8 段目) にいるとき
                let (start, home) = if us == WHITE { (1, 0) } else { (6, 7) };
                let promos = |to: u8, out: &mut Vec<Move>| {
                    for k in [QUEEN, ROOK, BISHOP, KNIGHT] {
                        out.push(Move { from, to, promo: k, special: Special::Normal, mask: P_ });
                    }
                };
                if let Some(to) = at(f, r + dir) {
                    if self.board[to as usize] == 0 {
                        if rank(to) == last {
                            promos(to, out);
                        } else if !captures_only {
                            masks[to as usize] |= P_;
                            touched |= 1 << to;
                        }
                        if !captures_only && !p.moved && (r == start || r == home) {
                            let to2 = at(f, r + 2 * dir).unwrap();
                            if self.board[to2 as usize] == 0 {
                                masks[to2 as usize] |= P_;
                                touched |= 1 << to2;
                            }
                        }
                    }
                }
                for df in [-1, 1] {
                    if let Some(to) = at(f + df, r + dir) {
                        if enemy(to) {
                            if rank(to) == last {
                                promos(to, out);
                            } else {
                                masks[to as usize] |= P_;
                                touched |= 1 << to;
                            }
                        } else if matches!(self.ep, Some((s, _)) if s == to) {
                            out.push(Move { from, to, promo: 0, special: Special::EnPassant, mask: P_ });
                        }
                    }
                }
            }
            for (kb, ds) in [(N_, &KNIGHT_D), (K_, &KING_D)] {
                if ks & kb == 0 {
                    continue;
                }
                for &(df, dr) in ds {
                    if let Some(to) = at(f + df, r + dr) {
                        if (!captures_only && self.board[to as usize] == 0) || enemy(to) {
                            masks[to as usize] |= kb;
                            touched |= 1 << to;
                        }
                    }
                }
            }
            for (kb, ds) in [(R_ | Q_, &ROOK_D), (B_ | Q_, &BISHOP_D)] {
                let m = ks & kb;
                if m == 0 {
                    continue;
                }
                for &(df, dr) in ds {
                    let (mut cf, mut cr) = (f + df, r + dr);
                    while let Some(to) = at(cf, cr) {
                        if self.board[to as usize] == 0 {
                            if !captures_only {
                                masks[to as usize] |= m;
                                touched |= 1 << to;
                            }
                        } else {
                            if enemy(to) {
                                masks[to as usize] |= m;
                                touched |= 1 << to;
                            }
                            break;
                        }
                        cf += df;
                        cr += dr;
                    }
                }
            }
            while touched != 0 {
                let to = touched.trailing_zeros() as u8;
                touched &= touched - 1;
                out.push(Move { from, to, promo: 0, special: Special::Normal, mask: masks[to as usize] });
            }
            if captures_only {
                continue;
            }

            // キャスリング: 未移動のキング候補 (e1/e8) と未移動のルーク候補
            let base = if us == WHITE { 0u8 } else { 56 };
            if from == base + 4 && !p.moved && p.promoted == 0 && p.domain & K_ != 0 {
                let sides: [(u8, &[u8], u8, [u8; 3]); 2] = [
                    (base + 7, &[base + 5, base + 6], base + 6, [base + 4, base + 5, base + 6]),
                    (base, &[base + 1, base + 2, base + 3], base + 2, [base + 4, base + 3, base + 2]),
                ];
                for (rook_sq, between, king_to, pass) in sides {
                    let rook_ok = self.piece_at(rook_sq).map_or(false, |ri| {
                        let rp = &self.pieces[ri];
                        rp.color == us && !rp.moved && rp.promoted == 0 && rp.domain & R_ != 0
                    });
                    if rook_ok
                        && between.iter().all(|&s| self.board[s as usize] == 0)
                        && pass.iter().all(|&s| !self.attacked(s, them))
                    {
                        out.push(Move { from, to: king_to, promo: 0, special: Special::Castle, mask: K_ });
                    }
                }
            }
        }
    }

    pub fn is_capture(&self, m: Move) -> bool {
        m.special == Special::EnPassant || self.board[m.to as usize] != 0
    }

    /// 合法性チェックなしで指す (pseudo-legal な手を前提)
    pub fn apply(&self, m: Move) -> Position {
        let mut n = self.clone();
        let us = self.stm;
        let id = self.piece_at(m.from).expect("piece on from");
        let capture = self.is_capture(m);
        let mut prune_them = false;

        if let Some(v) = self.piece_at(m.to) {
            n.pieces[v].sq = OFF;
        }
        if m.special == Special::EnPassant {
            // 取られる駒は直前に 2 マス進んだ駒で、ポーンに確定する
            let (_, v) = self.ep.expect("ep square");
            let v = v as usize;
            n.board[n.pieces[v].sq as usize] = 0;
            n.pieces[v].sq = OFF;
            n.pieces[v].domain &= P_;
            prune_them = true;
        }
        n.board[m.from as usize] = 0;
        n.board[m.to as usize] = id as u8 + 1;

        let before = n.pieces[id].domain;
        {
            let p = &mut n.pieces[id];
            p.sq = m.to;
            p.moved = true;
            if p.promoted == 0 {
                p.domain &= m.mask;
                if m.promo != 0 {
                    p.promoted = m.promo;
                }
            }
        }
        let mut changed = n.pieces[id].domain != before;

        if m.special == Special::Castle {
            let (rf, rt) = if m.to > m.from { (m.from + 3, m.from + 1) } else { (m.from - 4, m.from - 1) };
            let r = self.piece_at(rf).expect("castling rook");
            n.board[rf as usize] = 0;
            n.board[rt as usize] = r as u8 + 1;
            n.pieces[r].sq = rt;
            n.pieces[r].moved = true;
            n.pieces[r].domain &= R_;
            changed = true;
        }

        n.ep = None;
        let dist = (m.to as i8 - m.from as i8).abs();
        if m.special == Special::Normal && dist == 16 && n.pieces[id].kinds() & P_ != 0 {
            // 取り返せる相手のポーン候補がいるときだけ立てる (繰り返し判定のため)
            let them = us ^ 1;
            let can = [-1i8, 1].iter().any(|&df| {
                at(file(m.to) + df, rank(m.to)).and_then(|s| n.piece_at(s)).map_or(false, |i| {
                    let q = &n.pieces[i];
                    q.color == them && q.kinds() & P_ != 0
                })
            });
            if can {
                n.ep = Some(((m.from + m.to) / 2, id as u8));
            }
        }

        if changed {
            n.prune(us);
        }
        if prune_them {
            n.prune(us ^ 1);
        }
        n.halfmove = if capture || m.mask & P_ != 0 { 0 } else { self.halfmove + 1 };
        if us == BLACK {
            n.fullmove += 1;
        }
        n.stm = us ^ 1;
        n
    }

    /// 合法手と指した後の局面
    pub fn legal_children(&self) -> Vec<(Move, Position)> {
        self.legal_children_where(|_| true)
    }

    /// `keep` を満たす手だけの合法手と指した後の局面
    pub fn legal_children_where(&self, keep: impl Fn(&Move) -> bool) -> Vec<(Move, Position)> {
        self.pseudo_legal()
            .into_iter()
            .filter(|m| keep(m))
            .filter_map(|m| self.play_legal(m).map(|n| (m, n)))
            .collect()
    }

    /// 合法性を確かめる前の駒取り・アンパッサン・成り (静止探索用)
    pub fn pseudo_captures(&self) -> Vec<Move> {
        let mut out = Vec::with_capacity(32);
        self.gen_moves(&mut out, true);
        out
    }

    /// 合法性を確かめる前の手 (エンジンが手を並べ替えてから 1 手ずつ指すため)
    pub fn pseudo_legal(&self) -> Vec<Move> {
        let mut out = Vec::with_capacity(96);
        self.pseudo_moves(&mut out);
        out
    }

    /// pseudo-legal な手 `m` を指す。反則なら None
    pub fn play_legal(&self, m: Move) -> Option<Position> {
        let us = self.stm;
        let n = self.apply(m);
        if m.special == Special::Castle && !n.feasible(us) {
            return None;
        }
        // 確定したキングを利きに残す手は不可
        if let Some(k) = n.definite_king(us) {
            if n.attacked(k, us ^ 1) {
                return None;
            }
        }
        Some(n)
    }

    /// 置換表用のハッシュ (駒の位置・ありうる種類・成り・未移動、手番、アンパッサン)
    pub fn hash(&self) -> u64 {
        fn mix(mut z: u64) -> u64 {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        let mut h = mix(self.stm as u64 + 1);
        for (i, p) in self.pieces.iter().enumerate() {
            let v = p.sq as u64 | (p.domain as u64) << 8 | (p.promoted as u64) << 16 | (p.moved as u64) << 24;
            h ^= mix((i as u64) << 32 | v);
        }
        if let Some((s, _)) = self.ep {
            h ^= mix(0xEE00 | s as u64);
        }
        h
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        self.legal_children().into_iter().map(|(m, _)| m).collect()
    }

    pub fn parse_move(&self, s: &str) -> Option<Move> {
        let (from, to, promo, special) = Move::parse(s)?;
        self.legal_moves().into_iter().find(|m| m.from == from && m.to == to && m.promo == promo && m.special == special)
    }

    /// 表記: "e2-e4(PRQ)"、"b1-c3(N)"、"d7xe8=Q"、"O-O"、"e5xd6 e.p."、王手で "+"、詰み・キング捕獲で "#"、
    /// 自分のキングが取られていたと確定させた (指した側の負け) 手は "†"
    pub fn notation(&self, m: Move) -> String {
        let n = self.apply(m);
        let mut s = match m.special {
            Special::Castle => (if m.to > m.from { "O-O" } else { "O-O-O" }).to_string(),
            _ => {
                let mut s = sq_name(m.from);
                s.push(if self.is_capture(m) { 'x' } else { '-' });
                s.push_str(&sq_name(m.to));
                if m.special == Special::EnPassant {
                    s.push_str(" e.p.");
                } else if m.promo != 0 {
                    s.push('=');
                    s.push(LETTERS[m.promo as usize - 1] as char);
                } else {
                    let id = n.piece_at(m.to).unwrap();
                    s.push_str(&format!("({})", letters(n.pieces[id].kinds())));
                }
                s
            }
        };
        if n.king_candidates(us_of(&n)) == 0 {
            // 自分のキングがすでに取られていたことが確定した (指した側の負け)
            s.push('†');
        } else if n.king_candidates(n.stm) == 0 {
            s.push('#');
        } else if n.in_check() {
            s.push(if n.legal_children().is_empty() { '#' } else { '+' });
        }
        s
    }

    /// 生きている駒が確定キングだけなら駒不足
    pub fn insufficient_material(&self) -> bool {
        self.pieces.iter().filter(|p| p.alive()).all(|p| p.is_definite_king())
    }

    /// UI 用: 64 マスを空白区切り。空きは "."、駒は色 (w/b) + 今ありうる種類 (例 "wpnbrqk", "bn")
    pub fn board_tokens(&self) -> String {
        (0..64u8).map(|sq| match self.piece_at(sq) { None => ".".to_string(), Some(i) => self.token(i) }).collect::<Vec<_>>().join(" ")
    }

    /// UI 用: 取られた駒を空白区切り (色 + 初期配置での役割としてありうる種類)
    pub fn captured_tokens(&self) -> String {
        self.pieces.iter().enumerate().filter(|(_, p)| !p.alive()).map(|(i, _)| self.token(i)).collect::<Vec<_>>().join(" ")
    }

    fn token(&self, i: usize) -> String {
        token_of(&self.pieces[i])
    }

    /// 繰り返し判定用のキー (手数カウンタを除く)
    fn same_position(&self, o: &Position) -> bool {
        self.stm == o.stm && self.ep == o.ep && self.pieces == o.pieces
    }
}

/// UI 用の駒の表記: 色 (w/b) + 今ありうる種類 (取られた駒は取られた時点の種類)
fn token_of(p: &Piece) -> String {
    let mut s = String::from(if p.color == WHITE { "w" } else { "b" });
    s.push_str(&letters(p.kinds()).to_ascii_lowercase());
    s
}

/// 直前に指した側の色
fn us_of(p: &Position) -> u8 {
    p.stm ^ 1
}

/// 対局全体 (履歴つき)
pub struct Game {
    pub history: Vec<Position>,
    pub moves: Vec<Move>,
    pub notes: Vec<String>,
}

impl Default for Game {
    fn default() -> Self {
        Game { history: vec![Position::start()], moves: vec![], notes: vec![] }
    }
}

impl Game {
    pub fn pos(&self) -> &Position {
        self.history.last().unwrap()
    }

    /// 手のコードで指す。成功時は表記を返す
    pub fn play(&mut self, code: &str) -> Option<String> {
        if self.status() != Status::Ongoing {
            return None;
        }
        let m = self.pos().parse_move(code)?;
        self.push(m);
        Some(self.notes.last().unwrap().clone())
    }

    pub fn push(&mut self, m: Move) {
        let note = self.pos().notation(m);
        let next = self.pos().apply(m);
        self.history.push(next);
        self.moves.push(m);
        self.notes.push(note);
    }

    pub fn undo(&mut self) -> bool {
        if self.moves.is_empty() {
            return false;
        }
        self.history.pop();
        self.moves.pop();
        self.notes.pop();
        true
    }

    /// UI 用: 直前の手を「重ね合わせを更新する前」の状態で見せるための情報 (4 行)。
    /// 1 行目: 指した後の配置を、指す前の種類で表した 64 マス (board_tokens と同じ形式)
    /// 2 行目: 取られた駒を指す前の種類で (captured_tokens と同じ順)
    /// 3 行目: この手で種類が減った盤上の駒のマス
    /// 4 行目: この手で種類が減った取られた駒の、取られた駒一覧での位置
    pub fn preview(&self) -> Option<String> {
        let n = self.history.len();
        if n < 2 {
            return None;
        }
        let (prev, cur) = (&self.history[n - 2], &self.history[n - 1]);
        let board: Vec<String> = (0..64u8)
            .map(|sq| cur.piece_at(sq).map_or(".".to_string(), |i| token_of(&prev.pieces[i])))
            .collect();
        let captured: Vec<usize> = (0..32).filter(|&i| !cur.pieces[i].alive()).collect();
        let changed = (0..64u8)
            .filter(|&sq| cur.piece_at(sq).map_or(false, |i| prev.pieces[i].kinds() != cur.pieces[i].kinds()))
            .map(|sq| sq.to_string())
            .collect::<Vec<_>>();
        let changed_cap = captured
            .iter()
            .enumerate()
            .filter(|(_, &i)| prev.pieces[i].kinds() != cur.pieces[i].kinds())
            .map(|(k, _)| k.to_string())
            .collect::<Vec<_>>();
        Some(
            [
                board.join(" "),
                captured.iter().map(|&i| token_of(&prev.pieces[i])).collect::<Vec<_>>().join(" "),
                changed.join(" "),
                changed_cap.join(" "),
            ]
            .join("\n"),
        )
    }

    pub fn repetitions(&self) -> usize {
        let pos = self.pos();
        let n = pos.halfmove as usize;
        self.history.iter().rev().take(n + 1).filter(|p| p.same_position(pos)).count()
    }

    pub fn status(&self) -> Status {
        let pos = self.pos();
        // 指した側のキングが過去に取られていたと確定した場合を先に見る (取られたのはこの手より前なので)
        if pos.king_candidates(pos.stm ^ 1) == 0 {
            return Status::KingRevealed;
        }
        if pos.king_candidates(pos.stm) == 0 {
            return Status::KingCaptured;
        }
        if pos.legal_children().is_empty() {
            return if pos.in_check() { Status::Checkmate } else { Status::Stalemate };
        }
        if pos.insufficient_material() {
            return Status::Insufficient;
        }
        if pos.halfmove >= 100 {
            return Status::FiftyMove;
        }
        if self.repetitions() >= 3 {
            return Status::Threefold;
        }
        Status::Ongoing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(s: &str) -> u8 {
        parse_sq(s.as_bytes()).unwrap()
    }
    fn kinds_at(g: &Game, s: &str) -> String {
        let p = g.pos();
        letters(p.pieces[p.piece_at(sq(s)).unwrap()].kinds())
    }
    /// 盤面を空にして指定の駒だけ置く (テスト用)。残りの駒は「種類不明の取られた駒」
    fn custom(white: &[(&str, u8)], black: &[(&str, u8)]) -> Position {
        let mut p = Position::start();
        p.board = [0; 64];
        for pc in p.pieces.iter_mut() {
            pc.sq = OFF;
            pc.moved = true;
        }
        for (c, list) in [(WHITE, white), (BLACK, black)] {
            for (id, &(s, d)) in Position::ids(c).zip(list.iter()) {
                p.pieces[id].sq = sq(s);
                p.pieces[id].domain = d;
                p.board[sq(s) as usize] = id as u8 + 1;
            }
            p.prune(c);
        }
        p
    }

    #[test]
    fn knight_jump_collapses_to_knight() {
        let mut g = Game::default();
        assert!(g.play("b1c3").is_some());
        assert_eq!(kinds_at(&g, "c3"), "N");
    }

    #[test]
    fn two_step_keeps_consistent_kinds() {
        let mut g = Game::default();
        assert_eq!(g.play("e2e4").as_deref(), Some("e2-e4(PRQ)"));
        g.play("e7e6").unwrap();
        assert_eq!(kinds_at(&g, "e6"), "PRQK");
    }

    #[test]
    fn counting_removes_kinds_from_unmoved_pieces() {
        let mut g = Game::default();
        // d1 が斜めに 2 マス進めない (c2/e2 が塞いでいる) ので、b1 と g1 をナイトに確定させる
        g.play("b1c3").unwrap();
        g.play("b8c6").unwrap();
        g.play("g1f3").unwrap();
        // ナイトは 2 枚しかないので、白の他の駒はすべてナイトではない
        let p = g.pos();
        for i in 0..16 {
            let pc = &p.pieces[i];
            if pc.sq != sq("c3") && pc.sq != sq("f3") {
                assert_eq!(pc.domain & N_, 0, "piece at {}", sq_name(pc.sq));
            }
        }
        // 黒は 1 枚だけなので他の駒はまだナイトでありうる
        assert_ne!(p.pieces[p.piece_at(sq("g8")).unwrap()].domain & N_, 0);
    }

    #[test]
    fn queen_confirmation_excludes_others() {
        let mut p = Position::start();
        let id = p.piece_at(sq("d1")).unwrap();
        p.pieces[id].domain = Q_;
        p.prune(WHITE);
        for i in 0..16 {
            if i != id {
                assert_eq!(p.pieces[i].domain & Q_, 0);
            }
        }
    }

    #[test]
    fn eight_pawns_forced() {
        // 8 枚以外の駒がすべてポーン以外に確定すれば残り 8 枚はポーン
        let mut p = Position::start();
        let roles = [R_, N_, B_, Q_, K_, B_, N_, R_];
        for (f, &d) in roles.iter().enumerate() {
            p.pieces[f].domain = d;
        }
        p.prune(WHITE);
        for i in 8..16 {
            assert_eq!(p.pieces[i].domain, P_);
        }
    }

    #[test]
    fn check_applies_only_to_definite_king() {
        // 白キング確定 e1、黒ルーク確定 e8。白は e 列から逃げないといけない
        let p = custom(&[("e1", K_), ("a2", P_)], &[("e8", R_), ("h8", K_)]);
        assert!(p.in_check());
        for m in p.legal_moves() {
            assert_eq!(m.from, sq("e1"), "only king moves: {}", m.code());
            assert_ne!(file(m.to), 4);
        }
        // キングが未確定なら王手はない
        let p = custom(&[("e1", K_ | Q_), ("a2", P_)], &[("e8", R_), ("h8", K_)]);
        assert!(!p.in_check());
    }

    #[test]
    fn capturing_last_king_candidate_wins() {
        let p = custom(&[("e1", K_), ("d4", Q_)], &[("d5", K_ | R_), ("h8", R_)]);
        let mut g = Game { history: vec![p], moves: vec![], notes: vec![] };
        assert_eq!(g.play("d4d5").as_deref(), Some("d4xd5(Q)#"));
        assert_eq!(g.status(), Status::KingCaptured);
    }

    #[test]
    fn castling_confirms_king_and_rook() {
        let mut g = Game::default();
        for m in ["g1f3", "a7a6", "e2e3", "a6a5", "f1e2", "a5a4"] {
            g.play(m).unwrap();
        }
        assert!(g.play("e1g1c").is_some(), "{:?}", g.pos().legal_moves().iter().map(|m| m.code()).collect::<Vec<_>>());
        assert_eq!(kinds_at(&g, "g1"), "K");
        assert_eq!(kinds_at(&g, "f1"), "R");
    }

    #[test]
    fn en_passant_confirms_both_pawns() {
        let mut g = Game::default();
        for m in ["e2e4", "a7a6", "e4e5", "d7d5"] {
            g.play(m).unwrap();
        }
        // d6 への斜め移動は「アンパッサン (ポーン)」と「ビショップ等の通常移動」の 2 通り
        let codes: Vec<String> = g.pos().legal_moves().iter().filter(|m| m.from == sq("e5") && m.to == sq("d6")).map(|m| m.code()).collect();
        assert!(codes.contains(&"e5d6e".to_string()) && codes.contains(&"e5d6".to_string()), "{codes:?}");
        g.play("e5d6e").unwrap();
        assert_eq!(kinds_at(&g, "d6"), "P");
        assert!(g.pos().piece_at(sq("d5")).is_none());
        assert!(g.pos().captured_tokens().contains("bp"));
    }

    #[test]
    fn unmoved_pawn_on_first_rank_can_double_step() {
        let mut p = custom(&[("e1", K_), ("b1", P_)], &[("h8", K_), ("c3", P_), ("g8", P_)]);
        for s in ["b1", "g8"] {
            let id = p.piece_at(sq(s)).unwrap();
            p.pieces[id].moved = false;
        }
        let codes: Vec<String> = p.legal_moves().iter().filter(|m| m.from == sq("b1")).map(|m| m.code()).collect();
        assert!(codes.contains(&"b1b2".to_string()) && codes.contains(&"b1b3".to_string()), "{codes:?}");
        // 2 段目の駒が塞いでいると 2 マス前進もできない
        let mut q = custom(&[("e1", K_), ("b1", P_), ("b2", P_)], &[("h8", K_)]);
        let id = q.piece_at(sq("b1")).unwrap();
        q.pieces[id].moved = false;
        assert!(!q.legal_moves().iter().any(|m| m.from == sq("b1") && m.to == sq("b3")));

        // 1 段目からの 2 マス前進もアンパッサンで取れる
        let mut g = Game { history: vec![p], moves: vec![], notes: vec![] };
        g.play("b1b3").unwrap();
        g.play("c3b2e").unwrap();
        assert!(g.pos().piece_at(sq("b3")).is_none());
        // 一度動いた駒はもう 2 マス進めない。黒の 8 段目の未移動の駒は 2 マス進める
        assert!(g.pos().legal_moves().iter().any(|m| m.code() == "e1d1"));
        g.play("e1d1").unwrap();
        assert!(g.play("g8g6").is_some());
    }

    #[test]
    fn promotion_or_not() {
        let p = custom(&[("a7", P_ | R_), ("e1", K_)], &[("h8", K_)]);
        let codes: Vec<String> = p.legal_moves().iter().filter(|m| m.from == sq("a7") && m.to == sq("a8")).map(|m| m.code()).collect();
        assert!(codes.contains(&"a7a8q".to_string()) && codes.contains(&"a7a8".to_string()), "{codes:?}");
        let n = p.apply(p.parse_move("a7a8").unwrap());
        assert_eq!(n.pieces[n.piece_at(sq("a8")).unwrap()].kinds(), R_);
        let n = p.apply(p.parse_move("a7a8q").unwrap());
        let pc = n.pieces[n.piece_at(sq("a8")).unwrap()];
        assert_eq!((pc.domain, pc.kinds()), (P_, Q_));
    }

    /// 自分の手で「自分のキングはすでに取られていた」と確定させたら、その時点で指した側の負け
    #[test]
    fn revealing_own_captured_king_loses() {
        let mut g = Game::default();
        // f1 の駒 (RQK) が取られた後、白がクイーンと 2 枚のルークを確定させると、取られた駒がキングに確定する
        for m in ["e2e4", "b7e4", "d2e4", "a7f2", "f1f2", "f7f2", "g1f2", "f8f5", "f2f5", "e7f5", "c2c7", "a8c7"] {
            g.play(m).unwrap();
            assert_eq!(g.status(), Status::Ongoing, "{m}");
        }
        assert_eq!(g.play("b2b8").as_deref(), Some("b2xb8(R)†"));
        assert_eq!(g.status(), Status::KingRevealed);
        assert!(g.play("c8b8").is_none(), "決着後は指せない");
        assert!(g.pos().captured_tokens().split(' ').any(|t| t == "wk"));
    }

    #[test]
    fn preview_shows_kinds_before_collapse() {
        let mut g = Game::default();
        assert!(g.preview().is_none());
        g.play("b1c3").unwrap(); // ナイトに確定
        g.play("b8c6").unwrap();
        g.play("g1f3").unwrap(); // 2 枚目のナイト → 他の白の駒からナイトが除外される
        let pv = g.preview().unwrap();
        let lines: Vec<&str> = pv.split('\n').collect();
        assert_eq!(lines.len(), 4);
        let board: Vec<&str> = lines[0].split(' ').collect();
        assert_eq!(board[sq("f3") as usize], "wpnbrqk"); // 指す前の種類のまま f3 に
        assert_eq!(board[sq("g1") as usize], ".");
        let changed: Vec<u8> = lines[2].split(' ').map(|s| s.parse().unwrap()).collect();
        assert!(changed.contains(&sq("f3")) && changed.contains(&sq("a1")) && !changed.contains(&sq("c3")));
        assert!(!changed.contains(&sq("c6")), "黒の駒は変わらない");
    }

    // ---- 不変条件の検証 (Hall の条件とは独立に、二部マッチングで確かめる) ----

    /// 駒 → 種類の枠 (P×8, N×2, ...) の完全マッチングが存在するか (Kuhn 法)
    fn matching_exists(domains: &[u8; 16]) -> bool {
        let mut slots = Vec::new();
        for (b, &n) in INITIAL_COUNT.iter().enumerate() {
            for _ in 0..n {
                slots.push(1u8 << b);
            }
        }
        let mut owner = [usize::MAX; 16];
        fn augment(i: usize, d: &[u8; 16], slots: &[u8], owner: &mut [usize; 16], seen: &mut [bool; 16]) -> bool {
            for j in 0..16 {
                if d[i] & slots[j] != 0 && !seen[j] {
                    seen[j] = true;
                    if owner[j] == usize::MAX || augment(owner[j], d, slots, owner, seen) {
                        owner[j] = i;
                        return true;
                    }
                }
            }
            false
        }
        (0..16).all(|i| augment(i, domains, &slots, &mut owner, &mut [false; 16]))
    }

    /// 動きの記録 `geo` (各駒の手と矛盾しない種類) だけから、本来残るべき種類を求めて実装と比べる。
    /// 割り当てが存在すること、除外漏れ・除外しすぎがないことを同時に確かめる
    fn assert_consistent(p: &Position, geo: &[u8; 32], ctx: &str) {
        for c in [WHITE, BLACK] {
            let base = c as usize * 16;
            let mut g = [0u8; 16];
            g.copy_from_slice(&geo[base..base + 16]);
            assert!(matching_exists(&g), "割り当てが存在しない: {ctx}");
            for i in 0..16 {
                let mut expected = 0u8;
                for b in 0..6 {
                    let kb = 1u8 << b;
                    if g[i] & kb == 0 {
                        continue;
                    }
                    let mut fixed = g;
                    fixed[i] = kb;
                    if matching_exists(&fixed) {
                        expected |= kb;
                    }
                }
                let actual = p.pieces[base + i].domain;
                assert_eq!(letters(actual), letters(expected), "色 {c} 駒 {i}: {ctx}");
            }
        }
    }

    /// アンパッサンが起きやすいように偏らせたランダム対局で不変条件を確かめる
    #[test]
    fn en_passant_keeps_assignment_consistent() {
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut rnd = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let mut ep_count = 0;
        for game in 0..300 {
            let mut g = Game::default();
            let mut geo = [ALL; 32];
            for ply in 0..80 {
                if g.status() != Status::Ongoing {
                    break;
                }
                let pos = g.pos().clone();
                let moves = pos.legal_moves();
                let ep: Vec<&Move> = moves.iter().filter(|m| m.special == Special::EnPassant).collect();
                // アンパッサンがあれば必ず指す。なければ 2 マス前進と前進を優先して ep の機会を作る
                let m = if !ep.is_empty() {
                    *ep[rnd(ep.len())]
                } else {
                    let pushes: Vec<&Move> = moves
                        .iter()
                        .filter(|m| m.special == Special::Normal && m.promo == 0 && file(m.from) == file(m.to) && m.mask & P_ != 0)
                        .collect();
                    if !pushes.is_empty() && rnd(3) != 0 { *pushes[rnd(pushes.len())] } else { moves[rnd(moves.len())] }
                };
                let ctx = format!("game {game} ply {ply} move {}", m.code());
                let n = pos.apply(m);
                // 動きの記録を実装とは別に更新する
                let id = pos.piece_at(m.from).unwrap();
                if pos.pieces[id].promoted == 0 {
                    geo[id] &= m.mask;
                }
                if m.special == Special::Castle {
                    let rf = if m.to > m.from { m.from + 3 } else { m.from - 4 };
                    geo[pos.piece_at(rf).unwrap()] &= R_;
                }
                if m.special == Special::EnPassant {
                    geo[pos.ep.unwrap().1 as usize] &= P_;
                }
                assert_consistent(&n, &geo, &ctx);
                if m.special == Special::EnPassant {
                    ep_count += 1;
                    let (_, v) = pos.ep.unwrap();
                    let victim = &n.pieces[v as usize];
                    assert!(!victim.alive() && victim.domain == P_, "取られた駒がポーンに確定していない: {ctx}");
                    let mover = &n.pieces[n.piece_at(m.to).unwrap()];
                    assert_eq!(mover.domain, P_, "取った駒がポーンに確定していない: {ctx}");
                }
                g.push(m);
            }
        }
        assert!(ep_count > 30, "アンパッサンが少なすぎる: {ep_count}");
        eprintln!("en passant played: {ep_count}");
    }

    /// 取る手を優先したランダム対局で、キングの喪失確定・キング捕獲などの決着局面まで含めて、
    /// 取られた駒を含むすべての駒の重ね合わせが正しく更新されていることを確かめる
    #[test]
    fn random_games_keep_superposition_consistent() {
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut rnd = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let mut endings = std::collections::HashMap::new();
        for game in 0..400 {
            let mut g = Game::default();
            let mut geo = [ALL; 32];
            for ply in 0..200 {
                let st = g.status();
                if st != Status::Ongoing {
                    *endings.entry(format!("{st:?}")).or_insert(0) += 1;
                    if st == Status::KingRevealed {
                        // 指した側の盤上の駒にキング候補がなく、取られた駒のどれかがキングでありうる
                        let p = g.pos();
                        let mover = p.stm ^ 1;
                        let ids = Position::ids(mover);
                        assert!(ids.clone().all(|i| !p.pieces[i].alive() || p.pieces[i].kinds() & K_ == 0));
                        assert!(ids.clone().any(|i| !p.pieces[i].alive() && p.pieces[i].domain & K_ != 0));
                    }
                    break;
                }
                let pos = g.pos().clone();
                let moves = pos.legal_moves();
                let caps: Vec<&Move> = moves.iter().filter(|m| pos.is_capture(**m)).collect();
                let m = if !caps.is_empty() && rnd(4) != 0 { *caps[rnd(caps.len())] } else { moves[rnd(moves.len())] };
                let id = pos.piece_at(m.from).unwrap();
                if pos.pieces[id].promoted == 0 {
                    geo[id] &= m.mask;
                }
                if m.special == Special::Castle {
                    let rf = if m.to > m.from { m.from + 3 } else { m.from - 4 };
                    geo[pos.piece_at(rf).unwrap()] &= R_;
                }
                if m.special == Special::EnPassant {
                    geo[pos.ep.unwrap().1 as usize] &= P_;
                }
                g.push(m);
                assert_consistent(g.pos(), &geo, &format!("game {game} ply {ply} move {}", m.code()));
            }
        }
        eprintln!("endings: {endings:?}");
        assert!(endings.get("KingRevealed").copied().unwrap_or(0) > 10, "{endings:?}");
    }
}
