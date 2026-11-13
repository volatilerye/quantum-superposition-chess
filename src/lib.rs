//! WebAssembly 向けの薄い FFI 層。
//!
//! wasm-bindgen を使わず、`extern "C"` の関数と 2 つの静的バッファだけで
//! JavaScript とやり取りする (依存ゼロでビルドできるように)。
//!
//! - 文字列入力: JS が `input_ptr()` に UTF-8 を書き、長さを引数で渡す
//! - 文字列出力: 関数が `output_ptr()` に書き、戻り値で長さを返す

pub mod chess;

use chess::{Game, Position, START_FEN};
use std::cell::UnsafeCell;

const BUF: usize = 4096;

struct Global {
    game: UnsafeCell<Option<Game>>,
    input: UnsafeCell<[u8; BUF]>,
    output: UnsafeCell<Vec<u8>>,
}
// wasm32-unknown-unknown はシングルスレッドなので共有して問題ない
unsafe impl Sync for Global {}

static G: Global = Global {
    game: UnsafeCell::new(None),
    input: UnsafeCell::new([0; BUF]),
    output: UnsafeCell::new(Vec::new()),
};

fn game() -> &'static mut Game {
    // SAFETY: シングルスレッド前提。参照は各 FFI 呼び出しの中でのみ使う
    unsafe { (*G.game.get()).get_or_insert_with(Game::default) }
}

fn input(len: usize) -> &'static str {
    let buf = unsafe { &*G.input.get() };
    std::str::from_utf8(&buf[..len.min(BUF)]).unwrap_or("")
}

fn output(s: &str) -> usize {
    let out = unsafe { &mut *G.output.get() };
    out.clear();
    out.extend_from_slice(s.as_bytes());
    out.len()
}

#[no_mangle]
pub extern "C" fn input_ptr() -> *mut u8 {
    G.input.get() as *mut u8
}

#[no_mangle]
pub extern "C" fn input_capacity() -> usize {
    BUF
}

#[no_mangle]
pub extern "C" fn output_ptr() -> *const u8 {
    unsafe { (*G.output.get()).as_ptr() }
}

/// 初期局面から新しい対局を始める
#[no_mangle]
pub extern "C" fn new_game() {
    unsafe { *G.game.get() = Some(Game::new(Position::start())) };
}

/// 入力バッファの FEN から対局を始める。成功で 1
#[no_mangle]
pub extern "C" fn load_fen(len: usize) -> i32 {
    match Position::from_fen(input(len)) {
        Some(p) => {
            unsafe { *G.game.get() = Some(Game::new(p)) };
            1
        }
        None => 0,
    }
}

/// 入力バッファの UCI 手を指す。成功なら SAN を出力し長さを返す。不正手は 0
#[no_mangle]
pub extern "C" fn play(len: usize) -> usize {
    let mv = input(len).to_string();
    match game().play_uci(&mv) {
        Some(san) => output(&san),
        None => 0,
    }
}

/// 1 手戻す。成功で 1
#[no_mangle]
pub extern "C" fn undo() -> i32 {
    game().undo() as i32
}

/// 合法手を空白区切りの UCI で出力
#[no_mangle]
pub extern "C" fn legal_moves() -> usize {
    let s: Vec<String> = game().pos().legal_moves().iter().map(|m| m.uci()).collect();
    output(&s.join(" "))
}

#[no_mangle]
pub extern "C" fn fen() -> usize {
    output(&game().pos().to_fen())
}

/// 対局開始局面の FEN (エンジンへの position コマンド用)
#[no_mangle]
pub extern "C" fn initial_fen() -> usize {
    output(&game().initial.to_fen())
}

/// これまでの手を空白区切りの UCI で出力
#[no_mangle]
pub extern "C" fn move_history() -> usize {
    let s: Vec<String> = game().moves.iter().map(|m| m.uci()).collect();
    output(&s.join(" "))
}

/// これまでの手を空白区切りの SAN で出力
#[no_mangle]
pub extern "C" fn san_history() -> usize {
    output(&game().sans.join(" "))
}

/// 64 文字の盤面 (a1..h8、空きは '.')
#[no_mangle]
pub extern "C" fn board() -> usize {
    output(&game().pos().board_string())
}

/// 0 = 白番, 1 = 黒番
#[no_mangle]
pub extern "C" fn side_to_move() -> i32 {
    (game().pos().stm != 0) as i32
}

#[no_mangle]
pub extern "C" fn in_check() -> i32 {
    game().pos().in_check() as i32
}

/// 0 進行中 / 1 チェックメイト / 2 ステイルメイト / 3 50 手 / 4 千日手 / 5 駒不足
#[no_mangle]
pub extern "C" fn status() -> i32 {
    game().status() as i32
}

#[no_mangle]
pub extern "C" fn is_start_position() -> i32 {
    (game().initial.to_fen() == START_FEN) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(s: &str) -> usize {
        let buf = unsafe { &mut *G.input.get() };
        buf[..s.len()].copy_from_slice(s.as_bytes());
        s.len()
    }
    fn out(len: usize) -> String {
        let o = unsafe { &*G.output.get() };
        String::from_utf8(o[..len].to_vec()).unwrap()
    }

    #[test]
    fn ffi_flow() {
        new_game();
        assert_eq!(out(legal_moves()).split(' ').count(), 20);
        assert_eq!(out(play(put("e2e4"))), "e4");
        assert_eq!(play(put("e2e4")), 0);
        assert_eq!(side_to_move(), 1);
        assert_eq!(out(move_history()), "e2e4");
        assert_eq!(undo(), 1);
        assert_eq!(out(fen()), START_FEN);
        assert_eq!(load_fen(put("not a fen")), 0);
        assert_eq!(load_fen(put("4k3/8/8/8/8/8/8/4K3 w - - 0 1")), 1);
        assert_eq!(status(), 5);
    }
}
