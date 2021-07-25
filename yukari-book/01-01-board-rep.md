# Chapter 1: Board Representation

> I would like to thank:
> - Andrew Tridgell for [his post](https://groups.google.com/g/rec.games.chess.computer/c/g0-jwnP6__4/m/1AyOjo4ybE0J) about how KnightCap's board representation works.
> - Marcel van Kervinck for [his move generator](https://github.com/kervinck/rookiegen) which I received permission to look at.
> - H. G. Muller for [his thread](http://talkchess.com/forum3/viewtopic.php?f=7&t=76773) on writing a very fast search-optimised move generator.
>
> Yukari's board representation borrows much inspiration from all of these, and I will use their posts and code to discuss alternative approaches.

In order to describe how Yukari's board representation works, I think it's best to build up from first principles, and discuss strengths and address weaknesses as I go.

## An 8x8 array and the attacks-to-square problem

When the idea of a chessboard comes to mind, an 8x8 array is probably what most people think of. Each entry in the array indicates the presence of a piece, its type and
colour. To make a move on the board, one overwrites the contents of the destination square of the move with the contents of the source square, and then clears the source
square.

The biggest strength of this is in its simplicity: there's only one data structure that acts as the reference for everything. To answer "is this square occupied?" requires just indexing the array at that square to check if it has a piece.

Unfortunately, the 8x8 board by itself has a number of weaknesses:
- Finding the attacks to a given square is expensive. This is needed for in-check detection (where you see if there are any attacks to the square of your king), and also when seeing if a side can castle, because the king cannot castle through attacked squares.
- Finding the square of a specific piece is also expensive.

I think it's best to demonstrate this with the example of an algorithm for seeing if the side to move is in check.

In order to see if you are in check, you first have to iterate over the board to discover the king square.
The worst case here where the king is the very last square you look at.

Next you have to test for attacks by enemy sliders. This is achieved by iterating in each of the 8 compass directions by adding an increment to the square.
Further, due to the potential to wrap around files, each loop has to check if the next square would wrap around the board. It's quite slow and does not look pleasant.
Checking for sliders has the worst case when the king is central in the board and there is no piece in any direction, because every direction has a handful of squares
to examine.

Having checked for sliders, one now has to check for attacks by knights, pawns and kings. This again requires a lot of "look before you leap" to avoid wrapping around
the board.

The resulting code is slow and difficult to maintain.

## 16x8 ("0x88") square coordinates

One way to reframe the answer to the attacks-to-square problem is to look at each enemy piece on the board and ask
- whether you can get from the square that piece stands on to the target square you need to check for attacks.
- what direction (if any) you need to travel in to get from the piece's square to the target square.
- whether that direction is a valid movement direction for the piece that stands on it (e.g. rooks can't move diagonally, and so do not attack squares diagonal to it).

One problem with 8x8 coordinates is they suffer from file wrap-around, and thus do not one-to-one map to directions. If you try to subtract two squares and get a difference
of +1, this could be one square east (A1 to B1) or it could wrap around the board (H1 to A2).

The maximum valid movement along a file is from the A-file to the H-file, which is +7. If A1 is 0 and H1 is 7, we need to add 7 to H1 to get a board with at least 15 files
(0-14) to remove this ambiguity. I picked 16 files, because converting from 8x8 space to 16x8 space is much easier than to 15x8 space.

Now we can compute a table indexed by the difference between a destination and source square (A1 minus H8 is -119 in 16x8 space, so we add an offset of 119 to ensure all
indices are positive), and see if there is a direction between these two squares. As a personal note, I think it's best not to include knight directions in this array,
because it can be elegantly handled separately.

16x8 space has another useful property: if a target square is above the 128-square board, its index has bit `0x80` (the 128s bit) set. If it goes below the board, it
becomes negative and also has `0x80` set. If a target square is on one of the invalid files, it has bit `0x8` (the 8s bit) set. Thus, one can check if a square is off the
board if ANDing it with 0x88 is non-zero.

With all of this in mind, let's rewrite the code for testing if a square is attacked.

- Iterate over the board to discover the king square (this is still slow, unfortunately, but we will address this).
- Then iterate over the board again, and for each enemy piece you find:
  - if it's a slider, check if there's a direction between the piece and the king; check if that direction is valid for the slider; then travel along the direction to see if there are any blocking pieces.
  - if it's a leaper (king, knight, pawn), iterate over an array of directions for that piece, skipping if `(piece_square + direction) & 0x88 != 0` and then check if the target square is the king.

The resulting code is a lot nicer, but it still requires iterating over the board twice.

> Yukari uses an 8x8 board that converts to and from 16x8 board coordinates when necessary.
>
> As I understand it, Rookie and KnightCap work purely in 8x8 board space, by having arrays indexed by from-square and to-square.
> These arrays can get quite big in comparison to the same array indexed by 16x8 square difference, so I chose the latter.
>
> HGM's engine has a 16x12 board space, with a 2-square rim of invalid squares along the top and bottom of a 16x8 board, but it has a pointer relative to the A1 square
> of that 16x12 board to create a 16x8 board, relying on encountering the invalid square rim instead of using the 0x88 trick.

## Piece lists

The 8x8 board can be thought of as an array to answer "for each square, what piece is on it?".
A good complement to this is an array to answer "for each piece, what square is it on?". This is a "piece list".

Since the starting position has 32 pieces, we can have a 32-entry array, and each entry contains the square of a piece, its type and its colour, if there's an entry here.

This requires additional work over just an 8x8 board, because you need to find the entry that corresponds to a piece, and either you accept holes in the piece list and
potentially require traversing the entire piece list to check for the presence of a piece, or shuffle entries when pieces are removed to speed this up.

Neither option is ideal, but whichever you choose, it's worth noting that it's faster to iterate over the 32-entry piece list than the 64-entry board.

Let's rewrite the code for testing if a square is attacked again.

- Iterate over the piece list to discover the king square (one may choose to store the king in a known location in the piece list to accelerate this).
- Then iterate over the piece list again, and for each enemy piece you find:
  - if it's a slider, check if there's a direction between the piece and the king; check if that direction is valid for the slider; then travel along the direction to see if there are any blocking pieces.
  - if it's a leaper (king, knight, pawn), iterate over an array of directions for that piece, skipping if `(piece_square + direction) & 0x88 != 0` and then check if the target square is the king.

This is arguably fast enough for the average person to be happy.

## Piece masks

But of course it can go faster.

Let's imagine that each piece is assigned a unique index from 0 to 31. We'll reserve 0 to 15 for the white pieces and 16 to 31 for the black pieces.
We will store a piece's index (rather than its type) in the 8x8 board; a piece's colour is implicit by its index. The Nth piece list entry is for piece index N.

Now we need a handful of 32-bit masks, with bit N of the mask indicating the presence or absence of the piece with index N.
A good idea is to have one mask for each piece type, although Yukari optimises this for size.

When piece index N is removed, we clear the piece list entry corresponding to index N, and clear the Nth bit from the piece masks.

Modern CPUs have special instructions for finding the lowest-valued one bit in a 32-bit mask. I will call that operation "bit scan forward" as `bsf` is the name of the x86
instruction which achieves this.

If you want to find a particular subset of the possible pieces on the board, you can just AND masks togther.
For example, finding the set of all white pawns is done by ANDing together the pawn piece mask with the white colour mask.
Then using bit scan forward, we can hop through the subset to find the indices of pieces, which can be converted to squares through the piece list.
This also means we don't need to shuffle piece list entries anymore: we just skip over them.

Let's continue with the example of testing if a square is attacked again.

- Find the king of the current side. Use bit scan forward to find its piece index, use the piece list to find its square.
- Find the subset of bishops, rooks and queens of the opponent's side. Use bit scan forward to iterate through them. For each piece:
  - Check if there's a direction between the piece and the king; check if that direction is valid for the slider; then travel along the direction to see if there are any blocking pieces.
- Find the subset of pawns, knights, and king of the opponent's side. Use bit scan forward to iterate through them. For each piece:
  - Iterate over an array of directions for that piece, skipping if `(piece_square + direction) & 0x88 != 0` and then check if the target square is the king.

This is definitely fast enough.

> KnightCap and HGM's engine both use fixed piece masks that allocate specific locations for specific pieces.
> This has the advantage of being able to extract pieces using constant masks rather than keeping them as part of the board data structure.
>
> In particular, HGM's engine explicitly stores these from lowest value to highest value to make capture generation produce captures
> in the order of least valuable attacker capturing most valuable victim.
> I chose not to do this, because in very rare positions you might be able to promote multiple pawns and then run out of slots in the piece mask for it.
>
> Another thing worth mentioning is that while Yukari uses 0-31 as valid piece indices (actually 1-32 so it can use 0 as empty square), HGM's engine uses 16-47 as valid
> piece indices. This uses the 16s bit to represent white pieces and the 32s bit to represent black pieces. The invalid square rim mentioned in the 16x8 square coordinates
> section has the value 48, so that testing for hitting a piece or the edge of the board can be done by ANDing the contents of the target square with `0x30`, and checking if
> the result is nonzero.

## Attack tables

But of *course* it can go faster.

Let's say we have another 8x8 board, and each entry on this board is a 32-bit mask of which pieces attack this square.
We can compute subsets of attacks on a square using the piece and colour masks. This is called an attack table.

This requires a bit of thought to implement correctly, but when you do, it makes the problem of finding the attacks to a square simple.

- Find the king of the current side. Use bit scan forward to find its piece index, use the piece list to find its square.
- Find the subset of opponent attacks to the king's square by looking up the attack table.

And that's it. There are no loops here, just array lookups and masking off bits.

Yukari uses all of the techniques listed above, as they all complement each other nicely.

> Yukari follows KnightCap's approach here, which I think gives the most information at the cost of space.
>
> HGM instead has an attack mask for every *piece* on the board, which reduces space needed to store the attack table at the cost of needing to calculate attacks again for
> squares without pieces on.
>
> Rookie's attack table stores attacks to a square not by piece index, but by direction: 8 bits for the 8 compass directions, plus 8 bits for the 8 knight directions.
> The cost of this approach is that one needs to walk in a given direction to find the attacking piece.