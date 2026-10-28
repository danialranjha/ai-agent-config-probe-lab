// Unchanged Asyar helper, GPL-3.0. See source.json and LICENSE.
use std::path::Path;

pub fn register_mcp_server_in_config(config_file: &Path, current_exe_str: &str) -> bool {
    let mut doc: serde_json::Value = if config_file.is_file() {
        match std::fs::read_to_string(config_file) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({})),
            Err(_) => serde_json::json!({}),
        }
    } else {
        serde_json::json!({})
    };

    if !doc.is_object() {
        doc = serde_json::json!({});
    }

    let mcp_servers = match doc.get_mut("mcpServers") {
        Some(v) if v.is_object() => v,
        _ => {
            doc["mcpServers"] = serde_json::json!({});
            &mut doc["mcpServers"]
        }
    };

    let needs_update = match mcp_servers.get("asyar") {
        Some(asyar_entry) => {
            let cmd = asyar_entry.get("command").and_then(|c| c.as_str());
            let args = asyar_entry.get("args").and_then(|a| a.as_array());
            let has_mcp_arg =
                args.is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("mcp-server")));
            cmd != Some(current_exe_str) || !has_mcp_arg
        }
        None => true,
    };

    if needs_update {
        if let Some(servers_map) = mcp_servers.as_object_mut() {
            servers_map.insert(
                "asyar".to_string(),
                serde_json::json!({
                    "command": current_exe_str,
                    "args": ["mcp-server"]
                }),
            );

            if let Some(parent) = config_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(formatted) = serde_json::to_string_pretty(&doc) {
                if std::fs::write(config_file, formatted).is_ok() {
                    log::info!("[asyar mcp] Registered asyar MCP server in {config_file:?}");
                    return true;
                }
            }
        }
    }
    false
}
