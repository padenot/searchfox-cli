use std::process::Command;

fn help(flag: &str, marker: Option<&str>) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_searchfox-cli"));
    for name in [
        "CLAUDECODE",
        "CODEX_THREAD_ID",
        "CODEX_SESSION_ID",
        "CODEX_SANDBOX",
        "GEMINI_CLI",
        "OPENCODE",
    ] {
        command.env_remove(name);
    }
    if let Some(marker) = marker {
        command.env(marker, "test");
    }
    let output = command.arg(flag).output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn codex_gets_compact_help_independently_of_sandboxing() {
    for marker in ["CODEX_THREAD_ID", "CODEX_SESSION_ID", "CODEX_SANDBOX"] {
        for flag in ["--help", "-h"] {
            assert!(help(flag, Some(marker)).starts_with("searchfox-cli: Mozilla code search\n"));
        }
    }
    assert!(help("--help", None).contains("Usage: searchfox-cli [OPTIONS]"));
}
