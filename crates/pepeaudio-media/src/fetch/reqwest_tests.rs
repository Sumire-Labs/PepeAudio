use std::{net::SocketAddr, time::Duration};

use futures_util::StreamExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use url::Url;

use super::{HttpTransport, ReqwestTransport};
use crate::{ApprovedUrl, SafeHttpHeaders};

#[tokio::test]
async fn request_connects_to_the_pinned_address_without_system_dns() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let server = tokio::spawn(serve_once(listener, false));
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

#[tokio::test]
async fn generated_open_range_has_one_fixed_value() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let server = tokio::spawn(serve_once(listener, true));
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
        .get_with_headers_and_open_range(
            &target,
            &SafeHttpHeaders::default(),
            true,
            Duration::from_secs(2),
            Duration::from_secs(1),
        )
        .await
        .expect("pinned range request");

    assert_eq!(response.status, 200);
    server.await.expect("server task");
}

async fn serve_once(listener: TcpListener, expect_open_range: bool) {
    let (mut stream, _) = listener.accept().await.expect("accept");
    let mut request = [0_u8; 2_048];
    let read = stream.read(&mut request).await.expect("request");
    let request = String::from_utf8_lossy(&request[..read]).to_ascii_lowercase();
    assert!(request.starts_with("get /audio http/1.1"));
    assert_eq!(
        request.contains("\r\nrange: bytes=0-\r\n"),
        expect_open_range
    );
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nfixture")
        .await
        .expect("response");
}
