# quad-snd 0.2.8, patched for Minceraft

This is [quad-snd](https://github.com/not-fl3/quad-snd) 0.2.8 (MIT/Apache-2.0,
by Fedor Logachev), used through macroquad's audio. Minceraft points Cargo at it
with `[patch.crates-io]`. Only `src/mixer.rs` is changed (each change is marked
"Minceraft patch"); the examples were left out.

- **A limiter on the mix bus.** The mix was a plain sum, so loud moments went
  over full scale and clipped. Now the level dips smoothly when it gets too hot
  (about -1 dBFS) and anything left over is rounded off below 1.0.
- **Linear resampling** instead of nearest-neighbour, for any sound that isn't
  at 44.1 kHz. (Minceraft's own sounds already are; see `src/engine/sound.rs`.)
