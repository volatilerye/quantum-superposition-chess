//! WebAssembly 向けの薄い FFI 層。
//!
//! wasm-bindgen を使わず、`extern "C"` の関数と 2 つの静的バッファだけで
//! JavaScript とやり取りする (依存ゼロでビルドできるように)。
//!
//! - 文字列入力: JS が `input_ptr()` に UTF-8 を書き、長さを引数で渡す
//! - 文字列出力: 関数が `output_ptr()` に書き、戻り値で長さを返す

pub mod chess;
pub mod engine;
pub mod quantum;

use quantum::Game;
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

/// 初期局面 (全駒が重ね合わせ) から新しい対局を始める
#[no_mangle]
pub extern "C" fn new_game(setup: u32) {
    // setup: ビット 0 = 白を通常のチェスの配置に、ビット 1 = 黒を通常のチェスの配置に (0 なら両方とも重ね合わせ)
    let classical = [setup & 1 != 0, setup & 2 != 0];
    unsafe { *G.game.get() = Some(Game::with_setup(classical)) };
}

/// 入力バッファの手 (e2e4 / e7e8q / e1g1c / e5d6e) を指す。成功なら表記を出力し長さを返す。不正手は 0
#[no_mangle]
pub extern "C" fn play(len: usize) -> usize {
    let mv = input(len).to_string();
    match game().play(&mv) {
        Some(note) => output(&note),
        None => 0,
    }
}

/// 1 手戻す。成功で 1
#[no_mangle]
pub extern "C" fn undo() -> i32 {
    game().undo() as i32
}

/// 合法手を空白区切りで出力
#[no_mangle]
pub extern "C" fn legal_moves() -> usize {
    let s: Vec<String> = game().pos().legal_moves().iter().map(|m| m.code()).collect();
    output(&s.join(" "))
}

/// これまでの手を空白区切りで出力
#[no_mangle]
pub extern "C" fn move_history() -> usize {
    let s: Vec<String> = game().moves.iter().map(|m| m.code()).collect();
    output(&s.join(" "))
}

/// これまでの手の表記を空白区切りで出力
#[no_mangle]
pub extern "C" fn notation_history() -> usize {
    let s: Vec<String> = game().notes.iter().map(|n| n.replace(' ', "_")).collect();
    output(&s.join(" "))
}

/// 64 マス (a1..h8) を空白区切り。"." = 空き、それ以外は色 (w/b) + ありうる種類 (pnbrqk)
#[no_mangle]
pub extern "C" fn board() -> usize {
    output(&game().pos().board_tokens())
}

/// 取られた駒を空白区切り (形式は board と同じ)
#[no_mangle]
pub extern "C" fn captured() -> usize {
    output(&game().pos().captured_tokens())
}

/// 直前の手を重ね合わせの更新前の状態で見せるための情報 (Game::preview)。初手前は 0
#[no_mangle]
pub extern "C" fn preview() -> usize {
    match game().preview() {
        Some(s) => output(&s),
        None => 0,
    }
}

/// 手番側の駒を from から to (0..64) へ動かせない理由 (Position::why_not)。合法手があれば 0
#[no_mangle]
pub extern "C" fn why_not(from: u32, to: u32) -> usize {
    output(&game().pos().why_not(from as u8, to as u8))
}

/// 0 = 白番, 1 = 黒番
#[no_mangle]
pub extern "C" fn side_to_move() -> i32 {
    game().pos().stm as i32
}

/// 手番側の確定キングに王手がかかっているか
#[no_mangle]
pub extern "C" fn in_check() -> i32 {
    game().pos().in_check() as i32
}

/// 0 進行中 / 1 チェックメイト / 2 ステイルメイト / 3 50 手 / 4 千日手 / 5 駒不足 / 6 キング捕獲
/// / 7 自分のキングの喪失が確定 / 8 対消滅 (1 と 6 は手番側の負け、7 は直前に指した側の負け、8 は引き分け)
#[no_mangle]
pub extern "C" fn status() -> i32 {
    game().status() as i32
}

/// 現局面を ms ミリ秒まで、または深さ depth 手まで探索し "手 評価値 深さ" を出力
/// (評価値は手番側から見た centipawn)。手がなければ 0
#[no_mangle]
pub extern "C" fn search(ms: f64, depth: u32) -> usize {
    let r = engine::search(game(), ms, depth);
    match r.best {
        Some(m) => output(&format!("{} {} {}", m.code(), r.score, r.depth)),
        None => 0,
    }
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
        new_game(0);
        assert_eq!(out(board()).split(' ').count(), 64);
        assert!(out(board()).starts_with("wpnbrqk "));
        assert_eq!(out(play(put("b1c3"))), "b1-c3(N)");
        assert_eq!(play(put("b1c3")), 0);
        assert_eq!(side_to_move(), 1);
        assert_eq!(out(move_history()), "b1c3");
        assert_eq!(undo(), 1);
        assert_eq!(status(), 0);
        assert!(out(search(50.0, 64)).split(' ').count() == 3);
    }
}
