// 一時的な探索用 (コミットしない)
use chess_core::quantum::{Game, Status};

#[test]
#[ignore]
fn find_shortest_revealing_game() {
    let secs: f64 = std::env::var("SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(60.0);
    let t0 = std::time::Instant::now();
    let mut seed: u64 = 0x1234_5678;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut best = usize::MAX;
    let mut games = 0u64;
    while t0.elapsed().as_secs_f64() < secs {
        games += 1;
        let mut g = Game::default();
        while g.moves.len() + 1 < best && g.status() == Status::Ongoing {
            let pos = g.pos().clone();
            let us = pos.stm;
            let children = pos.legal_children();
            if children.is_empty() {
                break;
            }
            if let Some((m, _)) = children.iter().find(|(_, n)| n.king_candidates(us) == 0) {
                let mut g2 = Game { history: g.history.clone(), moves: g.moves.clone(), notes: g.notes.clone() };
                g2.push(*m);
                best = g2.moves.len();
                let body: Vec<String> = g2
                    .notes
                    .iter()
                    .enumerate()
                    .map(|(i, n)| if i % 2 == 0 { format!("{}. {}", i / 2 + 1, n) } else { n.clone() })
                    .collect();
                println!("{} plies (game {games}, {:?}): {}", best, g2.status(), body.join(" "));
                break;
            }
            // 駒取り・種類を絞る手を優先
            let caps: Vec<usize> = (0..children.len()).filter(|&i| pos.is_capture(children[i].0)).collect();
            let pick = if !caps.is_empty() && rnd() % 100 < 70 {
                caps[(rnd() % caps.len() as u64) as usize]
            } else {
                (rnd() % children.len() as u64) as usize
            };
            g.push(children[pick].0);
        }
    }
    println!("games {games}, best {best}");
}
