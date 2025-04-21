# Yukari 💜

Yukari is a primarily xboard-protocol chess program.
(UCI support exists, but xboard should be preferred if your GUI isn't from the stone age.)

## important notice

yukari is currently being [rewritten](https://github.com/yukarichess/yukari-rewrite).
future development will occur in that repository, which may be moved to this location.

## how do I build it?

Yukari uses nightly Rust features to accelerate NNUE evaluation without the author's eyes bleeding, and `cargo-pgo` to automate a profile-guided optimisation run.

```
rustup component add llvm-tools
cargo install cargo-pgo
make
```

## how strong is it?

- 2025.4.1: approximately 3450 CCRL 40/4
- 2025.3.4: 3376 CCRL 40/4; 3329 CCRL 40/40
- 2025.2.4: 3081 CCRL 40/4; 3103 CCRL 40/40
- 2024.12.1: 2825 CCRL 40/4; 2824 CCRL 40/40

## thanks to:

in no particular order:
- @Lunaphied who wrote the xboard protocol code
- @87flowers for motivating me to pick up Yukari after three years and for running lots of Yukari datagen
- @analog-hors for patiently debugging my transposition table code
- @dannyhammer for hosting and sharing the Toad OpenBench instance
- @Ciekce for guiding the NNUE implementation
- @JonathanHallstrom for running Yukari datagen
- Jim Ablett, for building binaries
- the Loftycord, for being amazing friends

sometimes you *can* teach an old wolf new tricks.
