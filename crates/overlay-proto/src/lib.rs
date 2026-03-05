use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverlayMessage {
    pub status: Status,
    pub value: Option<i32>,
    pub error: Option<String>,
    pub timestamp_ms: u64,
}

impl OverlayMessage {
    pub fn ok(value: i32, timestamp_ms: u64) -> Self {
        Self {
            status: Status::Ok,
            value: Some(value),
            error: None,
            timestamp_ms,
        }
    }

    pub fn error(message: impl Into<String>, timestamp_ms: u64) -> Self {
        Self {
            status: Status::Error,
            value: None,
            error: Some(message.into()),
            timestamp_ms,
        }
    }

    pub fn to_line(&self) -> Result<String, ProtocolError> {
        Ok(format!("{}\n", serde_json::to_string(self)?))
    }

    pub fn from_line(line: &str) -> Result<Self, ProtocolError> {
        Ok(serde_json::from_str(line.trim())?)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OverlayConfig {
    pub process: ProcessConfig,
    pub memory: MemoryConfig,
    pub signature: SignatureConfig,
    pub ipc: IpcConfig,
    pub poll: PollConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessConfig {
    pub exe: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryConfig {
    pub module: String,
    pub base_offset: u64,
    pub pointer_offsets: Vec<u64>,
    pub game_manager_imp: Option<u64>,
    pub soul_memory_chains: Option<Vec<Vec<u64>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignatureConfig {
    pub enabled: bool,
    pub pattern: String,
    pub relative_offset: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IpcConfig {
    pub pipe_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PollConfig {
    pub interval_ms: u64,
}

impl OverlayConfig {
    pub fn from_toml(input: &str) -> Result<Self, ProtocolError> {
        Ok(toml::from_str(input)?)
    }
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("json serialization failed: {0}")]
    JsonSerialize(#[from] serde_json::Error),
    #[error("toml parse failed: {0}")]
    Toml(#[from] toml::de::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[process]
exe = "DarkSoulsII.exe"

[memory]
module = "DarkSoulsII.exe"
base_offset = 0
pointer_offsets = [0]
game_manager_imp = 0
soul_memory_chains = [[208, 1168, 244], [208, 1168, 252]]

[signature]
enabled = true
pattern = "AA BB ?? CC"
relative_offset = 0

[ipc]
pipe_name = "SoulMemoryOverlay"

[poll]
interval_ms = 60000
"#;

    #[test]
    fn round_trip_overlay_message() {
        let msg = OverlayMessage::ok(123, 999);
        let line = msg.to_line().expect("serialize");
        let parsed = OverlayMessage::from_line(&line).expect("parse");
        assert_eq!(parsed, msg);
    }

    #[test]
    fn error_message_serialization_contains_error() {
        let msg = OverlayMessage::error("cannot read memory", 42);
        let line = msg.to_line().expect("serialize");
        assert!(line.contains("cannot read memory"));
        let parsed = OverlayMessage::from_line(&line).expect("parse");
        assert_eq!(parsed.status, Status::Error);
        assert_eq!(parsed.value, None);
    }

    #[test]
    fn parse_overlay_config() {
        let cfg = OverlayConfig::from_toml(SAMPLE).expect("config parse");
        assert_eq!(cfg.process.exe, "DarkSoulsII.exe");
        assert_eq!(cfg.memory.pointer_offsets, vec![0]);
        assert_eq!(cfg.memory.game_manager_imp, Some(0));
        assert_eq!(
            cfg.memory.soul_memory_chains,
            Some(vec![vec![208, 1168, 244], vec![208, 1168, 252]])
        );
        assert_eq!(cfg.ipc.pipe_name, "SoulMemoryOverlay");
        assert_eq!(cfg.poll.interval_ms, 60_000);
    }
}
