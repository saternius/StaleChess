use rayon::prelude::*;
use std::cmp::{max, min};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::io::Write;

/// A board is represented as an array of 64 Option<char>, where each cell is either
/// None (empty) or Some(piece). Uppercase letters are White’s pieces, lowercase are Black’s.
/// The board cells are indexed 0..63 with index = (rank-1)*8 + file,
/// where file 0 corresponds to “a” and rank 1 is the bottom row.
type BoardArray = [Option<char>; 64];

/// Convert (file, rank) into an index. Here rank is 1-indexed and file is 0-indexed.
fn pos_to_index(x: u8, y: u8) -> usize {
    ((y - 1) as usize) * 8 + (x as usize)
}

/// A Placement represents one mirrored pair: a White piece on (white_x, white_y)
/// and its Black mirror on (black_x, black_y) with the given symbols.
#[derive(Clone, Copy)]
struct Placement {
    white_x: u8, // file (0 = a, …, 7 = h)
    white_y: u8, // rank (1..8)
    white: char,
    black_x: u8,
    black_y: u8,
    black: char,
}

/// Given one piece type (one of 'P','N','B','R','Q','K'),
/// generate all placements obeying the “mirroring” rules.
fn generate_placements(piece: char) -> Vec<Placement> {
    // “Starting rank” for white pieces and their corresponding black placements.
    let (srw, white_sym) = match piece {
        'P' => (2, 'P'),
        'N' => (1, 'N'),
        'B' => (1, 'B'),
        'R' => (1, 'R'),
        'Q' => (1, 'Q'),
        'K' => (1, 'K'),
        _ => panic!("Invalid piece type"),
    };
    let (srb, black_sym) = match piece {
        'P' => (7, 'p'),
        'N' => (8, 'n'),
        'B' => (8, 'b'),
        'R' => (8, 'r'),
        'Q' => (8, 'q'),
        'K' => (8, 'k'),
        _ => panic!("Invalid piece type"),
    };

    let mut placements = Vec::new();
    for file in 0..8u8 {
        for white_rank in 1..=4u8 {
            // For a White pawn skip rank 1.
            if white_sym == 'P' && white_rank == 1 {
                continue;
            }
            let distance = (white_rank as i32 - srw as i32).abs();
            let mut candidate_ranks = Vec::new();
            let cand1 = srb as i32 + distance;
            let cand2 = srb as i32 - distance;
            if cand1 >= 1 && cand1 <= 8 {
                candidate_ranks.push(cand1 as u8);
            }
            if distance != 0 && cand2 >= 1 && cand2 <= 8 {
                candidate_ranks.push(cand2 as u8);
            }
            for &black_rank in &candidate_ranks {
                if black_rank < 5 {
                    continue;
                }
                if black_sym == 'p' && (black_rank == 1 || black_rank == 8) {
                    continue;
                }
                placements.push(Placement {
                    white_x: file,
                    white_y: white_rank,
                    white: white_sym,
                    black_x: file,
                    black_y: black_rank,
                    black: black_sym,
                });
            }
        }
    }
    placements
}

/// Generate all combinations (with replacement) of piece types (as characters)
/// of length num_pairs from the set ['P','N','B','R','Q','K'].
fn generate_combinations(num_pairs: usize) -> Vec<Vec<char>> {
    let types = ['P', 'N', 'B', 'R', 'Q', 'K'];
    let mut results = Vec::new();
    fn rec(comb: &mut Vec<char>, start: usize, num: usize, types: &[char], results: &mut Vec<Vec<char>>) {
        if num == 0 {
            results.push(comb.clone());
            return;
        }
        for i in start..types.len() {
            comb.push(types[i]);
            rec(comb, i, num - 1, types, results);
            comb.pop();
        }
    }
    rec(&mut Vec::new(), 0, num_pairs, &types, &mut results);
    results
}

/// The backtracking search. For the given (ordered) placement options (one vector per piece pair),
/// choose one placement per pair so that no two pieces share a square. When a complete board is built,
/// run the filters and, if it qualifies, send the FEN to the provided sender.
fn search(
    index: usize,
    options: &Vec<Vec<Placement>>,
    occupied: &mut [bool; 64],
    current: &mut Vec<Placement>,
    sender: &Sender<String>,
) {
    if index == options.len() {
        let board = build_board(current);

        // Filter: exactly one black king.
        if board.iter().filter(|&&sq| sq == Some('k')).count() != 1 {
            return;
        }
        if is_piece_under_attack(&board) {
            return;
        }
        if can_deliver_check(&board) {
            return;
        }
        if has_passed_pawn(&board) {
            return;
        }
        if count_white_pawn_islands(&board) > 1 {
            return;
        }
        let fen = board_to_fen(&board);
        // Send the FEN string. (Ignore errors if the channel has been closed.)
        let _ = sender.send(fen);
        return;
    }
    for placement in &options[index] {
        let white_index = pos_to_index(placement.white_x, placement.white_y);
        let black_index = pos_to_index(placement.black_x, placement.black_y);
        if occupied[white_index] || occupied[black_index] {
            continue;
        }
        occupied[white_index] = true;
        occupied[black_index] = true;
        current.push(*placement);
        search(index + 1, options, occupied, current, sender);
        current.pop();
        occupied[white_index] = false;
        occupied[black_index] = false;
    }
}

/// Process one combination by building the placement options and launching the backtracking search.
/// Valid FEN strings are sent immediately via the provided sender.
fn process_combination(combination: &[char], sender: &Sender<String>) {
    let placements_options: Vec<Vec<Placement>> =
        combination.iter().map(|&p| generate_placements(p)).collect();
    let mut occupied = [false; 64];
    let mut current = Vec::new();
    search(0, &placements_options, &mut occupied, &mut current, sender);
}

/// Build a board (an array of 64 Option<char>) from the list of placements.
fn build_board(placements: &Vec<Placement>) -> BoardArray {
    let mut board = [None; 64];
    for &placement in placements {
        let w_index = pos_to_index(placement.white_x, placement.white_y);
        let b_index = pos_to_index(placement.black_x, placement.black_y);
        board[w_index] = Some(placement.white);
        board[b_index] = Some(placement.black);
    }
    board
}

/// Convert the board to a FEN string (only the piece–placement part plus " w - - 0 1").
fn board_to_fen(board: &BoardArray) -> String {
    let mut fen_rows = Vec::new();
    // FEN rows go from rank 8 (top) to rank 1 (bottom)
    for rank in (0..8).rev() {
        let mut row_str = String::new();
        let mut empty_count = 0;
        for file in 0..8 {
            let index = (rank * 8 + file) as usize;
            if let Some(piece) = board[index] {
                if empty_count > 0 {
                    row_str.push_str(&empty_count.to_string());
                    empty_count = 0;
                }
                row_str.push(piece);
            } else {
                empty_count += 1;
            }
        }
        if empty_count > 0 {
            row_str.push_str(&empty_count.to_string());
        }
        fen_rows.push(row_str);
    }
    fen_rows.join("/") + " w - - 0 1"
}

/// --- Minimal Chess Functions for Filtering ---

/// Returns the piece (if any) at board cell (x,y); (x,y) are 0-indexed.
fn get_piece_at(board: &BoardArray, x: i32, y: i32) -> Option<char> {
    if x < 0 || x >= 8 || y < 0 || y >= 8 {
        return None;
    }
    board[(y as usize) * 8 + (x as usize)]
}

/// Given two coordinates “from” and “to” and a piece, test whether that piece “attacks” the destination.
/// For sliding pieces the path must be clear.
fn piece_attacks(board: &BoardArray, from: (i32, i32), to: (i32, i32), piece: char) -> bool {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    match piece.to_ascii_lowercase() {
        'p' => {
            // Pawns attack diagonally.
            if piece.is_uppercase() {
                (dx == -1 && dy == 1) || (dx == 1 && dy == 1)
            } else {
                (dx == -1 && dy == -1) || (dx == 1 && dy == -1)
            }
        }
        'n' => {
            let knight_moves = [
                (2, 1), (1, 2), (-1, 2), (-2, 1),
                (-2, -1), (-1, -2), (1, -2), (2, -1)
            ];
            knight_moves.iter().any(|&(mx, my)| dx == mx && dy == my)
        }
        'k' => (dx.abs() <= 1 && dy.abs() <= 1) && (dx != 0 || dy != 0),
        'b' => {
            if dx.abs() == dy.abs() && dx != 0 {
                let step_x = dx.signum();
                let step_y = dy.signum();
                let mut x = from.0 + step_x;
                let mut y = from.1 + step_y;
                while (x, y) != to {
                    if get_piece_at(board, x, y).is_some() {
                        return false;
                    }
                    x += step_x;
                    y += step_y;
                }
                true
            } else {
                false
            }
        }
        'r' => {
            if (dx == 0 && dy != 0) || (dy == 0 && dx != 0) {
                let step_x = if dx == 0 { 0 } else { dx.signum() };
                let step_y = if dy == 0 { 0 } else { dy.signum() };
                let mut x = from.0 + step_x;
                let mut y = from.1 + step_y;
                while (x, y) != to {
                    if get_piece_at(board, x, y).is_some() {
                        return false;
                    }
                    x += step_x;
                    y += step_y;
                }
                true
            } else {
                false
            }
        }
        'q' => {
            // Queen = rook + bishop.
            if (dx.abs() == dy.abs() && dx != 0) {
                let step_x = dx.signum();
                let step_y = dy.signum();
                let mut x = from.0 + step_x;
                let mut y = from.1 + step_y;
                while (x, y) != to {
                    if get_piece_at(board, x, y).is_some() {
                        return false;
                    }
                    x += step_x;
                    y += step_y;
                }
                true
            } else if (dx == 0 && dy != 0) || (dy == 0 && dx != 0) {
                let step_x = if dx == 0 { 0 } else { dx.signum() };
                let step_y = if dy == 0 { 0 } else { dy.signum() };
                let mut x = from.0 + step_x;
                let mut y = from.1 + step_y;
                while (x, y) != to {
                    if get_piece_at(board, x, y).is_some() {
                        return false;
                    }
                    x += step_x;
                    y += step_y;
                }
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Returns true if any piece on the board is attacked by an opponent.
fn is_piece_under_attack(board: &BoardArray) -> bool {
    for y in 0..8 {
        for x in 0..8 {
            let pos = (x as i32, y as i32);
            if let Some(piece) = board[y * 8 + x] {
                // For each enemy piece, test if it attacks pos.
                for yy in 0..8 {
                    for xx in 0..8 {
                        let enemy_pos = (xx as i32, yy as i32);
                        if let Some(op) = board[yy * 8 + xx] {
                            if (piece.is_uppercase() && op.is_lowercase())
                                || (piece.is_lowercase() && op.is_uppercase())
                            {
                                if piece_attacks(board, enemy_pos, pos, op) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// --- White move generation ---
/// We define a simple Move struct and generate pseudo–legal moves for White.
#[derive(Clone, Copy)]
struct Move {
    from: (i32, i32),
    to: (i32, i32),
    piece: char,
}

/// Helper: test if a board cell is empty.
fn is_empty(board: &BoardArray, x: i32, y: i32) -> bool {
    if x < 0 || x >= 8 || y < 0 || y >= 8 {
        false
    } else {
        get_piece_at(board, x, y).is_none()
    }
}

/// Helper: test if the piece at (x,y) is an enemy relative to `piece`.
fn is_enemy(board: &BoardArray, x: i32, y: i32, piece: char) -> bool {
    if let Some(p) = get_piece_at(board, x, y) {
        (piece.is_uppercase() && p.is_lowercase()) || (piece.is_lowercase() && p.is_uppercase())
    } else {
        false
    }
}

/// Generate pseudo–legal moves for a given white piece at (x,y).
fn generate_moves_for_piece(board: &BoardArray, x: i32, y: i32, piece: char) -> Vec<Move> {
    let mut moves = Vec::new();
    match piece {
        'P' => {
            // White pawn: forward move.
            if y + 1 < 8 && is_empty(board, x, y + 1) {
                moves.push(Move { from: (x, y), to: (x, y + 1), piece });
                // Double move if on rank 2.
                if y == 1 && is_empty(board, x, y + 2) {
                    moves.push(Move { from: (x, y), to: (x, y + 2), piece });
                }
            }
            // Captures.
            for dx in [-1, 1].iter() {
                let nx = x + dx;
                let ny = y + 1;
                if nx >= 0 && nx < 8 && ny < 8 && is_enemy(board, nx, ny, piece) {
                    moves.push(Move { from: (x, y), to: (nx, ny), piece });
                }
            }
        }
        'N' => {
            let offsets = [
                (2, 1), (1, 2), (-1, 2), (-2, 1),
                (-2, -1), (-1, -2), (1, -2), (2, -1),
            ];
            for (dx, dy) in offsets.iter() {
                let nx = x + dx;
                let ny = y + dy;
                if nx >= 0 && nx < 8 && ny >= 0 && ny < 8 {
                    if get_piece_at(board, nx, ny).is_none() || is_enemy(board, nx, ny, piece) {
                        moves.push(Move { from: (x, y), to: (nx, ny), piece });
                    }
                }
            }
        }
        'B' => {
            let directions = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
            for (dx, dy) in directions.iter() {
                let mut nx = x + dx;
                let mut ny = y + dy;
                while nx >= 0 && nx < 8 && ny >= 0 && ny < 8 {
                    if get_piece_at(board, nx, ny).is_none() {
                        moves.push(Move { from: (x, y), to: (nx, ny), piece });
                    } else {
                        if is_enemy(board, nx, ny, piece) {
                            moves.push(Move { from: (x, y), to: (nx, ny), piece });
                        }
                        break;
                    }
                    nx += dx;
                    ny += dy;
                }
            }
        }
        'R' => {
            let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];
            for (dx, dy) in directions.iter() {
                let mut nx = x + dx;
                let mut ny = y + dy;
                while nx >= 0 && nx < 8 && ny >= 0 && ny < 8 {
                    if get_piece_at(board, nx, ny).is_none() {
                        moves.push(Move { from: (x, y), to: (nx, ny), piece });
                    } else {
                        if is_enemy(board, nx, ny, piece) {
                            moves.push(Move { from: (x, y), to: (nx, ny), piece });
                        }
                        break;
                    }
                    nx += dx;
                    ny += dy;
                }
            }
        }
        'Q' => {
            let directions = [
                (1, 0), (-1, 0), (0, 1), (0, -1),
                (1, 1), (1, -1), (-1, 1), (-1, -1),
            ];
            for (dx, dy) in directions.iter() {
                let mut nx = x + dx;
                let mut ny = y + dy;
                while nx >= 0 && nx < 8 && ny >= 0 && ny < 8 {
                    if get_piece_at(board, nx, ny).is_none() {
                        moves.push(Move { from: (x, y), to: (nx, ny), piece });
                    } else {
                        if is_enemy(board, nx, ny, piece) {
                            moves.push(Move { from: (x, y), to: (nx, ny), piece });
                        }
                        break;
                    }
                    nx += dx;
                    ny += dy;
                }
            }
        }
        'K' => {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if dx == 0 && dy == 0 { continue; }
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && nx < 8 && ny >= 0 && ny < 8 {
                        if get_piece_at(board, nx, ny).is_none() || is_enemy(board, nx, ny, piece) {
                            moves.push(Move { from: (x, y), to: (nx, ny), piece });
                        }
                    }
                }
            }
        }
        _ => {}
    }
    moves
}

/// Generate all pseudo–legal moves for White.
fn generate_white_moves(board: &BoardArray) -> Vec<Move> {
    let mut moves = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            if let Some(piece) = get_piece_at(board, x, y) {
                if piece.is_uppercase() {
                    moves.extend(generate_moves_for_piece(board, x, y, piece));
                }
            }
        }
    }
    moves
}

/// Make a move on a board copy.
fn make_move(board: &BoardArray, mv: Move) -> BoardArray {
    let mut new_board = *board;
    let from_index = (mv.from.1 as usize) * 8 + (mv.from.0 as usize);
    let to_index = (mv.to.1 as usize) * 8 + (mv.to.0 as usize);
    new_board[from_index] = None;
    new_board[to_index] = Some(mv.piece);
    new_board
}

/// Find White’s king (if any) on the board.
fn get_white_king(board: &BoardArray) -> Option<(i32, i32)> {
    for y in 0..8 {
        for x in 0..8 {
            if let Some(piece) = get_piece_at(board, x, y) {
                if piece == 'K' {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

/// Return true if, after some white move (taken from the pseudo–legal list
/// and after we discard moves that leave white king in check), White can deliver
/// a check on the opposing king.
fn can_deliver_check(board: &BoardArray) -> bool {
    let moves = generate_white_moves(board);
    for mv in moves {
        let new_board = make_move(board, mv);
        if let Some(_) = get_white_king(&new_board) {
            if white_king_in_check(&new_board) {
                continue;
            }
        }
        if black_king_in_check(&new_board) {
            return true;
        }
    }
    false
}


/// Check if White’s king is in check.
fn white_king_in_check(board: &BoardArray) -> bool {
    if let Some(pos) = get_white_king(board) {
        for y in 0..8 {
            for x in 0..8 {
                if let Some(piece) = get_piece_at(board, x, y) {
                    if piece.is_lowercase() && piece_attacks(board, (x, y), pos, piece) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check if Black’s king is in check.
fn black_king_in_check(board: &BoardArray) -> bool {
    let mut king_pos = None;
    for y in 0..8 {
        for x in 0..8 {
            if let Some(piece) = get_piece_at(board, x, y) {
                if piece == 'k' {
                    king_pos = Some((x, y));
                    break;
                }
            }
        }
        if king_pos.is_some() {
            break;
        }
    }
    if let Some(pos) = king_pos {
        for y in 0..8 {
            for x in 0..8 {
                if let Some(piece) = get_piece_at(board, x, y) {
                    if piece.is_uppercase() && piece_attacks(board, (x, y), pos, piece) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Return true if at least one pawn (either color) is passed.
fn has_passed_pawn(board: &BoardArray) -> bool {
    for y in 0..8 {
        for x in 0..8 {
            if let Some(piece) = get_piece_at(board, x, y) {
                if piece.to_ascii_uppercase() == 'P' {
                    if is_passed_pawn(board, x, y, piece) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// A pawn is passed if there is no enemy pawn in the three “files” ahead (for White)
/// or behind (for Black).
fn is_passed_pawn(board: &BoardArray, x: i32, y: i32, piece: char) -> bool {
    if piece.is_uppercase() {
        for ny in (y + 1)..8 {
            for nx in max(x - 1, 0)..=min(x + 1, 7) {
                if let Some(p) = get_piece_at(board, nx, ny) {
                    if p == 'p' {
                        return false;
                    }
                }
            }
        }
        true
    } else {
        for ny in 0..y {
            for nx in max(x - 1, 0)..=min(x + 1, 7) {
                if let Some(p) = get_piece_at(board, nx, ny) {
                    if p == 'P' {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// Count the number of White pawn “islands” (contiguous groups of files with at least one White pawn).
fn count_white_pawn_islands(board: &BoardArray) -> u32 {
    let mut files_with_pawn = [false; 8];
    for file in 0..8 {
        for rank in 0..8 {
            let index = rank * 8 + file;
            if let Some(p) = board[index] {
                if p == 'P' {
                    files_with_pawn[file] = true;
                    break;
                }
            }
        }
    }
    let mut islands = 0;
    let mut in_island = false;
    for &has_pawn in files_with_pawn.iter() {
        if has_pawn {
            if !in_island {
                islands += 1;
                in_island = true;
            }
        } else {
            in_island = false;
        }
    }
    islands
}

/// --- Main ---
///
/// This main function creates an MPSC channel and spawns a writer thread that
/// continuously writes received FEN strings to "stale_boards_6.fen". Then it uses Rayon to
/// process all piece–type combinations in parallel. As soon as a valid board is found its FEN
/// is sent (and written) immediately.
fn main() {
    let num_pairs = 6;
    println!("Generating critical boards for {} mirrored pairs…", num_pairs);
    
    // Generate piece–type combinations.
    let combinations = generate_combinations(num_pairs);

    // Create a channel to send FEN strings.
    let (tx, rx) = mpsc::channel::<String>();

    // Spawn a writer thread that writes FENs as they are received.
    let writer_handle = thread::spawn(move || {
        let file = std::fs::File::create("stale_boards_6.fen")
            .expect("Unable to create stale_boards_6.fen");
        let mut writer = std::io::BufWriter::new(file);
        for fen in rx {
            writeln!(writer, "{}", fen).expect("Failed to write to file");
        }
    });

    // Process combinations in parallel. Each thread gets its own clone of the sender.
    combinations.into_par_iter().for_each(|comb| {
        let local_tx = tx.clone();
        process_combination(&comb, &local_tx);
    });
    
    // Drop the original sender to signal completion.
    drop(tx);
    writer_handle.join().expect("Writer thread panicked");
}
