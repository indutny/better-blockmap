pub const DEFAULT_WINDOW_SIZE: usize = 64;

#[derive(Debug)]
pub struct Table {
    pub shift: [u64; 256],
    pub drop: [u64; 256],
}
