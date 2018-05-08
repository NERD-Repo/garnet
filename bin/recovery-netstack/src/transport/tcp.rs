pub enum TcpOption {
    Mss(u16),
    WindowScale(u8),
    SackPermitted,
    Sack {
        blocks: [TcpSackBlock; 4],
        num_blocks: u8,
    },
    Timestamp {
        ts_val: u32,
        ts_echo_reply: u32,
    },
}

#[derive(Copy, Clone, Default)]
pub struct TcpSackBlock {
    pub left_edge: u32,
    pub right_edge: u32,
}
