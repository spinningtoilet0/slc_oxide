pub struct Replay {}

impl Replay {
    pub const HEADER: [u8; 4] = *b"SILL";
    pub const FOOTER: [u8; 3] = *b"EOM";
}
