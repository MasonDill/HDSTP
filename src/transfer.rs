use tokio;
use std::io;
use std::time::Duration;

struct FileTransfer {
    client: Option<Client>,
    server: Option<Server>,
}

impl FileTransfer {
    fn new() -> Self {
        Self {
            client: None,
            server: None,
        }
    }

    async fn start_server(&mut self) -> io::Result<()> {
        let stream = PacketStream::new();
        let mut server = Server::new(stream);
        
        println!("Server started, waiting for connections...");
        
        // Start listening for incoming data
        tokio::spawn(async move {
            loop {
                match server.recieve().await {
                    Ok(()) => {
                        println!("Successfully received data");
                    },
                    Err(e) => {
                        eprintln!("Error receiving data: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    async fn send_file(&mut self, data: Vec<u8>) -> io::Result<()> {
        let stream = PacketStream::new();
        let mut client = Client::new(stream);

        println!("Starting file transfer...");
        
        // Transmit the file data
        match client.transmit(data).await {
            Ok(()) => {
                println!("File transfer completed successfully");
                Ok(())
            },
            Err(e) => {
                eprintln!("Error during file transfer: {}", e);
                Err(e)
            }
        }
    }
}

async fn send_with_retry(data: Vec<u8>, max_retries: u32) -> io::Result<()> {
    let mut retry_count = 0;
    let mut transfer = FileTransfer::new();

    while retry_count < max_retries {
        match transfer.send_file(data.clone()).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                retry_count += 1;
                if retry_count < max_retries {
                    println!("Transfer failed, attempting retry {} of {}", retry_count, max_retries);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
    
    Err(io::Error::new(io::ErrorKind::Other, "Max retries exceeded"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_transfer() -> io::Result<()> {
        let mut file_transfer = FileTransfer::new();
        
        // Start server
        file_transfer.start_server().await?;
        
        // Test data
        let test_data = b"Test data".to_vec();
        
        // Send file
        file_transfer.send_file(test_data).await?;
        
        Ok(())
    }

    #[tokio::test]
    async fn test_large_transfer() -> io::Result<()> {
        let mut file_transfer = FileTransfer::new();
        
        // Start server
        file_transfer.start_server().await?;
        
        // Create large test data (100KB)
        let test_data = vec![0u8; 102400];
        
        // Send file
        file_transfer.send_file(test_data).await?;
        
        Ok(())
    }

    #[tokio::test]
    async fn test_retry_mechanism() -> io::Result<()> {
        let test_data = b"Test retry data".to_vec();
        
        send_with_retry(test_data, 3).await?;
        
        Ok(())
    }
}