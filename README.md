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



Included are the stale boardstates for 2, 4, 6, and even 8 pieces.
Included is the python code I used to generate the stale boards. It is not optimised at all.
Ideally someone who is more skilled at engine search could generate more stale boards and verify whether this lemma holds for 8 pieces +

**[Update]**  
I used stockfish with a depth of 20 to just get a vibe on how strong this lemma is on 4 pair boards, 
From this I found about 500 exceptions where this does not hold.

If we expand the condition (no trapped pieces) to include no discoverable threats, then the lemma holds for boards with 4 pairs or less.


o1 DeepResearch insights: https://chatgpt.com/share/67a412ff-6c68-8013-a391-04625f09a8af
