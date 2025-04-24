use anyhow::Result;
use celestia_types::Blob;
use hex::FromHex;
use http_body_util::{BodyExt, Full};
use hyper::{
    Request, Response,
    body::{Buf, Bytes, Incoming as IncomingBody},
    server::conn::http1,
    service::service_fn,
};
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use log::{debug, error, info, warn};
use rustls::ServerConfig;
use sha2::{Digest, Sha256};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    net::TcpListener,
    sync::{OnceCell, mpsc},
};
use tokio_rustls::TlsAcceptor;

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
    #[serde(default)]
    _tx_config: Option<serde_json::Value>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // We check here, and read in internal/runner.rs
    let _ = <[u8; 32]>::from_hex(
        std::env::var("ENCRYPTION_KEY").expect("Missing ENCRYPTION_KEY env var"),
    )
    .expect("ENCRYPTION_KEY must be 32 bytes, hex encoded (ex: `1234...abcd`)");

    // let da_node_token = std::env::var("CELESTIA_NODE_WRITE_TOKEN")
    //     .expect("CELESTIA_NODE_WRITE_TOKEN env var required");
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

    // TLS setup
    // TODO: use real certs and keys!!
    let _ = rustls::crypto::ring::default_provider().install_default();
    let tls_certs_path = std::env::var("TLS_CERTS_PATH").expect("TLS_CERTS_PATH env var required");
    let tls_certs = load_certs(&tls_certs_path)?;
    let tls_key_path = std::env::var("TLS_KEY_PATH").expect("TLS_KEY_PATH env var required");
    let tls_key = load_private_key(&tls_key_path)?;
    let listener = TcpListener::bind(service_socket).await?;

    let mut server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(tls_certs, tls_key)?;

    // NOTE: we only support http1 in this service presently
    server_config.alpn_protocols = vec![b"http/1.1".to_vec(), b"http/1.0".to_vec()];
    let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));

    info!("Listening on https://{}", service_socket);
    let https_builder = HttpsConnectorBuilder::new().with_native_roots()?;

    let https_or_http_connector = if std::env::var("UNSAFE_HTTP_UPSTREAM").is_ok() {
        warn!("UNSAFE_HTTP_UPSTREAM — allowing HTTP for upstream Celestia connection!");
        https_builder.https_or_http().enable_http1().build()
    } else {
        info!("Proxying to Celestia securly on https://{}", da_node_socket);
        https_builder.https_only().enable_http1().build()
    };
    let celesita_client: Client<_, BoxBody> =
        Client::builder(TokioExecutor::new()).build(https_or_http_connector);

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

    debug!("Restarting unfinished jobs");
    for (job_key, queue_data) in queue_db.iter().flatten() {
        let job: Job = bincode::deserialize(&job_key).unwrap();
        debug!("Sending {job:?}");
        if let Ok(job_status) = bincode::deserialize::<JobStatus>(&queue_data) {
            match job_status {
                JobStatus::LocalZkProofPending | JobStatus::RemoteZkProofPending(_) => {
                    let _ = job_sender
                        .send(Some(job))
                        .map_err(|e| error!("Failed to send existing job to worker: {}", e));
                }
                _ => {
                    error!("Unexpected job in queue! DB is in invalid state!")
                }
            }
        }
    }

    loop {
        let (stream, _) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();
        let runner = pda_runner.clone();
        let celestia_client = celesita_client.clone();

        tokio::spawn(async move {
            match tls_acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    let io = TokioIo::new(tls_stream);

                    let service = service_fn(move |mut plaintext_req: Request<IncomingBody>| {
                        let scheme = if std::env::var("UNSAFE_HTTP_UPSTREAM").is_ok() {
                            "http"
                        } else {
                            "https"
                        };
                        let uri_string = format!(
                            "{}://{}{}",
                            scheme,
                            da_node_socket.clone(),
                            plaintext_req
                                .uri()
                                .path_and_query()
                                .map(|x| x.as_str())
                                .unwrap_or("/")
                        );
                        let uri = uri_string.parse().unwrap();
                        *plaintext_req.uri_mut() = uri;

                        let runner = runner.clone();
                        let celestia_client = celestia_client.clone();

                        async move {
                            let mut request_method: String = Default::default();
                            let maybe_wrapped_req =
                                inbound_handler(plaintext_req, &mut request_method, runner).await?;

                            match maybe_wrapped_req {
                                Some(wrapped_req) => {
                                    let returned = celestia_client.request(wrapped_req).await?;
                                    let wrapped_resp =
                                        outbound_handler(returned, request_method).await?;
                                    anyhow::Ok(wrapped_resp)
                                }
                                None => {
                                    let raw_json = r#"{ "id": 1, "jsonrpc": "2.0", "status": "Verifiable encryption processing... Call back for result" }"#;
                                    let new_body = Full::new(Bytes::from(raw_json))
                                        .map_err(|err: std::convert::Infallible| match err {})
                                        .boxed();
                                    let new_response = Response::new(BoxBody::new(new_body));

                                    let (mut parts, body) = new_response.into_parts();
                                    parts.status = hyper::StatusCode::BAD_REQUEST;
                                    let response = Response::from_parts(parts, body);
                                    anyhow::Ok(response)
                                }
                            }
                        }
                    });

                    if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                        error!("Failed to serve the connection: {:?}", err);
                    }
                }
                Err(e) => error!("TLS handshake failed: {:?}", e),
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
) -> Result<Option<Request<BoxBody>>> {
    let (mut parts, body_stream) = req.into_parts();
    let mut body_buf = body_stream.collect().await?.aggregate();
    let body_bytes = body_buf.copy_to_bytes(body_buf.remaining());
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes)?;

    if let Some(method) = body_json.get("method").and_then(|m| m.as_str()) {
        *request_method = method.to_string();

        match method {
            // <https://node-rpc-docs.celestia.org/#blob.Submit>
            "blob.Submit" => {
                debug!("blob.Submit intercept");
                if let Some(params_raw) = body_json.get("params") {
                    let params: ParamsGet = serde_json::from_value(params_raw.clone())?;
                    // TODO: consider only allowing one blob and one job on the queue
                    for mut blob in params.blobs {
                        let pda_runner = pda_runner.clone();
                        let data = blob.data.to_owned();
                        let input = Input {
                            data: data.to_owned(),
                        };
                        let hash = Sha256::digest(&data);
                        let anchor = Anchor {
                            data: hash.as_slice().into(),
                        };

                        let job = Job { anchor, input };
                        #[allow(unused_assignments)] // mutating `blob` in place
                        if let Some(proof_with_values) =
                            pda_runner.get_verifiable_encryption(job).await?
                        {
                            let encrypted_data = bincode::serialize(&proof_with_values)?;
                            let encrypted_blob = Blob::new(
                                blob.namespace,
                                encrypted_data,
                                celestia_types::AppVersion::latest(),
                            )?;
                            blob = encrypted_blob;
                        } else {
                            return Ok(None);
                        }
                    }
                } else {
                    debug!("Forwarding `blob.Submit` error");
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

    Ok(Some(Request::from_parts(parts, new_body)))
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
    let mut body_buf = body_stream.collect().await?.aggregate();
    let body_bytes = body_buf.copy_to_bytes(body_buf.remaining());
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes)?;

    match request_method.as_str() {
        // <https://node-rpc-docs.celestia.org/#blob.Get>
        "blob.Get" => {
            debug!("blob.Get intercept");
            if let Some(result_raw) = body_json.get("result") {
                debug!("{result_raw:?}");
                let blob: Blob = serde_json::from_value(result_raw.clone())?;
                // TODO: SP1 Verify encryption & anchors proof, Decrypt data
                // TODO: Return {custom?} error and/or decrypted data
                debug!("{blob:?}");
            } else {
                debug!("Forwarding `blob.Get` error");
            }
        }
        // <https://node-rpc-docs.celestia.org/#blob.Get>
        "blob.GetAll" => {
            debug!("blob.GetAll intercept");
            if let Some(result_raw) = body_json.get("result") {
                debug!("{result_raw:?}");
                let blobs: Vec<Blob> = serde_json::from_value(result_raw.clone())?;
                for blob in blobs {
                    debug!("{blob:?}");
                }
            } else {
                debug!("Forwarding `blob.GetAll` error");
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
