use celestia_types::Blob;
use http_body_util::{BodyExt, Full};
use hyper::{
    body::{Buf, Bytes, Incoming as IncomingBody}, server::conn::http1, service::service_fn, Request, Response
};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use anyhow::{anyhow, Result};

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type BoxBody = http_body_util::combinators::BoxBody<Bytes, GenericError>;


#[derive(serde::Deserialize)]
struct ParamsGet {
    blobs: Vec<Blob>,
    _tx_config: serde_json::Value
}

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

        let service = service_fn(move |mut plaintext_req: Request<IncomingBody>| {
            let uri_string = format!(
                "http://{}{}",
                out_addr.clone(),
                plaintext_req
                    .uri()
                    .path_and_query()
                    .map(|x| x.as_str())
                    .unwrap_or("/")
            );
            let uri = uri_string.parse().unwrap();
            *plaintext_req.uri_mut() = uri;

            let host = plaintext_req.uri().host().expect("uri has no host");
            let port = plaintext_req.uri().port_u16().unwrap_or(80);
            let addr = format!("{}:{}", host, port);

            async move {
                let mut request_method: String = Default::default();
                let wrapped_req = inbound_handler(plaintext_req, &mut request_method).await?;

                let client_stream = TcpStream::connect(addr).await?;
                let io = TokioIo::new(client_stream);
                let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
                tokio::task::spawn(async move {
                    if let Err(err) = conn.await {
                        println!("Connection failed: {:?}", err);
                    }
                });

                let response = sender.send_request(wrapped_req).await?;
                let wrapped_resp = outbound_handler(response, request_method
                ).await?;
                anyhow::Ok(wrapped_resp)
            }
        });

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                println!("Failed to serve the connection: {:?}", err);
            }
        });
    }
}

/// Introspect a JSON RPC request and (conditionally) mutate it to enable encryption and decryption.
///
/// ### NOTE:
///
/// Presently we need to wait for the full request body to be received,
/// and fully serialize and deserialize it even if not needed.
/// This isn't optimal... but functional.
async fn inbound_handler(req: Request<IncomingBody>, request_method: &mut String) -> Result<Request<BoxBody>> {
    let (mut parts, body_stream) = req.into_parts();
    let body_complete = body_stream.collect().await?.aggregate();

    let body_json: serde_json::Value = serde_json::from_reader(body_complete.reader())?;

    if let Some(method) = body_json.get("method").and_then(|m| m.as_str()) {
        // Set a hook for outbound handler
        *request_method = method.to_string(); 

        match method {
            // <https://node-rpc-docs.celestia.org/#blob.Submit>
            "blob.Submit" => {
                println!("blob.Submit intercept");
                let params_raw= body_json.get("params").ok_or(anyhow!("`blob.Get` Invalid JSON params"))?;
                dbg!(&params_raw);
                let params: ParamsGet = serde_json::from_value(params_raw.clone())?;
                for blob in params.blobs {
                    dbg!(blob);
                }
            }
            &_ => {},

        }
    }

    let json = serde_json::to_string(&body_json)?;

    // MISSION CRITICAL
    // Without it, the request will likely hang or fail,
    // I am assuming it's recalculated if missing, as it works
    parts.headers.remove("content-length");

    let new_body = Full::new(Bytes::from(json))
        .map_err(|err: std::convert::Infallible| match err {})
        .boxed();

    Ok(Request::from_parts(parts, new_body))
}

/// Introspect a JSON RPC request and (conditionally) mutate it to enable encryption and decryption.
///
/// ### NOTE:
///
/// Presently we need to wait for the full request body to be received,
/// and fully serialize and deserialize it even if not needed.
/// This isn't optimal... but functional.
async fn outbound_handler(resp: Response<IncomingBody>, request_method: String) -> Result<Response<BoxBody>> {
    let (mut parts, body_stream) = resp.into_parts();
    let body_complete = body_stream.collect().await?.aggregate();

    let body_json: serde_json::Value = serde_json::from_reader(body_complete.reader())?;

        match request_method.as_str() {
            // <https://node-rpc-docs.celestia.org/#blob.Get>
            "blob.Get" => {
                println!("blob.Get intercept");
                let result_raw= body_json.get("result").ok_or(anyhow!("`blob.Get` missing \"result\" from upstream node"))?;
                dbg!(&result_raw);
                let blob: Blob = serde_json::from_value(result_raw.clone())?;
                dbg!(blob);
            }
            // <https://node-rpc-docs.celestia.org/#blob.Get>
            "blob.GetAll" => {
                println!("blob.GetAll intercept");
                let result_raw= body_json.get("result").ok_or(anyhow!("`blob.Get` missing \"result\" from upstream node"))?;
                dbg!(&result_raw);
                let blobs: Vec<Blob> = serde_json::from_value(result_raw.clone())?;
                for blob in blobs {
                    dbg!(blob);
                }
            }
            &_ => {},

        }

    let json = serde_json::to_string(&body_json)?;

    // MISSION CRITICAL
    // Without it, the request will likely hang or fail,
    // I am assuming it's recalculated if missing, as it works
    parts.headers.remove("content-length");

    let new_body = Full::new(Bytes::from(json))
        .map_err(|err: std::convert::Infallible| match err {})
        .boxed();

    Ok(Response::from_parts(parts, new_body))
}
