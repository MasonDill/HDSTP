# TCP Protocol
1. Initialize connection
2. Data transmission
3. Terminate connection
4. Release resources

# Initialization protocol (3-way handshake)
1. SYN
    - Client sends a SYN packet to the server to initiate the connection.
    - Provides a random client initial sequence number (cISN) for client packet ordering.
    - Retransmits if timeout and no SYN-ACK response
      - Gives up after 3 failures
2. SYN-ACK
    - Server responds to the client with a SYN-ACK packet using the client's cISN + 1 as payload.
    - Provides a random server initial sequence number (sISN) for server packet ordering.
    - Retransmits if timeout and no ACK
      - Gives up after 3 failures
3. ACK
    - Client responds to the server with an ACK packet using the server's sISN + 1 as payload.

# Data transmission protocol
Data and acknowledgement is performed in lock-step (synchronised). The Client must recieve an acknoledgement before sending the next packet. This protocol is not designed for out-of-order recieving for simplicity; furthermore, communication is half-duplex (client->server). Sequence numbers are unused for transmission at this time.

1. Client sends DATA/RETRANSMISSION
   - Client sends next DATA packet to the server if last packet from server was ACK
   - Client retransmits last packet if response was a NAK or timeout
   - Ignores invalid responses (anything that is not ACK/NAK)
     - May responds to SYN-ACK message, but **only** on the first dataframe
   - Retransmits after timeout or NAK
     - Give up after 3 failures
2. Server responds ACK/NAK
   - Server checks the parity bit
   - Server sends an ACK packet to confirm the packet was recieved correctly or a NAK if parity is incorrect
   - Retransmits ACK/NAK if timeout occurs
     - Give up after 3 failures
   - Ignores invalid packets (anything that is not DATA)

# Termination protocol (4-way handshake)
1. FIN
   - Client sends a FIN packet to the server to indicate transmission is complete.
   - Client provides CRC checksum in payload
   - Retransmits after timeout if it does not recieve an ACK response
     - Give up after 3 tries
2. ACK
   - Server sends an ACK packet to the Client.
3. FIN/RST
   - Server sends a FIN packet to indicate they are ready to close the connection or sends RST packet if the checksum does not match recieved data
   - RST restarts data transmission
   - Retransmits after timeout if it does not recieve an ACK or SYN response
     - Give up after 3 tries
4. ACK
   - Client sends an ACK packet to the server to acknowledge the closure or reset.

# Reset protocol
1. RST
   - Server sends RST message to notify client
   - Retransmit after timeout or invalid response
2. If client recieves RST from server after it has sent a FIN packet, it reinitiates 3 way handshake protocol
   - Ignores RST packets if it has not yet sent a FIN

# Packet Structure
| Field         | Size     | Description                                         |
|---------------|----------|-----------------------------------------------------|
| Length        | 4 bytes  | Length of the message (N)                           |
| Type          | 1 byte   | Type of message (data, acknowledgment, etc.)        |
| Sequence No.  | 4 bytes  | Message sequence number                             |
| Data/Checksum | N bytes  | Actual data being sent (or checksum)                |
| Parity        | 1 byte   | Bit (`01111` or `0b0000`) to achieve **even** parity|

## Packet frame
|   4 byte (length)  |   1 byte (type)   |   4 byte (sequence No.)   |   `length` bytes  |

# Type Enumeration
Types are a bitfield. Bit 3 indicate the subtype (control/data), bits 0 to 2 indicate variety.

|   1 bit (subtype)  |  3 bits (variety)  |

## Subtype (bit 3): 
  - `1` = Control packet
  - `0` = Data packet

## Control Packets (bit 3 = 1, control packets)
- `8` (`0b1000`) = SYN (synchronize connection)
- `9` (`0b1001`) = SYN-ACK (synchronize and acknowledge)
- `10` (`0b1010`) = ACK (acknowledge packet)
- `11` (`0b1011`) = NAK (negative acknowledge packet)
- `12` (`0b1100`) = FIN (finish/terminate connection)
- `13` (`0b1101`) = RST (reset connection)
- `14` & `15` (`0b1110` & `0b1111`): Undefined

## Data Packets (bit 3 = 0, data packets)
- `0` (`0b0000`) = Normal data
- `1` (`0b0001`) = Retransmission
- `4` to `7` (`0b0100` to `0b0111`): Undefined
