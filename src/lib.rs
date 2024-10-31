use tokio::time::{sleep, Duration};
use std::collections::VecDeque;
use tokio::sync::Notify;
use std::io;
use std::pin::Pin;
use std::future::Future;

use async_trait::async_trait;


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

fn bytes_to_u32(byte_vector: &Vec<u8>, start_index: usize) -> Result<u32, io::Error> {
    // converts to BIG endian u32
    if start_index + 4 <= byte_vector.len() {
        let result: u32 = (byte_vector[start_index] as u32) << 24
                        | (byte_vector[start_index + 1] as u32) << 16
                        | (byte_vector[start_index + 2] as u32) << 8
                        | (byte_vector[start_index + 3] as u32);
        Ok(result)
    } else {
        Err(std::io::Error::new(io::ErrorKind::Other, "Cannot read U32 from vector"))
    }
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

    pub async fn read_with_timeout(&mut self, timeout: Duration) -> Option<Packet> {
        // Simulate timeout behavior when reading
        tokio::select! {
            _ = sleep(timeout) => {
                // Timeout occurred
                None
            },
            packet = self.read() => {
                packet
            }
        }
    }
}

#[async_trait::async_trait]
trait TransportProtocol {
    async fn send_packet(&mut self, packet: Packet);
    async fn receive_packet(&mut self, timeout: Duration) -> Option<Packet>;
}

#[async_trait::async_trait]
impl TransportProtocol for PacketStream {
    // Send a packet using the stream (write it to the buffer)
    async fn send_packet(&mut self, packet: Packet) {
        self.write(packet);
    }

    // Read a packet from the stream with a timeout
    async fn receive_packet(&mut self, timeout: Duration) -> Option<Packet> {
        self.read_with_timeout(timeout).await
    }
}

struct Client {
    packet_stream: PacketStream,
    sequence_number: u32
}

impl Client {
    pub fn new(packet_stream: PacketStream) -> Client {
        let sequence_number = 0;
        Client {
            packet_stream,
            sequence_number
        }
    }

    pub async fn handshake(&mut self) -> Result<(), io::Error> {
        // SYN
        let syn_packet = Packet::new(PacketType::Syn, self.sequence_number, vec![]);
        self.packet_stream.send_packet(syn_packet).await;

        // SYN-ACK
        let response = self.packet_stream.receive_packet(Duration::from_secs(1)).await;
        // let packet_type = &response.unwrap().packet_type;

        // if packet_type == &PacketType::SynAck{
        //     return Err(std::io::Error::new(io::ErrorKind::Other, "Cannot read U32 from vector"))
        // }

        let response_data = response.unwrap().data;
        self.sequence_number = bytes_to_u32(&response_data, 0)?;
        
        // ACK
        let ack_packet = Packet::new(PacketType::Ack, self.sequence_number, vec![]);
        self.packet_stream.send_packet(ack_packet).await;

        Ok(())
    }

    pub async fn send_data(&mut self, data: Vec<u8>) -> io::Result<()> {
        let seq_no = self.conn.client_isn + 1;
        let data_packet = Packet::new(DATA, seq_no, data);

        self.packet_stream.send_packet(data_packet).await;
        println!("Client: Sent DATA with sequence number {}", seq_no);

        self.packet_stream.handle_timeout(
            || {
                tokio::spawn(async move {
                    let timeout = Duration::from_secs(3);
                    if let Some(response_packet) = self.packet_stream.receive_packet(timeout).await {
                        match response_packet.packet_type {
                            ACK => println!("Client: Received ACK for sequence number {}", seq_no),
                            NAK => println!("Client: Received NAK, retransmitting DATA"),
                            _ => println!("Client: Ignoring invalid response"),
                        }
                    } else {
                        return Err(io::Error::new(io::ErrorKind::TimedOut, "Data transmission timed out"));
                    }
                    Ok(())
                });
                Ok(())
            },
            Duration::from_secs(3)
        ).await
    }

    pub async fn terminate(&mut self) -> io::Result<()> {
        let fin_packet = Packet::new(FIN, self.conn.client_isn + 1, vec![]);
        self.packet_stream.send_packet(fin_packet).await;
        println!("Client: Sent FIN");

        self.packet_stream.handle_timeout(
            || {
                tokio::spawn(async move {
                    let timeout = Duration::from_secs(3);
                    if let Some(ack_packet) = self.packet_stream.receive_packet(timeout).await {
                        if ack_packet.packet_type == ACK {
                            println!("Client: Received ACK, waiting for server FIN");
                        } else {
                            return Err(io::Error::new(io::ErrorKind::InvalidData, "Expected ACK"));
                        }
                    }
                    Ok(())
                });
                Ok(())
            },
            Duration::from_secs(3)
        ).await
    }
}

struct Server {
    packet_stream: PacketStream,
    conn: Connection,
}

impl Server {
    pub fn new(packet_stream: PacketStream) -> Server {
        Server {
            packet_stream,
            conn: Connection::new(),
        }
    }

    pub async fn handshake(&mut self) -> io::Result<()> {
        self.packet_stream.handle_timeout(
            || {
                tokio::spawn(async move {
                    let timeout = Duration::from_secs(3);
                    if let Some(syn_packet) = self.packet_stream.receive_packet(timeout).await {
                        if syn_packet.packet_type != SYN {
                            return Err(io::Error::new(io::ErrorKind::InvalidData, "Expected SYN"));
                        }

                        self.conn.client_isn = syn_packet.seq_no;
                        println!("Server: Received SYN with Client ISN {}", self.conn.client_isn);

                        let syn_ack_packet = Packet::new(SYN_ACK, self.conn.client_isn + 1, vec![]);
                        self.packet_stream.send_packet(syn_ack_packet).await;

                        println!("Server: Sent SYN-ACK");
                    } else {
                        return Err(io::Error::new(io::ErrorKind::TimedOut, "Handshake timed out"));
                    }
                    Ok(())
                });
                Ok(())
            },
            Duration::from_secs(3)
        ).await
    }

    pub async fn receive_data(&mut self) -> io::Result<()> {
        self.packet_stream.handle_timeout(
            || {
                tokio::spawn(async move {
                    let timeout = Duration::from_secs(3);
                    if let Some(data_packet) = self.packet_stream.receive_packet(timeout).await {
                        if data_packet.packet_type == DATA {
                            println!("Server: Received DATA with sequence number {}", data_packet.seq_no);

                            if data_packet.verify_parity() {
                                let ack_packet = Packet::new(ACK, data_packet.seq_no, vec![]);
                                self.packet_stream.send_packet(ack_packet).await;
                                println!("Server: Sent ACK");
                            } else {
                                let nak_packet = Packet::new(NAK, data_packet.seq_no, vec![]);
                                self.packet_stream.send_packet(nak_packet).await;
                                println!("Server: Sent NAK due to parity error");
                            }
                        }
                    }
                    Ok(())
                });
                Ok(())
            },
            Duration::from_secs(3)
        ).await
    }

}