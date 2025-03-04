use dotenv::dotenv;
use mcp_core::{
    client::Client,
    transport::{ClientSseTransport, Transport},
    types::Implementation,
};
use std::{env, sync::Arc};

use mcp_rig::{completion::Prompt, providers};

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

    tracing::info!("Running RIG example");

    let openai_client = providers::openai::Client::new(
        &env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set"),
    );

    let mut agent_builder = openai_client.agent("gpt-4o")
                // Use preamble to allow for secure values to be used in the tool call
                .preamble("If a tool says fields can use a null value do not ask the user for those values and default them to an empty strings because the MCP Client will add them automatically.");

    // Add MCP tools to the agent
    agent_builder = tools_list_res
        .tools
        .into_iter()
        .fold(agent_builder, |builder, tool| {
            builder.mcp_tool(tool, mcp_client.clone())
        });
    let agent = agent_builder.build();

    let response = agent.prompt("Post 'hello' on twitter").await?;
    tracing::info!("Agent response: {:?}", response);

    Ok(())
}
