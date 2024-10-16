use std::time::{Duration, Instant};
use std::collections::VecDeque;
use tokio::sync::Notify;

#[derive(Copy, Clone, PartialEq)]
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
        let parity = check_parity(&data);
        Self { length, packet_type, sequence_no, data, parity }
    }
}

// Function to calculate parity (even parity)
fn check_parity(data: &[u8]) -> u8 {
    let ones_count = data.iter().map(|&byte| byte.count_ones()).sum::<u32>() % 2;
    if ones_count == 0 { 0b0000 } else { 0b1111 }
}

pub struct PacketStream {
    buffer: VecDeque<Packet>,
    notify: Notify,
}

impl PacketStream {
    pub fn new() -> Self {
        PacketStream {
            buffer: VecDeque::new(),
            notify: Notify::new(),
        }
    }

    pub fn write(&mut self, packet: Packet) {
        self.buffer.push_back(packet);
        self.notify.notify_one();
    }

    pub async fn read(&mut self) -> Option<Packet> {
        self.notify.notified().await;
        let packet = if self.buffer.is_empty() {
            None
        } else {
            Some(self.buffer.pop_front().unwrap())
        };

        packet
    }
}

pub struct Server {
    packet_stream: PacketStream,
    connected: bool,
    expected_sequence_no: u32,
}

impl Server {
    pub fn new(packet_stream: PacketStream) -> Self {
        Self {
            packet_stream,
            connected: false,
            expected_sequence_no: 0,
        }
    }

    // Handle connection setup (3-way handshake)
    pub async fn setup(&mut self) {
        // 1. Wait for SYN
        if let Some(packet) = self.packet_stream.read().await {
            if packet.packet_type == PacketType::Syn {
                // Respond with SYN-ACK
                let syn_ack_packet = Packet::new(PacketType::SynAck, self.expected_sequence_no, vec![packet.sequence_no as u8]);
                self.packet_stream.write(syn_ack_packet);
                self.expected_sequence_no += 1;

                // Wait for ACK
                if let Some(packet) = self.packet_stream.read().await {
                    if packet.packet_type == PacketType::Ack {
                        // Connection established
                        self.connected = true;
                        println!("Connection established.");
                    }
                }
            }
        }
    }

    // Handle data transmission
    pub async fn handle_data(&mut self) {
        while self.connected {
            if let Some(packet) = self.packet_stream.read().await {
                match packet.packet_type {
                    PacketType::Data => {
                        // Process data packet
                        println!("Received data: {:?}", packet.data);
                        let ack_packet = Packet::new(PacketType::Ack, self.expected_sequence_no, vec![]);
                        self.packet_stream.write(ack_packet);
                        self.expected_sequence_no += 1;
                    }
                    PacketType::Fin => {
                        // Handle FIN packet
                        let fin_ack_packet = Packet::new(PacketType::Ack, self.expected_sequence_no, vec![]);
                        self.packet_stream.write(fin_ack_packet);
                        self.connected = false; // Terminate connection
                        println!("Connection terminated.");
                    }
                    _ => {
                        // Recieved unexpected type
                    }
                }
            }
        }
    }
}