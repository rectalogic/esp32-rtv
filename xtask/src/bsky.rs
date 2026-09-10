use anyhow::Context;
use atrium_common::resolver::Resolver;
use atrium_identity::{
    did::{CommonDidResolver, CommonDidResolverConfig},
    handle::{WellKnownHandleResolver, WellKnownHandleResolverConfig},
    identity_resolver::{IdentityResolver, IdentityResolverConfig},
};
#[cfg(not(target_os = "espidf"))]
use atrium_xrpc_client::reqwest::ReqwestClient;
#[cfg(target_os = "espidf")]
use dummy::ReqwestClient;

use clap::{Args, Subcommand};
use std::sync::Arc;

#[cfg(target_os = "espidf")]
pub mod dummy {
    pub struct ReqwestClient;

    impl ReqwestClient {
        pub fn new(_url: &str) -> Self {
            Self
        }
    }

    impl atrium_xrpc::HttpClient for ReqwestClient {
        async fn send_http(
            &self,
            _request: atrium_xrpc::http::Request<Vec<u8>>,
        ) -> Result<
            atrium_xrpc::http::Response<Vec<u8>>,
            Box<dyn std::error::Error + Send + Sync + 'static>,
        > {
            Err(anyhow::anyhow!("Not implemented").into())
        }
    }
}

#[derive(Args)]
#[command(flatten_help = true)]
pub struct BskyArgs {
    #[command(subcommand)]
    command: BskyCommands,
}

#[derive(Subcommand)]
enum BskyCommands {
    /// Resolve bluesky handle to DID and PDS
    Resolve {
        /// Handle to resolve
        handle: String,
    },
}

#[cfg(target_os = "espidf")]
pub fn bsky(_bsky_args: &BskyArgs) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "espidf"))]
#[tokio::main]
pub async fn bsky(bsky_args: &BskyArgs) -> anyhow::Result<()> {
    match &bsky_args.command {
        BskyCommands::Resolve { handle } => resolve(handle).await,
    }
}

async fn resolve(handle: &str) -> anyhow::Result<()> {
    // Base URL doesn't matter for identity resolution,
    let http_client = Arc::new(ReqwestClient::new("https://bsky.social"));

    let did_resolver = CommonDidResolver::new(CommonDidResolverConfig {
        plc_directory_url: "https://plc.directory".to_string(),
        http_client: http_client.clone(),
    });
    let handle_resolver = WellKnownHandleResolver::new(WellKnownHandleResolverConfig {
        http_client: http_client.clone(),
    });
    let identity_resolver = IdentityResolver::new(IdentityResolverConfig {
        did_resolver,
        handle_resolver,
    });
    let resolved = identity_resolver
        .resolve(handle)
        .await
        .context("Failed to resolve identity")?;

    println!("Resolved DID: {}", resolved.did);
    println!("Resolved PDS: {}", resolved.pds);

    Ok(())
}
