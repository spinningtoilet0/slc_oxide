#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueAtom {
    pub id: u32,
    pub flags: u8,
    pub body: Vec<u8>,
}

impl OpaqueAtom {
    pub fn new(id: u32, flags: u8, body: Vec<u8>) -> Self {
        Self { id, flags, body }
    }
}
