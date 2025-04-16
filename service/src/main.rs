use anyhow::Result;
use celestia_types::Blob;
use http_body_util::{BodyExt, Full};
use hyper::{
    Request, Response,
    body::{Buf, Bytes, Incoming as IncomingBody},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use log::{debug, info};
use sha2::{Digest, Sha256};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{OnceCell, mpsc},
};

mod internal;
use internal::error::*;
use internal::job::*;
use internal::runner::*;
use internal::util::*;

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type BoxBody = http_body_util::combinators::BoxBody<Bytes, GenericError>;

#[derive(serde::Deserialize)]
struct ParamsGet {
    blobs: Vec<Blob>,
    _tx_config: serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // let da_node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN")
    //     .expect("CELESTIA_NODE_AUTH_TOKEN env var required");
    let da_node_socket: SocketAddr = std::env::var("CELESTIA_NODE_SOCKET")
        .expect("CELESTIA_NODE_SOCKET env var required")
        .parse()
        .expect("CELESTIA_NODE_SOCKET cannot parse");
    let service_socket: SocketAddr = std::env::var("PDA_SOCKET")
        .expect("PDA_SOCKET env var required")
        .parse()
        .expect("PDA_SOCKET cannot parse");

    let db_path = std::env::var("PDA_DB_PATH").expect("PDA_DB_PATH env var required");
    let db = sled::open(db_path.clone())?;
    let config_db = db.open_tree("config")?;
    let queue_db = db.open_tree("queue")?;
    let finished_db = db.open_tree("finished")?;

    let listener = TcpListener::bind(service_socket).await?;

    println!("Listening on http://{}", service_socket);
    println!("Proxying on http://{}", da_node_socket);

    info!("Building clients and service setup");
    let (job_sender, job_receiver) = mpsc::unbounded_channel::<Option<Job>>();
    let pda_runner = Arc::new(PdaRunner::new(
        PdaRunnerConfig {},
        OnceCell::new(),
        config_db.clone(),
        queue_db.clone(),
        finished_db.clone(),
        job_sender.clone(),
    ));

    tokio::spawn({
        let runner = pda_runner.clone();
        async move {
            let program_id = get_program_id().await;
            let zk_client = runner.clone().get_zk_client().await;
            debug!("ZK client prepared, acquiring setup");
            let _ = runner.get_proof_setup(&program_id, zk_client).await;
            info!("ZK client ready!");
        }
        // TODO: crash whole program if this fails
    });

    debug!("Runner hook shutdown signals");
    tokio::spawn({
        let runner = pda_runner.clone();
        async move {
            wait_shutdown_signals().await;
            runner.shutdown();
        }
    });

    debug!("Starting runner worker");
    tokio::spawn({
        let runner = pda_runner.clone();
        async move { runner.job_worker(job_receiver).await }
    });

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let runner = pda_runner.clone();
        let service = service_fn(move |mut plaintext_req: Request<IncomingBody>| {
            let uri_string = format!(
                "http://{}{}",
                da_node_socket.clone(),
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

            let runner = runner.clone();
            async move {
                let mut request_method: String = Default::default();
                let wrapped_req =
                    inbound_handler(plaintext_req, &mut request_method, runner).await?;

                let client_stream = TcpStream::connect(addr).await?;
                let io = TokioIo::new(client_stream);
                let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
                tokio::task::spawn(async move {
                    if let Err(err) = conn.await {
                        println!("Connection failed: {:?}", err);
                    }
                });

                let response = sender.send_request(wrapped_req).await?;
                let wrapped_resp = outbound_handler(response, request_method).await?;
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

/// Introspect a JSON RPC request and (conditionally) mutate it to encrypt before forwarding to the downstream node.
/// Set `request_method` as a hook for [outbound_handler] to use.
///
/// ### NOTE:
///
/// Presently we need to wait for the full request body to be received,
/// and fully serialize and deserialize it even if not needed.
/// This isn't optimal... but functional.
async fn inbound_handler(
    req: Request<IncomingBody>,
    request_method: &mut String,
    pda_runner: Arc<PdaRunner>,
) -> Result<Request<BoxBody>> {
    let (mut parts, body_stream) = req.into_parts();
    let body_complete = body_stream.collect().await?.aggregate();

    let body_json: serde_json::Value = serde_json::from_reader(body_complete.reader())?;

    if let Some(method) = body_json.get("method").and_then(|m| m.as_str()) {
        *request_method = method.to_string();

        match method {
            // <https://node-rpc-docs.celestia.org/#blob.Submit>
            "blob.Submit" => {
                println!("blob.Submit intercept");
                if let Some(params_raw) = body_json.get("params") {
                    let params: ParamsGet = serde_json::from_value(params_raw.clone())?;
                    // TODO: consider only allowing one blob and one job on the queue
                    for blob in params.blobs {
                        let pda_runner = pda_runner.clone();
                        tokio::spawn(async move {
                            let data = blob.data.to_owned();
                            let input = Input {
                                data: data.to_owned(),
                            };
                            let hash = Sha256::digest(&data);
                            let anchor = Anchor {
                                data: hash.as_slice().into(),
                            };

                            let job = Job { anchor, input };
                            let _ = pda_runner.get_verifiable_encryption(job).await;
                        });
                    }
                } else {
                    println!("Forwarding `blob.Submit` error");
                }
            }
            &_ => {}
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

/// Introspect a JSON RPC response and (conditionally) mutate it by decrypting data before returning to the original client.
/// Requires a `request_method` typically set in [inbound_handler].
///
/// ### NOTE:
///
/// Presently we need to wait for the full request body to be received,
/// and fully serialize and deserialize it even if not needed.
/// This isn't optimal... but functional.
async fn outbound_handler(
    resp: Response<IncomingBody>,
    request_method: String,
) -> Result<Response<BoxBody>> {
    let (mut parts, body_stream) = resp.into_parts();
    let body_complete = body_stream.collect().await?.aggregate();

    let body_json: serde_json::Value = serde_json::from_reader(body_complete.reader())?;

    match request_method.as_str() {
        // <https://node-rpc-docs.celestia.org/#blob.Get>
        "blob.Get" => {
            println!("blob.Get intercept");
            if let Some(result_raw) = body_json.get("result") {
                dbg!(&result_raw);
                let blob: Blob = serde_json::from_value(result_raw.clone())?;
                // TODO: SP1 Verify encryption & anchors proof, Decrypt data
                // TODO: Return {custom?} error and/or decrypted data
                dbg!(blob);
            } else {
                println!("Forwarding `blob.Get` error");
            }
        }
        // <https://node-rpc-docs.celestia.org/#blob.Get>
        "blob.GetAll" => {
            println!("blob.GetAll intercept");
            if let Some(result_raw) = body_json.get("result") {
                dbg!(&result_raw);
                let blobs: Vec<Blob> = serde_json::from_value(result_raw.clone())?;
                for blob in blobs {
                    dbg!(blob);
                }
            } else {
                println!("Forwarding `blob.GetAll` error");
            }
        }
        &_ => {}
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
