# Yukari 💜

Yukari is a primarily xboard-protocol chess program.
(UCI support exists, but xboard should be preferred if your GUI isn't from the stone age.)

## how do I build it?

Yukari uses nightly Rust features to accelerate NNUE evaluation without the author's eyes bleeding, and `cargo-pgo` to automate a profile-guided optimisation run.

```
rustup component add llvm-tools
cargo install cargo-pgo
make
```

## how strong is it?

- 2025.11.1: 3540 CCRL 40/4; 3452 CCRL 40/40
- 2025.4.1: 3507 CCRL 40/4; 3423 CCRL 40/40
- 2025.3.4: 3370 CCRL 40/4; 3318 CCRL 40/40
- 2025.2.4: 3075 CCRL 40/4; 3102 CCRL 40/40
- 2024.12.1: 2822 CCRL 40/4; 2835 CCRL 40/40

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
