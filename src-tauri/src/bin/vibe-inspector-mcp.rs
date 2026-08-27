use rmcp::ServiceExt;
use vibe_inspector_lib::{
    mcp::{McpAuthorization, VibeInspectorMcp},
    storage::InspectorDatabase,
    tray_runtime::{
        ensure_tray_running_unless_quit, record_diagnostic, resolve_tray_executable,
        DiagnosticEvent, DiagnosticProcess, EnsureTrayOutcome, TrayError, TrayPaths,
    },
};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ensure_tray_for_stdio_session();
    let database = InspectorDatabase::for_macos_user()?;
    database.initialize()?;
    let authorization = McpAuthorization::from_env()?;
    let server = VibeInspectorMcp::new(database, authorization)
        .serve(rmcp::transport::stdio())
        .await?;
    server.waiting().await?;
    Ok(())
}

fn ensure_tray_for_stdio_session() {
    let Ok(paths) = TrayPaths::for_macos_user() else {
        return;
    };
    let outcome = std::env::current_exe()
        .map_err(TrayError::from)
        .and_then(|current| {
            resolve_tray_executable(&current).ok_or_else(|| {
                TrayError::Config("tray helper executable was not found".to_string())
            })
        })
        .and_then(|executable| ensure_tray_running_unless_quit(&paths, &executable));
    let event = match outcome {
        Ok(EnsureTrayOutcome::Running | EnsureTrayOutcome::Started) => {
            DiagnosticEvent::TrayHelperReady
        }
        Ok(EnsureTrayOutcome::Suppressed) => DiagnosticEvent::TrayStartSuppressed,
        Err(_) => DiagnosticEvent::TrayHelperUnavailable,
    };
    let _ = record_diagnostic(&paths, DiagnosticProcess::Mcp, event);
}
