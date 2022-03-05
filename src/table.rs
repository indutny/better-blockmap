pub const DEFAULT_WINDOW_SIZE: usize = 64;

#[cfg(any(feature = "window_size"))]
pub const POLYNOMIAL: u64 = 0xbfe6b8a5bf378d83;

#[derive(Debug)]
pub struct Table {
    pub shift: [u64; 256],
    pub drop: [u64; 256],
}
