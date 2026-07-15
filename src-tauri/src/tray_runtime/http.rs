use std::{
    fs::{self, OpenOptions},
    io::Write,
    net::SocketAddr,
    os::unix::fs::OpenOptionsExt,
    sync::Arc,
};

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Json, Router,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{TrayError, TrayPaths};
use crate::{
    mcp::{McpAuthorization, VibeInspectorMcp},
    storage::InspectorDatabase,
};

#[derive(Clone)]
struct HttpAuthorization {
    token: Arc<str>,
    port: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HttpError {
    code: &'static str,
    message: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EndpointMetadata<'a> {
    schema_version: u32,
    transport: &'static str,
    url: &'a str,
    token_path: String,
}

pub async fn bind_mcp_listener(address: SocketAddr) -> Result<TcpListener, TrayError> {
    if !address.ip().is_loopback() {
        return Err(TrayError::Config(
            "MCP HTTP must bind to a loopback address".to_string(),
        ));
    }
    Ok(TcpListener::bind(address).await?)
}

pub async fn serve_mcp_http(
    listener: TcpListener,
    database: InspectorDatabase,
    bearer_token: String,
    paths: TrayPaths,
    cancellation: CancellationToken,
) -> Result<(), TrayError> {
    let address = listener.local_addr()?;
    let endpoint = format!("http://{address}/mcp");
    write_endpoint_metadata(&paths, &endpoint)?;

    let factory_database = database.clone();
    let service: StreamableHttpService<VibeInspectorMcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || {
                let projects = factory_database
                    .list_projects(false)
                    .map_err(|error| std::io::Error::other(error.to_string()))?;
                let authorization = McpAuthorization::for_local_tray(
                    format!("tray-http:{}", Uuid::now_v7()),
                    projects.into_iter().map(|project| project.id),
                )
                .map_err(|error| std::io::Error::other(error.to_string()))?;
                Ok(VibeInspectorMcp::new(
                    factory_database.clone(),
                    authorization,
                ))
            },
            Default::default(),
            StreamableHttpServerConfig::default()
                .with_sse_keep_alive(None)
                .with_cancellation_token(cancellation.child_token()),
        );

    let authorization = HttpAuthorization {
        token: bearer_token.into(),
        port: address.port(),
    };
    let router = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(
            authorization,
            authorize_request,
        ));

    axum::serve(listener, router)
        .with_graceful_shutdown(cancellation.cancelled_owned())
        .await
        .map_err(|error| TrayError::Runtime(error.to_string()))?;
    Ok(())
}

async fn authorize_request(
    State(authorization): State<HttpAuthorization>,
    request: Request,
    next: Next,
) -> Response {
    match request_authorization_error(request.headers(), &authorization) {
        None => next.run(request).await,
        Some((status, code, message)) => {
            (status, Json(HttpError { code, message })).into_response()
        }
    }
}

fn request_authorization_error(
    headers: &HeaderMap,
    authorization: &HttpAuthorization,
) -> Option<(StatusCode, &'static str, &'static str)> {
    let expected_hosts = [
        format!("127.0.0.1:{}", authorization.port),
        format!("localhost:{}", authorization.port),
        format!("[::1]:{}", authorization.port),
    ];
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    if !host.is_some_and(|host| expected_hosts.iter().any(|expected| expected == host)) {
        return Some((
            StatusCode::FORBIDDEN,
            "invalid_host",
            "MCP requests must target the local endpoint",
        ));
    }

    if let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        let allowed = [
            format!("http://127.0.0.1:{}", authorization.port),
            format!("http://localhost:{}", authorization.port),
            format!("http://[::1]:{}", authorization.port),
        ];
        if !allowed.iter().any(|expected| expected == origin) {
            return Some((
                StatusCode::FORBIDDEN,
                "invalid_origin",
                "browser origins are not authorized for this local MCP endpoint",
            ));
        }
    }

    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if !provided.is_some_and(|provided| constant_time_eq(provided, &authorization.token)) {
        return Some((
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "a valid local MCP bearer token is required",
        ));
    }
    None
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn write_endpoint_metadata(paths: &TrayPaths, endpoint: &str) -> Result<(), TrayError> {
    let metadata = serde_json::to_vec_pretty(&EndpointMetadata {
        schema_version: 1,
        transport: "streamable-http",
        url: endpoint,
        token_path: paths.token_path.to_string_lossy().into_owned(),
    })
    .map_err(|error| TrayError::Protocol(error.to_string()))?;
    let temporary_path = paths
        .app_support_dir
        .join(format!(".mcp-endpoint-{}.tmp", Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary_path)?;
    file.write_all(&metadata)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(temporary_path, &paths.endpoint_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn authorization() -> HttpAuthorization {
        HttpAuthorization {
            token: Arc::from("0123456789abcdef"),
            port: 47821,
        }
    }

    #[test]
    fn bearer_host_and_origin_are_all_enforced() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:47821"));
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer 0123456789abcdef"),
        );
        assert!(request_authorization_error(&headers, &authorization()).is_none());

        headers.insert(header::HOST, HeaderValue::from_static("attacker.example"));
        assert_eq!(
            request_authorization_error(&headers, &authorization())
                .unwrap()
                .0,
            StatusCode::FORBIDDEN
        );

        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:47821"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://attacker.example"),
        );
        assert_eq!(
            request_authorization_error(&headers, &authorization())
                .unwrap()
                .0,
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn bearer_comparison_rejects_wrong_or_different_length_values() {
        assert!(constant_time_eq("same", "same"));
        assert!(!constant_time_eq("same", "diff"));
        assert!(!constant_time_eq("short", "longer"));
    }
}
