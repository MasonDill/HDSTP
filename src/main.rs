use HDSTP::{FileTransfer, send_with_retry};
use tokio;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    // Start the server in a separate task
    let mut file_transfer = FileTransfer::new();
    file_transfer.start_server().await?;

    // Example data to transfer
    let test_data = b"Hello, this is a test file transfer!".to_vec();
    
    // Send the file with retries
    match send_with_retry(test_data.clone(), 3).await {
        Ok(()) => println!("File transfer completed successfully"),
        Err(e) => eprintln!("File transfer failed: {}", e),
    }

    Ok(())
}
