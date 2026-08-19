use std::{net::SocketAddr, time::Duration};

use futures_util::StreamExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use url::Url;

use super::{HttpTransport, ReqwestTransport};
use crate::ApprovedUrl;

#[tokio::test]
async fn request_connects_to_the_pinned_address_without_system_dns() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let server = tokio::spawn(serve_once(listener));
    let url = Url::parse(&format!(
        "http://unresolvable.invalid:{}/audio",
        address.port()
    ))
    .expect("URL");
    let target = ApprovedUrl::test_only(
        url,
        "unresolvable.invalid".to_owned(),
        vec![SocketAddr::from(([127, 0, 0, 1], address.port()))],
    );

    let response = ReqwestTransport
        .get(&target, Duration::from_secs(2), Duration::from_secs(1))
        .await
        .expect("pinned request");

    assert_eq!(response.status, 200);
    let bytes = response
        .body
        .fold(Vec::new(), |mut output, chunk| async move {
            output.extend_from_slice(&chunk.expect("body chunk"));
            output
        })
        .await;
    assert_eq!(bytes, b"fixture");
    server.await.expect("server task");
}

async fn serve_once(listener: TcpListener) {
    let (mut stream, _) = listener.accept().await.expect("accept");
    let mut request = [0_u8; 2_048];
    let read = stream.read(&mut request).await.expect("request");
    assert!(String::from_utf8_lossy(&request[..read]).starts_with("GET /audio HTTP/1.1"));
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nfixture")
        .await
        .expect("response");
}
