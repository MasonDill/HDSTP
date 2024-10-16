use std::time::{Duration, Instant};

pub enum PacketType {
    // Data packets (0b0XXX)
    Data = 0b0001,
    Retransmission = 0b0010,

    // Control packets (0b1xxx)
    Syn = 0b1000,
    SynAck = 0b1001,
    Ack = 0b1010,
    Nak = 0b1011,
    Fin = 0b1100,
    Rst = 0b1101
}

pub struct Packet {
    length: u32,
    packet_type: PacketType,
    sequence_no: u32,
    data: Vec<u8>,
    parity: u8,
}

impl Packet {
    fn new(packet_type: PacketType, sequence_no: u32, data: Vec<u8>) -> Self {
        let length = (data.len() + 9) as u32; // 4 bytes for length + 1 byte for type + 4 bytes for sequence no + parity
        let parity = Self::check_parity(&data);
        Self { length, packet_type, sequence_no, data, parity }
    }

    // Function to calculate parity (even parity)
    fn check_parity(data: &[u8]) -> u8 {
        let ones_count = data.iter().map(|&byte| byte.count_ones()).sum::<u32>() % 2;
        if ones_count == 0 { 0b0000 } else { 0b1111 }
    }
}