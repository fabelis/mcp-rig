use dotenv::dotenv;
use mcp_core::{
    client::Client,
    transport::{ClientSseTransport, Transport},
    types::Implementation,
};
use serde_json::json;
use std::{env, sync::Arc};

const DISCORD_SERVER_URL: &str = "https://discord-mcp.fabelis.ai";
const TWITTER_SERVER_URL: &str = "https://twitter-mcp.fabelis.ai";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt().init();

    dotenv().ok();

    let transport = ClientSseTransport::builder(TWITTER_SERVER_URL.to_string()).build();
    transport.open().await?;

    let mcp_client = Arc::new(
        Client::builder(transport)
            .with_secure_value(
                "twitter_api_key",
                mcp_core::client::SecureValue::Env("TWITTER_API_KEY".to_string()),
            )
            .with_secure_value(
                "twitter_api_secret",
                mcp_core::client::SecureValue::Env("TWITTER_API_SECRET".to_string()),
            )
            .with_secure_value(
                "twitter_access_token",
                mcp_core::client::SecureValue::Env("TWITTER_ACCESS_TOKEN".to_string()),
            )
            .with_secure_value(
                "twitter_access_token_secret",
                mcp_core::client::SecureValue::Env("TWITTER_ACCESS_TOKEN_SECRET".to_string()),
            )
            .use_strict()
            .build(),
    );
    let mcp_client_clone = mcp_client.clone();
    tokio::spawn(async move { mcp_client_clone.start().await });

    let init_res = mcp_client
        .initialize(Implementation {
            name: "mcp-client".to_string(),
            version: "0.1.0".to_string(),
        })
        .await?;
    println!("Initialized: {:?}", init_res);

    let tools_list_res = mcp_client.list_tools(None, None).await?;
    println!("Tools: {:?}", tools_list_res);

    tracing::info!("Running Direct example");

    let tool_res = mcp_client
        .call_tool(
            "Post",
            Some(json!({
                "tweet": "hello",
                "twitter_api_key": "",
                "twitter_api_secret": "",
                "twitter_access_token": "",
                "twitter_access_token_secret": ""
            })),
        )
        .await?;

    println!("Tool response: {:?}", tool_res);

    Ok(())
}
