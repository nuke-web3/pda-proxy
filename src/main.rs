mod rpc;
use celestia_types::blob::RawBlob;
use rpc::state::{StateClient, StateServer, StateServerImpl};
use rpc::tx_config::TxConfig;
use std::net::SocketAddr;

use jsonrpsee::http_client::HttpClientBuilder;
use jsonrpsee::server::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // tracing_subscriber::FmtSubscriber::builder()
    // 	.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    // 	.try_init()
    // 	.expect("setting default subscriber failed");

    let server_addr = run_server().await?;
    let url = format!("https://{}", server_addr);

    let client = HttpClientBuilder::default().build(&url)?;
    client
        .state_submit_pay_for_blob(
            vec![RawBlob {
                namespace_id: todo!(),
                data: todo!(),
                share_version: todo!(),
                namespace_version: todo!(),
                signer: todo!(),
            }],
            TxConfig {
                signer_address: todo!(),
                key_name: todo!(),
                gas_price: todo!(),
                gas: todo!(),
                fee_granter_address: todo!(),
            },
        )
        .await
        .unwrap();

    Ok(())
}

async fn run_server() -> anyhow::Result<SocketAddr> {
    let server = Server::builder().build("127.0.0.1:0").await?;

    let addr = server.local_addr()?;
    let handle = server.start(StateServerImpl.into_rpc());

    // In this example we don't care about doing shutdown so let's it run forever.
    // You may use the `ServerHandle` to shut it down or manage it yourself.
    tokio::spawn(handle.stopped());

    Ok(addr)
}
