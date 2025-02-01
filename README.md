# StaleChess
An public project where I am attempting to find properties of a chess board that guarentees a draw. 

Inspired by this prediciton market: https://manifold.markets/IsaacKing/will-chess-be-solved-by-2040
For fun and to ever so slightly accelerate the resolution, I used an LLM to draft small programs to search syzygy's 6 piece table base for useful conditioned properties that guarantee a draw.

From this I got the lemma:

For a chess board with 6 pieces or less, perfect play results in a draw if all conditions below are satisfied:

 - the position is mirrored

 - there are no attacking pieces

 - there are no checks

 - there are no passed pawns

 - there is a single pawn island

 - There are no trapped pieces, ex: [ rk6/b7/8/8/8/8/B7/RK6 w - - 0 1 ]

The starting position satisfies all of these conditions, so it will be interesting to see if this lemma holds for 8 pieces, 10 pieces, etc.

Trust this as far as you trust auto-generated code.
