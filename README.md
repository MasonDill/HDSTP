# Custom TCP Protocol
This project implements a custom TCP-like protocol designed for data transmission. It incorporates mechanisms for connection establishment, data transmission, retransmission, and connection termination. The protocol aims to provide a reliable, albeit slow means of communication over a networking layer.

View SPECIFICATION.md for more information on the protocol details.

## Features
- **3-Way Handshake**: Establishes a reliable connection between client and server.
- **Data Transmission**: Efficiently sends data packets with acknowledgment and retransmission capabilities.
- **Packet loss detection**: Sequence number verify that every packet was recieved in order
- **Data corruption detection**: Checksums verify the recieved data was not corrupted in transmission
- **4-Way Handshake**: Gracefully terminates connections while ensuring all data is transmitted.
- **Custom Packet Structure**: A defined packet structure allows for extensibility and control.
