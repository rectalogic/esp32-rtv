use super::client::EspIdfXrpcClient;
use atrium_api::{
    client::AtpServiceClient,
    com::atproto::repo::list_records,
    types::{
        LimitedNonZeroU8,
        string::{AtIdentifier, Handle, Nsid},
    },
};
use esp_idf_svc::hal::task::block_on;
use std::thread;

const SYNC_THREAD_STACK_SIZE: usize = 32 * 1024;

pub fn sync_videos() -> anyhow::Result<()> {
    let sync_thread = thread::Builder::new()
        .name("bsky-sync".to_owned())
        .stack_size(SYNC_THREAD_STACK_SIZE)
        .spawn(run_sync)?;

    match sync_thread.join() {
        Ok(result) => result,
        Err(panic) => Err(anyhow::anyhow!("bsky sync task panicked: {panic:?}")),
    }
}

fn run_sync() -> anyhow::Result<()> {
    let xrpc = EspIdfXrpcClient::new_default(env!("BSKY_PDS"))?;

    block_on(async {
        let client = AtpServiceClient::new(xrpc);
        let page = client
            .service
            .com
            .atproto
            .repo
            .list_records(
                list_records::ParametersData {
                    repo: AtIdentifier::from(
                        Handle::new(env!("BSKY_HANDLE").into()).map_err(|s| anyhow::anyhow!(s))?,
                    ),
                    collection: Nsid::new("app.bsky.feed.post".to_string())
                        .map_err(|s| anyhow::anyhow!(s))?,
                    limit: Some(
                        LimitedNonZeroU8::<100u8>::try_from(5).map_err(|s| anyhow::anyhow!(s))?,
                    ),
                    cursor: None, //cursor.clone(),
                    reverse: None,
                }
                .into(),
            )
            .await?;
        log::info!("{page:?}");
        Ok(())
    })
}
