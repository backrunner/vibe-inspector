use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
};

use uuid::Uuid;

use super::{TrayError, TrayPaths};

const TOKEN_BYTES: usize = 32;
const TOKEN_HEX_LENGTH: usize = TOKEN_BYTES * 2;

pub fn load_or_create_bearer_token(paths: &TrayPaths) -> Result<String, TrayError> {
    paths.ensure_runtime_directory()?;
    match fs::read_to_string(&paths.token_path) {
        Ok(token) => {
            let token = token.trim();
            validate_token(token)?;
            fs::set_permissions(&paths.token_path, fs::Permissions::from_mode(0o600))?;
            Ok(token.to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => create_token(paths),
        Err(error) => Err(error.into()),
    }
}

fn create_token(paths: &TrayPaths) -> Result<String, TrayError> {
    let mut bytes = [0_u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| TrayError::Runtime(format!("OS random source failed: {error}")))?;
    let token = encode_hex(&bytes);
    let temporary_path = paths
        .app_support_dir
        .join(format!(".mcp-token-{}.tmp", Uuid::now_v7()));

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary_path)?;
    file.write_all(token.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(&temporary_path, &paths.token_path)?;
    fs::set_permissions(&paths.token_path, fs::Permissions::from_mode(0o600))?;
    Ok(token)
}

fn validate_token(token: &str) -> Result<(), TrayError> {
    if token.len() != TOKEN_HEX_LENGTH || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(TrayError::Config(
            "the persisted MCP bearer token is malformed".to_string(),
        ));
    }
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::MetadataExt, path::PathBuf};

    use super::*;

    fn temporary_paths() -> TrayPaths {
        let root = std::env::temp_dir().join(format!("vibe-inspector-token-{}", Uuid::now_v7()));
        TrayPaths::from_directories(root.join("runtime"), root.join("agents"))
    }

    #[test]
    fn token_is_stable_private_and_256_bits() {
        let paths = temporary_paths();
        let first = load_or_create_bearer_token(&paths).expect("create token");
        let second = load_or_create_bearer_token(&paths).expect("load token");

        assert_eq!(first, second);
        assert_eq!(first.len(), TOKEN_HEX_LENGTH);
        assert_eq!(
            fs::metadata(&paths.token_path).unwrap().mode() & 0o777,
            0o600
        );

        fs::remove_dir_all(paths.app_support_dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn malformed_persisted_token_is_rejected() {
        let paths = temporary_paths();
        fs::create_dir_all(&paths.app_support_dir).unwrap();
        fs::write(&paths.token_path, "not-a-secret\n").unwrap();

        assert!(matches!(
            load_or_create_bearer_token(&paths),
            Err(TrayError::Config(_))
        ));

        let root = PathBuf::from(paths.app_support_dir.parent().unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
