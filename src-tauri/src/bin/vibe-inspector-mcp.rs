use rmcp::ServiceExt;
use vibe_inspector_lib::{
    mcp::{McpAuthorization, VibeInspectorMcp},
    storage::InspectorDatabase,
};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database = InspectorDatabase::for_macos_user()?;
    database.initialize()?;
    let authorization = McpAuthorization::from_env()?;
    let server = VibeInspectorMcp::new(database, authorization)
        .serve(rmcp::transport::stdio())
        .await?;
    server.waiting().await?;
    Ok(())
}
