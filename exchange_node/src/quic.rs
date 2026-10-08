use crate::{Frame, ProtocolError};
use quinn::{Connection, Endpoint, RecvStream, SendStream};
use std::net::SocketAddr;
use std::sync::Arc;

pub async fn accept_loop(
    endpoint: Endpoint,
    on_frame: Arc<dyn Fn(Frame) + Send + Sync>,
) -> Result<(), quinn::ConnectionError> {
    while let Some(incoming) = endpoint.accept().await {
        let connection = incoming.await?;
        let callback = Arc::clone(&on_frame);
        tokio::spawn(async move {
            if let Err(error) = serve_connection(connection, callback).await {
                eprintln!("peer connection closed: {error:?}");
            }
        });
    }
    Ok(())
}

async fn serve_connection(
    connection: Connection,
    on_frame: Arc<dyn Fn(Frame) + Send + Sync>,
) -> Result<(), ProtocolError> {
    loop {
        let (_, mut recv) = connection
            .accept_bi()
            .await
            .map_err(|_| ProtocolError::InvalidLength)?;
        let frame = read_frame(&mut recv).await?;
        on_frame(frame);
    }
}

async fn read_frame(recv: &mut RecvStream) -> Result<Frame, ProtocolError> {
    let mut length_bytes = [0u8; 2];
    recv.read_exact(&mut length_bytes)
        .await
        .map_err(|_| ProtocolError::InvalidLength)?;
    let length = u16::from_be_bytes(length_bytes) as usize;
    if length > crate::MAX_FRAME_BYTES || length != crate::FRAME_BYTES {
        return Err(ProtocolError::InvalidLength);
    }

    let mut buf = [0u8; crate::FRAME_BYTES];
    recv.read_exact(&mut buf)
        .await
        .map_err(|_| ProtocolError::InvalidLength)?;
    Frame::decode(&buf)
}

pub async fn send_frame(
    connection: &Connection,
    frame: Frame,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut send, _) = connection.open_bi().await?;
    write_frame(&mut send, frame).await?;
    send.finish()?;
    Ok(())
}

async fn write_frame(
    send: &mut SendStream,
    frame: Frame,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    send.write_all(&(crate::FRAME_BYTES as u16).to_be_bytes())
        .await?;
    send.write_all(&frame.encode()).await?;
    Ok(())
}

pub async fn connect(
    endpoint: &Endpoint,
    addr: SocketAddr,
    server_name: &str,
) -> Result<Connection, Box<dyn std::error::Error + Send + Sync>> {
    Ok(endpoint.connect(addr, server_name)?.await?)
}
