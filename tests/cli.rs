//! Integration tests for the `info`, `list` and `view` commands.

use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    let mut c = Command::cargo_bin("sprcmtrd").unwrap();
    c.current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("RUST_LOG")
        .env("SPRCMTRD_CONFIG", "/nonexistent/config.toml");
    c
}

#[test]
fn info_prints_summary_and_tables() {
    cmd()
        .args(["info", "tests/data/fault_1999.dat"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Station       SUBSTATION A"))
        .stdout(predicate::str::contains("Revision      1999"))
        .stdout(predicate::str::contains("Sample rates  2 kHz (samples 1-400)"))
        .stdout(predicate::str::contains("Analog channels"))
        .stdout(predicate::str::contains("CB_OPEN"))
        .stdout(predicate::str::contains("Synthetic phase A to ground fault"));
}

#[test]
fn info_no_extra_skips_header() {
    cmd()
        .args(["info", "--no-extra", "tests/data/fault_1999.cfg"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Synthetic").not());
}

#[test]
fn info_json_is_valid() {
    let out = cmd()
        .args(["info", "--json", "tests/data/combined_2013.cff"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["config"]["revision"], "Rev2013");
    assert_eq!(v["config"]["data_format"], "Float32");
    assert_eq!(v["samples"], 800);
    assert_eq!(v["config"]["analog"].as_array().unwrap().len(), 4);
    assert!(v["info"].as_str().unwrap().contains("Source=synthetic"));
}

#[test]
fn info_binary32_2013() {
    let out = cmd()
        .args(["info", "--json", "tests/data/fault_2013.dat"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["config"]["revision"], "Rev2013");
    assert_eq!(v["config"]["data_format"], "Binary32");
    assert_eq!(v["config"]["analog"].as_array().unwrap().len(), 8);
    assert_eq!(v["config"]["binary"].as_array().unwrap().len(), 10);
    assert_eq!(v["samples"], 4001);
    assert_eq!(v["duration_s"], 2.0);
    assert_eq!(v["trigger_offset_s"], 0.5);
}

#[test]
fn view_binary32_2013() {
    cmd()
        .args(["view", "tests/data/fault_2013.cfg", "--channels", "IB,CB_CLOSED", "--width", "80"])
        .assert()
        .success()
        .stdout(predicate::str::contains("A2 IB [A]"))
        .stdout(predicate::str::contains("D7 CB_CLOSED"));
}

#[test]
fn list_shows_recordings_and_errors() {
    cmd()
        .args(["list", "tests/data", "--sort", "name"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"(?s)broken\.cfg.*combined_2013\.cff.*fault_1999\.cfg.*fault_2013\.cfg.*old_1991\.cfg").unwrap())
        .stdout(predicate::str::contains("channel count 5 does not match"));
}

#[test]
fn list_searches_subdirectories_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("station/2024");
    std::fs::create_dir_all(&nested).unwrap();
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    for f in ["fault_1999.cfg", "fault_1999.dat"] {
        std::fs::copy(format!("{data}/{f}"), nested.join(f)).unwrap();
    }
    std::fs::copy(format!("{data}/old_1991.cfg"), dir.path().join("old_1991.cfg")).unwrap();
    std::fs::copy(format!("{data}/old_1991.dat"), dir.path().join("old_1991.dat")).unwrap();

    cmd()
        .arg("list")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("station/2024/fault_1999.cfg"))
        .stdout(predicate::str::contains("old_1991.cfg"));
    cmd()
        .args(["list", "--no-recursive"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("fault_1999").not())
        .stdout(predicate::str::contains("old_1991.cfg"));
}

#[test]
fn list_filter() {
    cmd()
        .args(["list", "tests/data", "--filter", "^fault"])
        .assert()
        .success()
        .stdout(predicate::str::contains("fault_1999.cfg"))
        .stdout(predicate::str::contains("old_1991").not());
    cmd()
        .args(["list", "tests/data", "--filter", "("])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("invalid --filter"));
}

#[test]
fn view_plots_selected_channels() {
    cmd()
        .args([
            "view", "tests/data/old_1991.cfg", "--channels", "IA,d2", "--width", "80", "--height", "6",
            "--from", "-10", "--to", "40",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("A1 IA [A]"))
        .stdout(predicate::str::contains("D2 TRIP"))
        .stdout(predicate::str::contains("VA").not())
        .stdout(predicate::str::contains("-10.00 ms"))
        .stdout(predicate::str::contains("█"));
}

#[test]
fn view_rejects_unknown_channel_and_empty_window() {
    cmd()
        .args(["view", "tests/data/fault_1999.cfg", "--channels", "XX"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("unknown channel 'XX'"));
    cmd()
        .args(["view", "tests/data/fault_1999.cfg", "--from", "10", "--to", "5"])
        .assert()
        .code(1);
}

#[test]
fn exit_codes() {
    // Missing file.
    cmd().args(["info", "tests/data/nope.cfg"]).assert().code(3);
    // .cfg without .dat.
    cmd()
        .args(["view", "tests/data/broken.cfg"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("missing file"));
    // Invalid content.
    cmd()
        .args(["info", "tests/data/broken.cfg"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("cfg line 2"));
    // Invalid arguments.
    cmd().args(["list", "--sort", "bogus"]).assert().code(2);
}

#[test]
fn invalid_config_file_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "theme = \"purple\"\n").unwrap();
    cmd()
        .env("SPRCMTRD_CONFIG", &path)
        .args(["list", "tests/data"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("invalid config"));
}
