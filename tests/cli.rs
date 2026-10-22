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
fn info_ascii_1999_long() {
    let out = cmd()
        .args(["info", "--json", "tests/data/reclose_1999_ascii.dat"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["config"]["revision"], "Rev1999");
    assert_eq!(v["config"]["data_format"], "Ascii");
    assert_eq!(v["config"]["analog"].as_array().unwrap().len(), 6);
    assert_eq!(v["config"]["binary"].as_array().unwrap().len(), 6);
    assert_eq!(v["config"]["sample_rates"][0]["rate"], 1000.0);
    assert_eq!(v["samples"], 4001);
    assert_eq!(v["duration_s"], 4.0);
    assert_eq!(v["trigger_offset_s"], 1.0);
    // The whole .dat parses (view reads every sample).
    cmd()
        .args([
            "view",
            "tests/data/reclose_1999_ascii.cfg",
            "--channels",
            "IA,LOCKOUT",
            "--width",
            "80",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("D6 LOCKOUT"));
}

#[test]
fn view_binary32_2013() {
    cmd()
        .args([
            "view",
            "tests/data/fault_2013.cfg",
            "--channels",
            "IB,CB_CLOSED",
            "--width",
            "80",
        ])
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
        .stdout(
            predicate::str::is_match(
                r"(?s)broken\.cfg.*combined_2013\.cff.*fault_1999\.cfg.*fault_2013\.cfg.*fault_2013_uppercase\.cfg.*old_1991\.cfg.*reclose_1999_ascii\.cfg.*test_ascii\.cff.*test_ascii_uppercase\.CFF",
            )
            .unwrap(),
        )
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
        // Path shown relative to DIR, with the platform's separator.
        .stdout(predicate::str::contains(
            std::path::Path::new("station")
                .join("2024")
                .join("fault_1999.cfg")
                .display()
                .to_string(),
        ))
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
fn list_valid_hides_broken_recordings() {
    let dir = tempfile::tempdir().unwrap();
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    for f in ["broken.cfg", "fault_1999.cfg", "fault_1999.dat", "old_1991.cfg"] {
        std::fs::copy(format!("{data}/{f}"), dir.path().join(f)).unwrap();
    }

    // old_1991.cfg parses but has no data file.
    cmd()
        .args(["list", "--valid"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("fault_1999.cfg"))
        .stdout(predicate::str::contains("broken.cfg").not())
        .stdout(predicate::str::contains("old_1991.cfg").not())
        .stdout(
            predicate::str::contains("error")
                .not()
                .or(predicate::str::is_match(r"error\s*\n").unwrap()),
        );

    std::fs::remove_file(dir.path().join("fault_1999.cfg")).unwrap();
    cmd()
        .args(["list", "--valid"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout("")
        .stderr(predicate::str::contains("no valid COMTRADE recordings"));
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
            "view",
            "tests/data/old_1991.cfg",
            "--channels",
            "IA,d2",
            "--width",
            "80",
            "--height",
            "6",
            "--from",
            "-10",
            "--to",
            "40",
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
fn completions_to_stdout_and_file() {
    cmd()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete -o nospace -o bashdefault"))
        .stdout(predicate::str::contains("COMPLETE=\"bash\""));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("_sprcmtrd");
    cmd()
        .args(["completions", "zsh", "--output"])
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("wrote zsh completions"));
    assert!(std::fs::read_to_string(&path).unwrap().contains("#compdef sprcmtrd"));

    // Missing directory: error with the path, exit code 3 (file not found).
    cmd()
        .args(["completions", "fish", "-o"])
        .arg(dir.path().join("missing/dir/sprcmtrd.fish"))
        .assert()
        .code(3)
        .stderr(predicate::str::contains("creating"));
    cmd().args(["completions", "tcsh"]).assert().code(2);
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

#[test]
fn convert_writes_cfg_dat_and_cff() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("fault.cff");
    cmd()
        .args(["convert", "tests/data/fault_1999.cfg", "--output"])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicate::str::contains("(1999 BINARY) ->"))
        .stdout(predicate::str::contains("(2013 BINARY)"))
        .stdout(predicate::str::contains(
            "read back: 400 samples, 4 analog, 4 binary channels",
        ));
    // The .hdr of the source goes into the combined file.
    cmd()
        .arg("info")
        .arg(&out)
        .assert()
        .success()
        .stdout(predicate::str::contains("Revision      2013"));

    let out = dir.path().join("float.cfg");
    cmd()
        .args([
            "convert",
            "tests/data/combined_2013.cff",
            "--rev",
            "1999",
            "--format",
            "binary",
            "-o",
        ])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicate::str::contains("note: IA: rescaled to 16-bit integers"));
    assert!(dir.path().join("float.dat").exists());
    assert!(dir.path().join("float.hdr").exists());
    cmd()
        .args(["info", "--json"])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"data_format\": \"Binary\""));
}

#[test]
fn convert_refuses_bad_targets_and_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("x.cfg");
    let convert = |args: &[&str], out: &std::path::Path| {
        cmd()
            .args(["convert", "tests/data/fault_1999.cfg"])
            .args(args)
            .arg("-o")
            .arg(out)
            .assert()
    };
    convert(&["--rev", "1999", "--format", "float32"], &out)
        .code(1)
        .stderr(predicate::str::contains("FLOAT32 data needs revision 2013"));
    convert(&["--rev", "1999"], &dir.path().join("x.cff"))
        .code(1)
        .stderr(predicate::str::contains("needs revision 2013"));
    convert(&[], &dir.path().join("x.txt"))
        .code(1)
        .stderr(predicate::str::contains("must end in .cfg or .cff"));
    assert!(!out.exists());

    convert(&[], &out).success();
    convert(&[], &out)
        .code(1)
        .stderr(predicate::str::contains("already exists (use --force"));
    convert(&["--force"], &out).success();

    // Never the source itself, not even with --force.
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    for f in ["fault_1999.cfg", "fault_1999.dat"] {
        std::fs::copy(format!("{data}/{f}"), dir.path().join(f)).unwrap();
    }
    let src = dir.path().join("fault_1999.cfg");
    cmd()
        .arg("convert")
        .arg(&src)
        .args(["--format", "ascii", "--force", "-o"])
        .arg(&src)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("part of the source recording"));
    assert_eq!(
        std::fs::read(&src).unwrap(),
        std::fs::read(format!("{data}/fault_1999.cfg")).unwrap()
    );

    cmd().args(["convert", "missing.cfg", "-o"]).arg(&out).assert().code(3);
}

#[test]
fn export_csv_with_selection() {
    cmd()
        .args(["export", "tests/data/fault_1999.cfg"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with(
            "sample,time_ms,IA [A],IB [A],IC [A],VA [kV],START,TRIP,CB_OPEN,SPARE\n\
             1,-60,0,-346.4,346.4,0,0,0,0,0\n2,-59.5,62.6,-373.4,310.8,14.08,0,0,0,0\n",
        ));

    // 2 kHz: -1 ms .. 1 ms is 5 samples, every 2nd of them is 3.
    let out = cmd()
        .args([
            "export",
            "tests/data/fault_1999.cfg",
            "--channels",
            "IB,d2",
            "--from",
            "-1",
            "--to",
            "1",
        ])
        .args(["--every", "2", "--absolute"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "sample,time,IB [A],TRIP");
    assert_eq!(lines.len(), 4, "{text}");
    assert!(lines[1].starts_with("119,2024-03-15T08:30:00.059000,"), "{text}");

    cmd()
        .args(["export", "tests/data/fault_1999.cfg", "--channels", "NOPE"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("unknown channel 'NOPE'"));
    cmd()
        .args(["export", "tests/data/fault_1999.cfg", "--every", "0"])
        .assert()
        .code(2);
}

#[test]
fn export_json_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("old.json");
    cmd()
        .args(["export", "tests/data/old_1991.cfg", "--channels", "IA,TRIP", "-o"])
        .arg(&out)
        .assert()
        .success()
        .stderr(predicate::str::contains("wrote 100 samples × 2 channels"));
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(v["revision"], "1991");
    assert_eq!(v["side"], "primary");
    assert_eq!(v["time_ms"].as_array().unwrap().len(), 100);
    assert_eq!(v["analog"][0]["id"], "IA");
    // 99999 marks a missing value in the 1991 data file.
    assert!(v["analog"][0]["values"][50].is_null());
    assert_eq!(v["binary"][0]["values"][50], 1);

    // Never over a file of the recording.
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    for f in ["old_1991.cfg", "old_1991.dat"] {
        std::fs::copy(format!("{data}/{f}"), dir.path().join(f)).unwrap();
    }
    let dat = dir.path().join("old_1991.dat");
    cmd()
        .arg("export")
        .arg(dir.path().join("old_1991.cfg"))
        .arg("-o")
        .arg(&dat)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("part of the source recording"));
    assert_eq!(
        std::fs::read(&dat).unwrap(),
        std::fs::read(format!("{data}/old_1991.dat")).unwrap()
    );
}

#[test]
fn cut_window_and_channels() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("short.cfg");
    cmd()
        .args(["cut", "tests/data/fault_2013.cfg", "--from", "-20", "--to", "100"])
        .args(["--channels", "IA,IB,IC,TRIP_A,CB_CLOSED", "-o"])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicate::str::contains("(2013 BINARY32)"))
        .stdout(predicate::str::contains(
            "kept samples 961..=1201 of 4001 (-20.0 ms .. 100.0 ms), 3 of 8 analog and 2 of 10 binary channels",
        ))
        .stdout(predicate::str::contains(
            "read back: 241 samples, 3 analog, 2 binary channels",
        ));

    // Same values at the same times relative to the trigger as the source.
    let export = |file: &std::path::Path, channels: &str| {
        let out = cmd()
            .arg("export")
            .arg(file)
            .args(["--channels", channels, "--from", "-5", "--to", "5"])
            .output()
            .unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        // Without the sample number, which starts at 1 again in the cut file.
        text.lines()
            .map(|l| l.split_once(',').unwrap().1.to_owned())
            .collect::<Vec<_>>()
    };
    let cut = export(&out, "IA,IB,IC,TRIP_A,CB_CLOSED");
    assert_eq!(cut.len(), 22);
    assert_eq!(
        cut,
        export(
            std::path::Path::new("tests/data/fault_2013.cfg"),
            "IA,IB,IC,TRIP_A,CB_CLOSED"
        )
    );
}

#[test]
fn cut_reports_trigger_and_bad_windows() {
    let dir = tempfile::tempdir().unwrap();
    cmd()
        .args(["cut", "tests/data/fault_1999.cfg", "--from", "50", "-o"])
        .arg(dir.path().join("late.cff"))
        .assert()
        .success()
        .stdout(predicate::str::contains("(2013 BINARY)"))
        .stdout(predicate::str::contains("note: the trigger is outside the window"));
    cmd()
        .args(["cut", "tests/data/fault_1999.cfg", "--from", "500", "-o"])
        .arg(dir.path().join("x.cfg"))
        .assert()
        .code(1)
        .stderr(predicate::str::contains("the recording runs from -60.0 ms to 139.5 ms"));
    // Between two samples (2 kHz): nothing to keep.
    cmd()
        .args(["cut", "tests/data/fault_1999.cfg", "--from", "0.1", "--to", "0.2", "-o"])
        .arg(dir.path().join("x.cfg"))
        .assert()
        .code(1)
        .stderr(predicate::str::contains("no samples in the time window"));
    assert!(!dir.path().join("x.cfg").exists());
}

#[test]
fn stats_text_and_json() {
    cmd()
        .args(["stats", "tests/data/fault_2013.cfg", "--channels", "IB,CB_CLOSED"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Window  -0.500 s .. 1.500 s (4001 samples)"))
        .stdout(predicate::str::is_match(r"A2 IB +A +-4\.602k .* 7\.244k +0\.011 s").unwrap())
        .stdout(predicate::str::is_match(r"D7 CB_CLOSED +1 +1 +2 +0\.100 s +1\.200 s").unwrap());

    // `stat` is an alias; 99999 marks a missing value in the 1991 data file.
    let out = cmd()
        .args(["stat", "tests/data/old_1991.cfg", "--channels", "IA,TRIP", "--json"])
        .args(["--from", "0", "--to", "30"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["samples"], 31);
    let ia = &v["analog"][0];
    assert_eq!(ia["id"], "IA");
    assert_eq!(ia["samples"].as_u64().unwrap() + ia["missing"].as_u64().unwrap(), 31);
    assert_eq!(ia["missing"], 1);
    assert_eq!(v["binary"][0]["id"], "TRIP");
    assert_eq!(v["binary"][0]["first_change"], 0.02);
}

#[test]
fn events_timeline() {
    cmd()
        .args(["events", "tests/data/fault_2013.cfg"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"\n0\.000 s +2026-10-20 14:00:00\.500000  trigger\n").unwrap())
        .stdout(
            predicate::str::is_match(r"\n\+0\.010 s +\+0\.010 s +2026-10-20 14:00:00\.510000  D2 START_B +0 → 1\n")
                .unwrap(),
        )
        .stdout(predicate::str::is_match(r"\n\+0\.060 s +\+0\.050 s .* D5 TRIP_B +0 → 1\n").unwrap())
        .stdout(predicate::str::contains("8 changes in 10 binary channels"));

    let out = cmd()
        .args([
            "events",
            "tests/data/fault_1999.cfg",
            "--channels",
            "TRIP,d3",
            "--from",
            "0",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ev = v["events"].as_array().unwrap();
    let short: Vec<_> = ev
        .iter()
        .map(|e| {
            (
                e["id"].as_str().unwrap(),
                e["time"].as_f64().unwrap(),
                e["to"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        short,
        [("TRIP", 0.02, true), ("CB_OPEN", 0.06, true), ("TRIP", 0.07, false)]
    );
    assert_eq!(ev[0]["absolute"], "2024-03-15 08:30:00.080000");

    cmd()
        .args(["events", "tests/data/fault_1999.cfg", "--to", "-30"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No changes in 4 binary channels."));
    cmd()
        .args(["events", "tests/data/fault_1999.cfg", "--channels", "IA"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("IA is an analog channel"));
}

/// Asks for completions the way the bash script does: the words of the
/// command line and the index of the word under the cursor.
fn complete(words: &[&str]) -> Vec<String> {
    // Word 0 is the program, so the last word has index `words.len()`.
    let out = cmd()
        .env("COMPLETE", "bash")
        .env("_CLAP_COMPLETE_INDEX", words.len().to_string())
        .arg("--")
        .arg("sprcmtrd")
        .args(words)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout)
        .unwrap()
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn dynamic_completion() {
    let sep = std::path::MAIN_SEPARATOR;
    // Recordings only (no .dat, .hdr), in the typed directory and below.
    for command in ["info", "view", "stats", "events", "export", "cut", "convert"] {
        let got = complete(&[command, "tests/data/fa"]);
        assert_eq!(
            got,
            [
                "tests/data/fault_1999.cfg",
                "tests/data/fault_2013.cfg",
                "tests/data/fault_2013_uppercase.cfg"
            ],
            "{command}"
        );
    }
    let got = complete(&["view", "tests/d"]);
    assert_eq!(got[0], format!("tests/data{sep}"));
    assert!(got.contains(&format!("tests/data{sep}combined_2013.cff")), "{got:?}");
    assert!(
        !got.iter().any(|c| c.ends_with(".dat") || c.ends_with(".hdr")),
        "{got:?}"
    );

    // Upper case extensions are recordings too.
    assert_eq!(
        complete(&["view", "tests/data/test_"]),
        ["tests/data/test_ascii.cff", "tests/data/test_ascii_uppercase.CFF"]
    );

    // Subcommands and option values come from the clap definitions.
    assert!(complete(&["ev"]).contains(&"events".to_owned()));
    assert_eq!(complete(&["convert", "x.cfg", "--rev", ""]), ["1991", "1999", "2013"]);

    // Without words: the registration script for the shell.
    cmd()
        .env("COMPLETE", "bash")
        .assert()
        .success()
        .stdout(predicate::str::contains("_clap_complete_sprcmtrd"));
}

#[test]
fn check_files_and_directories() {
    cmd()
        .args(["check", "tests/data/fault_2013.cfg"])
        .assert()
        .success()
        .stdout("status  file                       channel  finding\n✓ OK    tests/data/fault_2013.cfg\n");

    // A warning passes, unless --strict.
    cmd()
        .args(["check", "tests/data/old_1991.cfg"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "! WARNING  tests/data/old_1991.cfg  A1 IA    1 missing value (first at sample 51)\n",
        ));
    cmd()
        .args(["check", "tests/data/old_1991.cfg", "--strict"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains(
            "check failed: 1 of 1 with warnings (--strict)",
        ));

    // A directory with a broken recording fails.
    cmd()
        .args(["check", "tests/data"])
        .assert()
        .code(4)
        .stdout(
            predicate::str::is_match(r"\n✗ ERROR +broken\.cfg +cfg line 2: channel count 5 does not match 2A \+ 1D\n")
                .unwrap(),
        )
        .stdout(predicate::str::is_match(r"\n✓ OK +fault_1999\.cfg\n").unwrap())
        // Piped output has no color codes.
        .stdout(predicate::str::contains("\x1b").not())
        .stdout(predicate::str::is_match(r"\d+ recordings: \d+ ok, 1 with warnings, 1 with errors").unwrap());

    // Data problems: a cut-short data file, and none at all.
    let dir = tempfile::tempdir().unwrap();
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    std::fs::copy(format!("{data}/fault_2013.cfg"), dir.path().join("short.cfg")).unwrap();
    let dat = std::fs::read(format!("{data}/fault_2013.dat")).unwrap();
    std::fs::write(dir.path().join("short.dat"), &dat[..1000]).unwrap();
    std::fs::copy(format!("{data}/fault_1999.cfg"), dir.path().join("nodata.cfg")).unwrap();
    let out = cmd().arg("check").arg(dir.path()).arg("--json").output().unwrap();
    assert_eq!(out.status.code(), Some(4));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let status: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["file"].as_str().unwrap(), r["status"].as_str().unwrap()))
        .collect();
    assert_eq!(status, [("nodata.cfg", "error"), ("short.cfg", "error")]);
    assert!(
        v[1]["issues"][0]["message"]
            .as_str()
            .unwrap()
            .contains("not a multiple of the record size")
    );

    // A missing file is a file error (3), reported once.
    cmd()
        .args(["check", "tests/data/nope.cfg"])
        .assert()
        .code(3)
        .stderr("error: tests/data/nope.cfg: no such file\n");
}
