# slc replay format

A tiny and incredibly fast replay format for Geometry Dash.

Requires Rust version `1.86.0` or higher.

## Documentation

For documentation, please refer to the [original slc repo](https://github.com/silicate-bot/slc).

## Example

### Loading a Replay

```rust
use slc_oxide::{Replay, Meta};
use std::fs::File;
use std::io::BufReader;

// Load any replay file - automatically detects slc version
let file = File::open("replay.slc")?;
let mut reader = BufReader::new(file);
let replay = Replay::<()>::read(&mut reader)?;

println!("TPS: {}", replay.tps);
println!("Input count: {}", replay.inputs.len());
```

### Creating and Saving a Replay

```rust
use slc_oxide::{Replay, InputData, PlayerInput};
use std::fs::File;
use std::io::BufWriter;

struct ReplayMeta {
  pub seed: u64
}

let mut replay = Replay::<ReplayMeta>::new(
  240.0, 
  ReplayMeta { 
    seed: 1234 
  }
);

// OR

let mut replay = Replay::<()>::new(240.0, ());

replay.add_input(200, InputData::Player(PlayerInput {
  button: 1,
  hold: true,
  player_2: false
}));

// Other input types
replay.add_input(400, InputData::Death);
replay.add_input(600, InputData::TPS(480.0));

// Save the replay in v2 format (default)
let file = File::create("replay_v2.slc")?;
let mut bw = BufWriter::new(file);
replay.write(&mut bw)?;

// Or save in v3 format
let file = File::create("replay_v3.slc")?;
let mut bw = BufWriter::new(file);
replay.write_v3(&mut bw)?;
```
