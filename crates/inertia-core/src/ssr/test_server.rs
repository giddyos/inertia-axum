use bytes::Bytes;
use http::StatusCode;
use std::{future::Future, net::SocketAddr, sync::Arc};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

pub(crate) struct Request {
    pub(crate) path: String,
    pub(crate) body: Bytes,
    pub(crate) peer: SocketAddr,
}

pub(crate) struct Response {
    status: StatusCode,
    body: Bytes,
}

impl Response {
    pub(crate) fn ok(body: impl Into<Bytes>) -> Self {
        Self {
            status: StatusCode::OK,
            body: body.into(),
        }
    }

    pub(crate) fn status(status: StatusCode, body: impl Into<Bytes>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }
}

pub(crate) async fn server<H, F>(handler: H) -> String
where
    H: Fn(Request) -> F + Send + Sync + 'static,
    F: Future<Output = Response> + Send + 'static,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handler = Arc::new(handler);
    tokio::spawn(async move {
        loop {
            let Ok((stream, peer)) = listener.accept().await else {
                break;
            };
            let handler = handler.clone();
            tokio::spawn(async move {
                serve_connection(stream, peer, handler).await;
            });
        }
    });
    format!("http://{address}")
}

async fn serve_connection<H, F>(
    mut stream: tokio::net::TcpStream,
    peer: SocketAddr,
    handler: Arc<H>,
) where
    H: Fn(Request) -> F + Send + Sync + 'static,
    F: Future<Output = Response> + Send + 'static,
{
    let mut buffer = Vec::with_capacity(1024);
    loop {
        let header_end = loop {
            if let Some(position) = find_header_end(&buffer) {
                break position;
            }
            let mut chunk = [0_u8; 1024];
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
            }
        };
        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let mut request_line = headers
            .lines()
            .next()
            .unwrap_or_default()
            .split_whitespace();
        let _method = request_line.next().unwrap_or_default();
        let path = request_line.next().unwrap_or_default().to_owned();
        let content_length = headers
            .lines()
            .skip(1)
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        let request_end = header_end + 4 + content_length;
        while buffer.len() < request_end {
            let mut chunk = [0_u8; 1024];
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
            }
        }
        let body = Bytes::copy_from_slice(&buffer[header_end + 4..request_end]);
        buffer.drain(..request_end);

        let response = handler(Request { path, body, peer }).await;
        let reason = response.status.canonical_reason().unwrap_or("Unknown");
        let head = format!(
            "HTTP/1.1 {} {reason}\r\ncontent-length: {}\r\ncontent-type: application/json\r\nconnection: keep-alive\r\n\r\n",
            response.status.as_u16(),
            response.body.len()
        );
        if stream.write_all(head.as_bytes()).await.is_err()
            || stream.write_all(&response.body).await.is_err()
        {
            return;
        }
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}
