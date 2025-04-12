use http_body_util::{BodyExt, Full};
use hyper::{
    Request,
    body::{Buf, Bytes, Incoming as IncomingBody},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, GenericError>;
type BoxBody = http_body_util::combinators::BoxBody<Bytes, GenericError>;

#[tokio::main]
async fn main() -> Result<()> {
    let in_addr: SocketAddr = ([127, 0, 0, 1], 3001).into();
    let out_addr: SocketAddr = ([127, 0, 0, 1], 26658).into();

    let listener = TcpListener::bind(in_addr).await?;

    println!("Listening on http://{}", in_addr);
    println!("Proxying on http://{}", out_addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let service = service_fn(move |mut plarintext_req: Request<IncomingBody>| {
            let uri_string = format!(
                "http://{}{}",
                out_addr.clone(),
                plarintext_req
                    .uri()
                    .path_and_query()
                    .map(|x| x.as_str())
                    .unwrap_or("/")
            );
            let uri = uri_string.parse().unwrap();
            *plarintext_req.uri_mut() = uri;

            let host = plarintext_req.uri().host().expect("uri has no host");
            let port = plarintext_req.uri().port_u16().unwrap_or(80);
            let addr = format!("{}:{}", host, port);

            async move {
                let encrypted_req = intercept_request_and_encrypt(plarintext_req).await.unwrap();
                let client_stream = TcpStream::connect(addr).await.unwrap();
                let io = TokioIo::new(client_stream);

                let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
                tokio::task::spawn(async move {
                    if let Err(err) = conn.await {
                        println!("Connection failed: {:?}", err);
                    }
                });

                sender.send_request(encrypted_req).await
            }
        });

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                println!("Failed to serve the connection: {:?}", err);
            }
        });
    }
}

/// Introspect a JSON RPC request and mutate it to enable encryption and decryption.
///
/// ### NOTE:
///
/// Presently we need to wait for the full request body to be recived, and fully serialize it.
/// This isn't optimal... but functional
async fn intercept_request_and_encrypt(req: Request<IncomingBody>) -> Result<Request<BoxBody>> {
    let (mut parts, body) = req.into_parts();
    let full_body = body.collect().await?.aggregate();

    let data: serde_json::Value = serde_json::from_reader(full_body.reader())?;

    if let Some(method) = data.get("method").and_then(|m| m.as_str()) {
        if method == "blob.Get" {
            println!("INTERCEPT");
        }
    }

    let json = serde_json::to_string(&data)?;

    // MISSION CRITICAL
    // Without it, the request will likely hang or fail,
    // I am assuming it's recalculated if missing, as it works
    parts.headers.remove("content-length");

    let new_body = Full::new(Bytes::from(json))
        .map_err(|err: std::convert::Infallible| match err {})
        .boxed();

    Ok(Request::from_parts(parts, new_body))
}
