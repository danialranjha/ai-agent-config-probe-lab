// Original harness, GPL-3.0-only. No command or server is executed.
mod upstream;
use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const COMMAND: &str = "INERT_ASYAR_COMMAND";
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        for _ in 0..1000 {
            let serial = NEXT.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!("config-probe-lab-{}-{serial}", std::process::id()));
            match fs::create_dir(&p) {
                Ok(()) => return Self(p),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("cannot create owned scratch: {e}"),
            }
        }
        panic!("scratch names unavailable");
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { fs::remove_dir_all(&self.0).expect("owned scratch cleanup failed"); }
}

fn read_bytes(path: &Path) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => panic!("oracle read failed: {e}"),
    }
}
fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn inspect(path: &Path) -> &'static str {
    let Some(bytes) = read_bytes(path) else { return "file_absent"; };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else { return "invalid_json"; };
    let Some(obj) = doc.as_object() else { return "invalid_document"; };
    let Some(servers) = obj.get("mcpServers") else { return "registration_absent"; };
    let Some(servers) = servers.as_object() else { return "invalid_servers"; };
    let Some(entry) = servers.get("asyar") else { return "registration_absent"; };
    if entry.get("command") == Some(&json!(COMMAND)) && entry.get("args") == Some(&json!(["mcp-server"])) {
        "matches_expected_command_and_args"
    } else {
        "different_registration"
    }
}
fn snapshot(path: &Path) -> Value {
    let bytes = read_bytes(path);
    let doc = bytes.as_ref().and_then(|b| serde_json::from_slice::<Value>(b).ok());
    json!({"exists":bytes.is_some(), "parent_exists":path.parent().unwrap().exists(),
        "bytes_hex":bytes.as_ref().map(|b|hex(b)), "parsed":doc, "configuration_status":inspect(path)})
}
fn action(path: &Path, method: &str) -> Value {
    let reported_write = match method {
        "registration_during_check" => Some(upstream::register_mcp_server_in_config(path, COMMAND)),
        "status_only" => None, // Inspection has no registration side effect.
        "explicit_registration" => Some(upstream::register_mcp_server_in_config(path, COMMAND)),
        _ => panic!("unknown method"),
    };
    json!({"reported_write":reported_write, "configuration_status":inspect(path)})
}
fn fixtures() -> Vec<(&'static str, Option<Vec<u8>>)> {
    vec![
        ("missing_file", None),
        ("empty_object", Some(b"{}\n".to_vec())),
        ("unrelated_settings", Some(b"{\"mcpServers\":{\"local_notes\":{\"command\":\"INERT_EXISTING_COMMAND\"}},\"theme\":\"light\"}\n".to_vec())),
        ("removed_registration", Some(b"{\"mcpServers\":{},\"theme\":\"light\"}\n".to_vec())),
        ("already_matching", Some(b"{\"mcpServers\":{\"asyar\":{\"command\":\"INERT_ASYAR_COMMAND\",\"args\":[\"mcp-server\"]}},\"theme\":\"light\"}\n".to_vec())),
        ("changed_command", Some(b"{\"mcpServers\":{\"asyar\":{\"command\":\"INERT_OLD_COMMAND\",\"args\":[\"mcp-server\"]}}}\n".to_vec())),
        ("extra_arguments", Some(b"{\"mcpServers\":{\"asyar\":{\"command\":\"INERT_ASYAR_COMMAND\",\"args\":[\"unused\",\"mcp-server\"]}}}\n".to_vec())),
        ("malformed_json", Some(b"{\"theme\":\"light\",\n".to_vec())),
        ("non_object_json", Some(b"[1,2,3]\n".to_vec())),
        ("malformed_servers", Some(b"{\"mcpServers\":7,\"theme\":\"light\"}\n".to_vec())),
        ("non_utf8", Some(vec![0xff, 0x00, 0x0a])),
    ]
}
fn install_fixture(path: &Path, bytes: &Option<Vec<u8>>) {
    if let Some(bytes) = bytes { fs::create_dir_all(path.parent().unwrap()).unwrap(); fs::write(path, bytes).unwrap(); }
}
fn run_case(name: &str, bytes: &Option<Vec<u8>>, method: &str) -> Value {
    let root=Scratch::new(); let path=root.0.join("nested/config.json");
    install_fixture(&path,bytes);
    let before=snapshot(&path); let result=action(&path,method); let after=snapshot(&path);
    let changed=before["exists"]!=after["exists"] || before["bytes_hex"]!=after["bytes_hex"];
    json!({"case":name,"method":method,"before":before,"after":after,"file_changed":changed,"result":result})
}
fn removal_trace(method:&str) -> Value {
    let root=Scratch::new(); let path=root.0.join("config.json"); fs::write(&path,b"{\"theme\":\"light\"}\n").unwrap();
    action(&path,"explicit_registration"); let installed=snapshot(&path);
    let mut doc=installed["parsed"].clone();
    assert!(doc["mcpServers"].is_object(), "explicit registration must create a server map before the removal trace");
    doc["mcpServers"].as_object_mut().unwrap().remove("asyar");
    fs::write(&path,serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    let removed=snapshot(&path); let result=action(&path,method); let checked=snapshot(&path);
    json!({"check_method":method,"installed":installed,"removed":removed,"after_check":checked,"result":result})
}
fn experiment() -> Value {
    let mut rows=Vec::new();
    for (name,bytes) in fixtures() { for method in ["registration_during_check","status_only","explicit_registration"] { rows.push(run_case(name,&bytes,method)); } }
    json!({"schema":1,"scope":"unchanged registration helper, invented temporary configuration; no app/CLI/model/MCP execution",
        "fixture_count":11,"method_count":3,"rows":rows,
        "removal_traces":[removal_trace("registration_during_check"),removal_trace("status_only")]})
}
fn main() { println!("{}",serde_json::to_string_pretty(&experiment()).unwrap()); }

#[cfg(test)]
mod tests {
    use super::*;
    fn row(name:&str,method:&str)->Value { let (_,b)=fixtures().into_iter().find(|(n,_)| *n==name).unwrap();run_case(name,&b,method) }
    #[test] fn status_preserves_every_raw_fixture() {
        for (n,b) in fixtures() { let r=run_case(n,&b,"status_only");assert_eq!(r["before"],r["after"],"{n}"); }
    }
    #[test] fn registration_adds_absent_entry() {
        for n in ["missing_file","empty_object","removed_registration"] { let r=row(n,"registration_during_check");assert_eq!(r["file_changed"],true); assert_eq!(r["after"]["parsed"]["mcpServers"]["asyar"],json!({"command":COMMAND,"args":["mcp-server"]})); }
    }
    #[test] fn explicit_registration_is_functional() { let r=row("empty_object","explicit_registration");assert_eq!(r["file_changed"],true);assert_eq!(r["after"]["parsed"]["mcpServers"]["asyar"]["command"],COMMAND); }
    #[test] fn valid_unrelated_keys_survive_registration() { let r=row("unrelated_settings","registration_during_check");assert_eq!(r["before"]["parsed"]["theme"],r["after"]["parsed"]["theme"]);assert_eq!(r["before"]["parsed"]["mcpServers"]["local_notes"],r["after"]["parsed"]["mcpServers"]["local_notes"]); }
    #[test] fn matching_twin_is_byte_unchanged() { for m in ["registration_during_check","status_only","explicit_registration"] {let r=row("already_matching",m);assert_eq!(r["file_changed"],false);assert_eq!(r["before"],r["after"]);} }
    #[test] fn changed_command_is_replaced_only_by_registration() { let r=row("changed_command","registration_during_check"); assert_eq!(r["file_changed"],true);assert_eq!(r["after"]["parsed"]["mcpServers"]["asyar"]["command"],COMMAND);assert_eq!(row("changed_command","status_only")["file_changed"],false); }
    #[test] fn extra_args_are_not_strict_match_but_helper_does_not_write() { let r=row("extra_arguments","registration_during_check");assert_eq!(r["file_changed"],false);assert_eq!(r["result"]["reported_write"],false);assert_eq!(r["after"]["configuration_status"],"different_registration"); }
    #[test] fn invalid_bytes_are_preserved_by_inspection() { for n in ["malformed_json","non_object_json","malformed_servers","non_utf8"] {let r=row(n,"status_only");assert_eq!(r["before"],r["after"]);assert!(r["after"]["configuration_status"].as_str().unwrap().starts_with("invalid"));} }
    #[test] fn original_registration_replaces_malformed_documents() { for n in ["malformed_json","non_object_json","non_utf8"] {let r=row(n,"registration_during_check");assert_eq!(r["file_changed"],true);assert_eq!(r["after"]["parsed"].as_object().unwrap().len(),1);} }
    #[test] fn removal_stays_removed_only_in_status_path() { let a=removal_trace("registration_during_check");let b=removal_trace("status_only");assert_eq!(a["removed"],b["removed"]);assert_eq!(b["removed"],b["after_check"]);assert!(a["after_check"]["parsed"]["mcpServers"].get("asyar").is_some());assert!(b["after_check"]["parsed"]["mcpServers"].get("asyar").is_none()); }
    #[test] fn artifact_covers_all_pairs_and_oracle_uses_bytes() { let e=experiment();assert_eq!(e["rows"].as_array().unwrap().len(),33);for r in e["rows"].as_array().unwrap(){assert_eq!(r["file_changed"],json!(r["before"]["exists"]!=r["after"]["exists"] || r["before"]["bytes_hex"]!=r["after"]["bytes_hex"]));} }
}
