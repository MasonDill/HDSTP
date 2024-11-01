use tokio::time::{sleep, Duration};
use std::collections::VecDeque;
use tokio::sync::Notify;
use std::io;
use rand::Rng;
use std::cmp::min;
use std::pin::Pin;
use std::future::Future;
use async_trait::async_trait;

pub mod transfer;
pub use crate::transfer::FileTransfer;
pub use crate::transfer::send_with_retry;

#[derive(Copy, Clone, PartialEq)]
enum PacketType {
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
struct Packet {
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

struct PacketStream {
    buffer: VecDeque<Packet>,
    notify: Notify,
}

impl PacketStream {
    fn new() -> Self {
        PacketStream {
            buffer: VecDeque::new(),
            notify: Notify::new(),
        }
    }

    fn write(&mut self, packet: Packet) {
        self.buffer.push_back(packet);
        self.notify.notify_one();
    }

    async fn read(&mut self) -> Option<Packet> {
        self.notify.notified().await;
        let packet = if self.buffer.is_empty() {
            None
        } else {
            Some(self.buffer.pop_front().unwrap())
        };

        packet
    }

    async fn read_with_timeout(&mut self, timeout: Duration) -> Option<Packet> {
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

    async fn handshake(&mut self) -> io::Result<()> {
        // SYN
        let syn_packet = Packet::new(PacketType::Syn, self.sequence_number, vec![]);
        self.packet_stream.send_packet(syn_packet).await;

        // SYN-ACK
        let response = self.packet_stream.receive_packet(Duration::from_secs(1)).await;
        let packet_type = response.as_ref().unwrap().packet_type;
        if packet_type != PacketType::SynAck{
            return Err(std::io::Error::new(io::ErrorKind::Other, "Did not recieve handshake SNY-ACK response"))
        }

        let response_data = response.unwrap().data;
        self.sequence_number = bytes_to_u32(&response_data, 0)?;
        
        // ACK
        let ack_packet = Packet::new(PacketType::Ack, self.sequence_number, vec![]);
        self.packet_stream.send_packet(ack_packet).await;

        Ok(())
    }

    async fn send_data(&mut self, data: Vec<u8>) -> io::Result<()> {
        let seq_no = self.sequence_number + 1;
        let data_packet = Packet::new(PacketType::Data, seq_no, data);

        // Send data
        self.packet_stream.send_packet(data_packet).await;
        
        // Await ACK
        let response = self.packet_stream.receive_packet(Duration::new(1, 0)).await;
        let packet_type = response.as_ref().unwrap().packet_type;
        if packet_type != PacketType::Ack{
            return Err(std::io::Error::new(io::ErrorKind::Other, "Did not recieve data ACK response"));
        }
        Ok(())
    }

    async fn terminate(&mut self) -> io::Result<()> {
        let seq_no = self.sequence_number + 1;
        // Send FIN
        let fin_packet = Packet::new(PacketType::Fin, seq_no, vec![]);
        self.packet_stream.send_packet(fin_packet).await;

        // Await ACK response
        let response = self.packet_stream.receive_packet(Duration::new(1, 0)).await;
        let packet_type = response.as_ref().unwrap().packet_type;
        if packet_type != PacketType::Ack{
            return Err(std::io::Error::new(io::ErrorKind::Other, "Did not recieve fin ACK response"));
        }
        
        // Await FIN/RST
        let response = self.packet_stream.receive_packet(Duration::new(1, 0)).await;
        let packet_type = response.as_ref().unwrap().packet_type;
        if packet_type != PacketType::Fin{
            return Err(std::io::Error::new(io::ErrorKind::Other, "Did not recieve fin FIN response"));
        }

        // Send ACK
        let ack_packet = Packet::new(PacketType::Ack, seq_no, vec![]);
        self.packet_stream.send_packet(ack_packet).await;
        Ok(())
    }

    fn packetize(&self, data: Vec<u8>, packet_size: usize) -> Vec<Vec<u8>> {
        let mut packets: Vec<Vec<u8>> = Vec::new();
        let mut i = 0;

        while i < data.len() {
            // Determine the size of the next chunk
            let end = min(i + packet_size, data.len());
            // Push the chunk into packets
            packets.push(data[i..end].to_vec());
            i += packet_size;
        }

        return packets
    }

    pub async fn transmit(&mut self, data: Vec<u8>) -> io::Result<()>{
        const PACKET_SIZE: usize = 8; // packet data size (in bytes)
        let packets = self.packetize(data, PACKET_SIZE);

        _ = self.handshake().await;
        for packet in packets {
            _ = self.send_data(packet).await;
        }
        _ = self.terminate().await;

        Ok(())
    }
}

struct Server {
    packet_stream: PacketStream,
    sequence_number: u32
}

impl Server {
    pub fn new(packet_stream: PacketStream) -> Server {
        let mut rng = rand::thread_rng();
        let sequence_number: u32 = rng.gen();
        Server {
            packet_stream,
            sequence_number,
        }
    }

    async fn handshake(&mut self) -> io::Result<()> {
        // await SYN
        let request = self.packet_stream.receive_packet(Duration::new(1, 0)).await;
        let packet_type = request.as_ref().unwrap().packet_type;
        if packet_type == PacketType::Syn{
            return Err(std::io::Error::new(io::ErrorKind::Other, "Expected SYN request"));
        }

        // return SYN-ACK with sequence number
        let syn_ack_packet = Packet::new(PacketType::SynAck, self.sequence_number, vec![]);
        self.packet_stream.send_packet(syn_ack_packet).await;

        // Await ACK
        let response = self.packet_stream.receive_packet(Duration::new(1, 0)).await;
        let packet_type = response.as_ref().unwrap().packet_type;
        if packet_type == PacketType::Syn{
            return Err(std::io::Error::new(io::ErrorKind::Other, "Expected SYN request"));
        }

        Ok(())
    }

    async fn receive_data(&mut self) -> Result<Packet, io::Error> {
        // await DATA packet
        let request = self.packet_stream.receive_packet(Duration::new(1, 0)).await;
        let packet_type = request.as_ref().unwrap().packet_type;

        if packet_type == PacketType::Fin{
            return Ok(request.unwrap());
        }
        else if packet_type != PacketType::Data{
            return Err(std::io::Error::new(io::ErrorKind::Other, "Expected DATA request"));
        }

        // respond with ACK
        let ack_packet = Packet::new(PacketType::Ack, self.sequence_number, vec![]);
        self.packet_stream.send_packet(ack_packet).await;
        
        return Ok(request.unwrap())
    }

    async fn terminate(&mut self) -> io::Result<()> {
        // respond with ACK
        let sequence_no = self.sequence_number + 1;
        let ack_packet = Packet::new(PacketType::Ack, sequence_no, vec![]);
        self.packet_stream.send_packet(ack_packet).await;
        
        Ok(())
    }

    pub async fn recieve(&mut self) -> io::Result<()> {
        let _ = self.handshake();

        let mut data_packets : Vec<Vec<u8>> = vec![vec![]];
        while let packet = self.receive_data().await? {
            if packet.packet_type == PacketType::Data {
                data_packets.push(packet.data);
            } else {
                break; // Exit the loop if the condition is not met
            }
        }

        let _ = self.terminate().await;
        Ok(())
    }
}