#!/usr/bin/env python3
"""
draw_checker.py

This script takes chess positions in FEN notation from an input file,
analyzes them with a UCI engine (e.g. Stockfish) at a given search depth,
and writes the engine’s evaluation (e.g. centipawn score or "mate") along
with the FEN to an output file.

This version uses multiprocessing to run many analyses concurrently,
so that it utilizes all available CPU cores.

Usage:
    python draw_checker.py [--engine /path/to/engine] [--depth 20] [--threshold 1]
    
The script expects two files:
  - "stale_boards_4.fen" : input file with one FEN per line.
  - "stale_boards_4_cp.txt" : output file (appended) with evaluations.
"""

import argparse
import chess
import chess.engine
import concurrent.futures
import os

def is_theoretical_draw(fen, engine_path, depth, threshold):
    """
    Analyzes the given FEN position using the specified UCI engine.
    Returns a centipawn score (or "mate") from the engine’s evaluation.
    
    (The function currently simply returns the evaluation of the last variation;
     you can add your drawn-detection logic if needed.)
    """
    board = chess.Board(fen)
    # Use the provided engine_path rather than hardcoding it.
    with chess.engine.SimpleEngine.popen_uci(engine_path) as engine:
        # Optionally, configure the engine to use only one thread per instance:
        # engine.configure({"Threads": 1})
        infos = engine.analyse(board, chess.engine.Limit(depth=depth), multipv=2)
        cp = None
        for info in infos:
            score = info["score"].relative
            if score.is_mate():
                return "mate"
            cp = score.score()
    return cp

def process_fen(fen, engine_path, depth, threshold):
    """
    Wrapper function for use in the process pool.
    Returns a tuple (fen, evaluation).
    """
    cp = is_theoretical_draw(fen, engine_path, depth, threshold)
    return (fen, cp)

def main():
    parser = argparse.ArgumentParser(
        description="Determine if positions are drawn with perfect play (heuristically)."
    )
    parser.add_argument("--engine", default="stockfish",
                        help="Path to the UCI engine binary (default: stockfish).")
    parser.add_argument("--depth", type=int, default=50,
                        help="Search depth for engine analysis (default: 20).")
    parser.add_argument("--threshold", type=int, default=1,
                        help="Centipawn threshold (default: 1).")
    args = parser.parse_args()


    # Load FENs already processed (if any) to avoid re-processing.
    explored = set()
    # try:
    #     with open("challengers.txt", "r") as f:
    #         for line in f:
    #             parts = line.strip().split("\t")
    #             if len(parts) >= 2:
    #                 explored.add(parts[1])
    # except FileNotFoundError:
    #     pass  # If the output file doesn't exist yet, that's fine.

    # Read FEN positions from the input file, skipping ones already processed.
    fens_to_process = []
    with open("challengers.txt", "r") as f:
        for line in f:
            fen = line.strip()
            if fen and fen not in explored:
                fens_to_process.append(fen)

    # Open the output file for appending.
    with open("stale_boards_4.fen", "a") as outfile:
        # Use ProcessPoolExecutor to run analyses in parallel.
        workers = os.cpu_count()//2
        print(f"Using {workers} workers")
        with concurrent.futures.ProcessPoolExecutor(max_workers=workers) as executor:
            # Submit all tasks to the pool.
            future_to_fen = {
                executor.submit(process_fen, fen, args.engine, args.depth, args.threshold): fen
                for fen in fens_to_process
            }
            count = 0
            for future in concurrent.futures.as_completed(future_to_fen):
                fen = future_to_fen[future]
                count += 1
                try:
                    fen_result, cp = future.result()
                except Exception as exc:
                    print(f"Error processing {fen}: {exc}")
                    continue
                if cp is not None:
                    out_line = f"{cp}\t{fen}\n"
                    if cp != 0:
                        print(f"{count}\t{out_line.strip()}")
                    outfile.write(out_line)
                else:
                    print(f"Error: {fen}")

if __name__ == '__main__':
    main()
