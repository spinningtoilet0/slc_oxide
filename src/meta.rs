/// A trait for Meta objects in slc.
///
/// This trait provides a simple way to serialize slc metas
/// to and from byte packets.
///
/// The () type has an existing implementation. It is considered
/// a zero-sized type by the format.
///
/// # Examples
/// ```
/// use slc_oxide::meta::Meta;
///
/// struct ReplayMeta {
///     seed: u64
/// }
///
///
/// impl Meta for ReplayMeta {
///     fn size() -> u64 {
///         8
///     }
///
///     fn from_bytes(bytes: &[u8]) -> Self {
///         let mut seed_buf = [0u8; 8];
///         seed_buf.copy_from_slice(&bytes[0..7]);
///         Self {
///             seed: u64::from_le_bytes(seed_buf),
///         }
///     }
///
///     fn to_bytes(&self) -> Box<[u8]> {
///         let bytes = self.seed.to_le_bytes();
///
///         Box::new(bytes)
///     }
/// }
/// ```
pub trait Meta {
    /// Size of the meta object.
    ///
    /// The size is provided in bytes.
    fn size() -> u64;

    /// Converts a slice of bytes to the meta object.
    ///
    /// When implementing this method, you may safely assume
    /// bytes will match the size provided by the `Meta::size` method.
    fn from_bytes(bytes: &[u8]) -> Self;

    /// Converts a meta object to an array of bytes, heap-allocated.
    fn to_bytes(&self) -> Box<[u8]>;
}

impl Meta for () {
    fn size() -> u64 {
        0
    }

    fn from_bytes(_bytes: &[u8]) -> Self {}

    fn to_bytes(&self) -> Box<[u8]> {
        Box::new([])
    }
}
