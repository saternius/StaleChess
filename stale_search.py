import chess
import itertools
from multiprocessing import Pool, cpu_count
from tqdm import tqdm
from collections import defaultdict

# ------------------------------------------------------------------------------
# Filtering / Utility Functions
# ------------------------------------------------------------------------------

def is_piece_under_attack(board: chess.Board) -> bool:
    """
    Return True if at least one piece on the board is under attack.
    """
    for square in chess.SQUARES:
        piece = board.piece_at(square)
        if piece and board.is_attacked_by(not piece.color, square):
            return True
    return False

def can_deliver_check(board: chess.Board) -> bool:
    """
    Return True if the side to move (always White in this FEN construction)
    can deliver a check on the next move.
    """
    for move in board.legal_moves:
        board.push(move)
        if board.is_check():
            board.pop()
            return True
        board.pop()
    return False

def has_passed_pawn(board: chess.Board) -> bool:
    """
    Return True if the position has at least one passed pawn.
    """
    for square in chess.SQUARES:
        piece = board.piece_at(square)
        if piece and piece.piece_type == chess.PAWN:
            if is_passed_pawn(board, square):
                return True
    return False

def is_passed_pawn(board: chess.Board, square: chess.Square) -> bool:
    """
    Return True if the pawn on 'square' is a passed pawn.
    """
    piece = board.piece_at(square)
    if not piece or piece.piece_type != chess.PAWN:
        return False
    color = piece.color
    file = chess.square_file(square)
    rank = chess.square_rank(square)
    if color == chess.WHITE:
        ranks_to_check = range(rank + 1, 8)
        opp_color = chess.BLACK
    else:
        ranks_to_check = range(rank - 1, -1, -1)
        opp_color = chess.WHITE

    for r in ranks_to_check:
        for f in [file - 1, file, file + 1]:
            if 0 <= f < 8:
                opp_piece = board.piece_at(chess.square(f, r))
                if opp_piece and opp_piece.color == opp_color and opp_piece.piece_type == chess.PAWN:
                    return False
    return True

def has_pawn_on_first_or_eighth(board: chess.Board) -> bool:
    """
    Return True if there is a pawn on rank 1 or rank 8.
    """
    first_rank_squares = [chess.square(file, 0) for file in range(8)]
    eighth_rank_squares = [chess.square(file, 7) for file in range(8)]
    squares_to_check = first_rank_squares + eighth_rank_squares
    for sq in squares_to_check:
        piece = board.piece_at(sq)
        if piece and piece.piece_type == chess.PAWN:
            return True
    return False

def count_pawn_islands(board: chess.Board):
    """
    Return (white_islands, black_islands).
    """
    # Build file-wise lists of ranks where pawns exist
    white_pawns = [[] for _ in range(8)]
    black_pawns = [[] for _ in range(8)]

    for file in range(8):
        for rank in range(8):
            piece = board.piece_at(chess.square(file, rank))
            if piece and piece.piece_type == chess.PAWN:
                if piece.color == chess.WHITE:
                    white_pawns[file].append(rank)
                else:
                    black_pawns[file].append(rank)

    def calc_islands(pawn_files):
        islands = 0
        in_island = False
        for file_pawns in pawn_files:
            if file_pawns:  # at least one pawn in this file
                if not in_island:
                    islands += 1
                    in_island = True
            else:
                in_island = False
        return islands

    return calc_islands(white_pawns), calc_islands(black_pawns)

def board_to_fen(board_dict):
    """
    Build a FEN string from a dict {square: piece_symbol},
    where square is like 'a1', piece_symbol is in ['P','N','B','R','Q','K','p','n','b','r','q','k'].
    """
    board = [['' for _ in range(8)] for _ in range(8)]
    for sq, piece in board_dict.items():
        file = ord(sq[0]) - ord('a')
        rank = int(sq[1]) - 1
        board[7 - rank][file] = piece

    fen_rows = []
    for row in board:
        empty = 0
        fen_row = ""
        for cell in row:
            if cell == '':
                empty += 1
            else:
                if empty:
                    fen_row += str(empty)
                    empty = 0
                fen_row += cell
        if empty:
            fen_row += str(empty)
        fen_rows.append(fen_row)

    return "/".join(fen_rows) + " w - - 0 1"

# ------------------------------------------------------------------------------
# Main Generation Logic
# ------------------------------------------------------------------------------

def generate_positions_for_combination(piece_types_pair):
    """
    Given a tuple of piece types (e.g. ('P','K')), generate *mirrored* boards
    with those pairs, filter out anything that doesn't meet the criteria, and
    return the valid FEN strings.
    """
    piece_types = {
        'P': (2, 'P'),  'N': (1, 'N'),  'B': (1, 'B'),
        'R': (1, 'R'),  'Q': (1, 'Q'),  'K': (1, 'K'),
    }
    # black piece starts
    piece_types_black = {
        'P': (7, 'p'),  'N': (8, 'n'),  'B': (8, 'b'),
        'R': (8, 'r'),  'Q': (8, 'q'),  'K': (8, 'k'),
    }
    files = ['a','b','c','d','e','f','g','h']
    ranks = range(1,9)

    # Build all squares for each piece in the pair
    possible_positions_per_pair = []
    for pt in piece_types_pair:
        srw, white_symbol = piece_types[pt]
        srb, black_symbol = piece_types_black[pt]

        # Collect all mirrored placements (white_sq, black_sq) for this piece type
        pos_list = []
        for f in files:
            for rw in ranks:
                # We skip White squares beyond rank 4 (since no White piece can go past mid-zone)
                if rw > 4:
                    continue
                # If it's a White pawn, skip rank=1 or rank=8
                if white_symbol == 'P' and (rw == 1 or rw == 8):
                    continue

                distance = abs(rw - srw)
                rb_candidates = [srb + distance, srb - distance]
                for rb in rb_candidates:
                    if 1 <= rb <= 8:
                        # We skip black squares below rank 5 (no black piece can be in ranks 1..4)
                        if rb < 5:
                            continue
                        # If it's a Black pawn, skip rank=1 or rank=8
                        if black_symbol == 'p' and (rb == 1 or rb == 8):
                            continue

                        pos_list.append(((f, rw, white_symbol),
                                         (f, rb, black_symbol)))
        possible_positions_per_pair.append(pos_list)

    valid_fens = []
    fen_set = set()
    # Cartesian product across all pairs
    for combo in itertools.product(*possible_positions_per_pair):
        squares = set()
        board_dict = {}
        valid = True

        # Check for collisions in squares
        for (wf, wr, wsym), (bf, br, bsym) in combo:
            wsq = f"{wf}{wr}"
            bsq = f"{bf}{br}"
            if wsq in squares or bsq in squares:
                valid = False
                break
            squares.add(wsq)
            squares.add(bsq)
            board_dict[wsq] = wsym
            board_dict[bsq] = bsym

        if not valid:
            continue

        # Build FEN
        fen = board_to_fen(board_dict)
        if fen in fen_set:
            continue
        fen_set.add(fen)
        
        # Make sure there's exactly 1 black king in the position
        # (the original code used `'k' in fen and len(fen.split('k')) == 2`)
        if fen.count('k') != 1:
            continue

        # -- Build a Board from fen and run all checks in one pass --
        board = chess.Board(fen)
        
        # 1) No pieces under attack
        if is_piece_under_attack(board):
            continue
        # 2) Cannot deliver check immediately
        if can_deliver_check(board):
            continue
        # 3) No passed pawns
        if has_passed_pawn(board):
            continue
        # 4) No pawn on first or eighth
        if has_pawn_on_first_or_eighth(board):
            continue
        # 5) Single pawn island for White (ignore black's count as per original code)
        white_islands, black_islands = count_pawn_islands(board)
        if white_islands > 1:
            continue

        valid_fens.append(fen)

    return valid_fens


def get_critical_boards(num_pairs=3):
    """
    Generate all mirrored positions (for up to `num_pairs` pieces),
    filter them by the given criteria, and return the final FEN list.
    """
    # All piece types that can appear
    piece_types = ['P', 'N', 'B', 'R', 'Q', 'K']

    # Generate all (multi-)combinations of these piece types of length `num_pairs`.
    # e.g. for num_pairs=2: (('P','P'), ('P','N'), ('P','B'), ..., ('K','K'))
    piece_type_combinations = list(itertools.combinations_with_replacement(piece_types, num_pairs))

    # Parallel processing
    cpu_cores = min(cpu_count(), 32)
    results = []
    with Pool(cpu_cores) as pool:
        # If you want a global progress bar:
        # chunk the list of combinations and process them in parallel
        chunk_size = max(1, len(piece_type_combinations) // cpu_cores)
        for chunk_start in tqdm(range(0, len(piece_type_combinations), chunk_size),
                                desc="Overall progress"):
            chunk = piece_type_combinations[chunk_start:chunk_start+chunk_size]
            # Map the function to each combination in this chunk
            partial_results = pool.map(generate_positions_for_combination, chunk)
            # Flatten the chunk results
            for fen_list in partial_results:
                results.extend(fen_list)

    return results

# ------------------------------------------------------------------------------
# Example usage
# ------------------------------------------------------------------------------
if __name__ == "__main__":
    critical_boards = get_critical_boards(num_pairs=5)
    print(f"Found {len(critical_boards)} critical boards.")
    # Print out the first few:
    outfile = open("parallel_critical_boards_5.fen", "a+")
    for fen in critical_boards:
        outfile.write(fen + "\n")
    outfile.close()
