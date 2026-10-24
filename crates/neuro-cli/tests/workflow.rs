use neuro_domain::{Parameters, Scenario};
use neuro_engine::evaluate;
use std::{fs, path::PathBuf, process::Command};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "neurolab-test-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_neuro-cli"))
}

#[test]
fn saved_scenario_cli_and_shared_app_path_agree() {
    let temp = Temp::new();
    let input = temp.0.join("scenario.json");
    let output = temp.0.join("results.csv");
    let json = temp.0.join("results.json");
    let scenario = Scenario {
        parameters: Parameters { a: 3.0, b: 6.0 },
        ..Scenario::default()
    };
    scenario.save(&input).unwrap();
    assert_eq!(Scenario::load(&input).unwrap(), scenario);
    let shared = evaluate(&Scenario::load(&input).unwrap()).unwrap();
    let result = cli()
        .arg("run")
        .arg(&input)
        .arg("--csv")
        .arg(&output)
        .arg("--json")
        .arg(&json)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read_to_string(&output).unwrap(), shared.csv());
    let result: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        result["current"]["p_a"].as_f64().unwrap(),
        shared.current.p_a
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&fs::read_to_string(json).unwrap()).unwrap(),
        result
    );
}

#[test]
fn corrupted_unsupported_and_unwritable_files_are_clear_errors() {
    let temp = Temp::new();
    let input = temp.0.join("bad.json");
    for (text, message) in [
        ("{bad", "Повреждённый JSON"),
        ("{\"format_version\":99}", "версия"),
    ] {
        fs::write(&input, text).unwrap();
        let result = cli().arg("run").arg(&input).output().unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(message));
    }
    let valid = temp.0.join("valid.json");
    Scenario::default().save(&valid).unwrap();
    let missing_parent = temp.0.join("missing/result.csv");
    let result = cli()
        .arg("run")
        .arg(&valid)
        .arg("--csv")
        .arg(missing_parent)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Ошибка файла"));
    let before = fs::read(&valid).unwrap();
    let result = cli()
        .arg("run")
        .arg(&valid)
        .arg("--csv")
        .arg(&valid)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(fs::read(&valid).unwrap(), before);
}

#[test]
fn creation_is_usable_and_refuses_accidental_overwrite() {
    let temp = Temp::new();
    let path = temp.0.join("new.json");
    assert!(cli().arg("new").arg(&path).status().unwrap().success());
    assert!(Scenario::load(&path).is_ok());
    assert!(
        !cli()
            .arg("new")
            .arg(&path)
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn response_cli_roundtrip_export_and_benchmark() {
    use neuro_domain::{Experiment, ResponseScenario};
    use neuro_engine::evaluate_response;
    let temp = Temp::new();
    let input = temp.0.join("response.json");
    let csv = temp.0.join("response.csv");
    assert!(
        cli()
            .arg("new-response")
            .arg(&input)
            .output()
            .unwrap()
            .status
            .success()
    );
    let Experiment::Response(scenario) = Experiment::load(&input).unwrap() else {
        panic!("wrong model");
    };
    assert_eq!(scenario, ResponseScenario::default());
    let shared = evaluate_response(&scenario).unwrap();
    let output = cli()
        .arg("run")
        .arg(&input)
        .arg("--csv")
        .arg(&csv)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(&csv).unwrap(), shared.csv());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["current"]["fractional_response"].as_f64().unwrap(),
        shared.current.fractional_response
    );
    let before = fs::read(&input).unwrap();
    assert!(
        !cli()
            .arg("run")
            .arg(&input)
            .arg("--csv")
            .arg(&input)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read(&input).unwrap(), before);
    let report = cli().arg("benchmark-response").output().unwrap();
    assert!(report.status.success());
    let value: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(value["passed"], true);
    assert_eq!(value["independent_experimental_validation"], false);
}

#[test]
fn anatomical_inventory_validates_meshes_and_exports_unknown_activity() {
    let temp = Temp::new();
    let csv = temp.0.join("regions.csv");
    let output = cli().arg("atlas").arg("--csv").arg(&csv).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let atlas: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(atlas["regions"].as_array().unwrap().len(), 58);
    assert_eq!(atlas["meshes"].as_array().unwrap().len(), 63);
    assert_eq!(
        std::fs::read_to_string(csv).unwrap(),
        neuro_domain::BrainAtlas::bundled().unwrap().csv()
    );
}

#[test]
fn neural_network_cli_reproduces_shared_trace_and_guards_input() {
    let temp = Temp::new();
    let input = temp.0.join("network.json");
    let csv = temp.0.join("trace.csv");
    assert!(
        cli()
            .arg("new-network")
            .arg(&input)
            .output()
            .unwrap()
            .status
            .success()
    );
    let atlas = neuro_domain::BrainAtlas::bundled().unwrap();
    let network = neuro_domain::NetworkScenario::load(&input, &atlas).unwrap();
    let expected = neuro_engine::simulate_network(network, &atlas).unwrap();
    let result = cli()
        .arg("simulate")
        .arg(&input)
        .arg("--csv")
        .arg(&csv)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read_to_string(csv).unwrap(), expected.csv());
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["elapsed_ms"], 1000.0);
    assert_eq!(value["scenario"]["data_kind"], "illustrative");
    let before = fs::read(&input).unwrap();
    assert!(
        !cli()
            .arg("simulate")
            .arg(&input)
            .arg("--csv")
            .arg(&input)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read(&input).unwrap(), before);
}

#[test]
fn caffeine_cli_roundtrip_trace_and_explicit_neural_hypothesis() {
    let temp = Temp::new();
    let path = temp.0.join("caffeine.json");
    assert!(
        cli()
            .arg("new-caffeine")
            .arg(&path)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !cli()
            .arg("new-caffeine")
            .arg(&path)
            .output()
            .unwrap()
            .status
            .success()
    );
    let atlas = neuro_domain::BrainAtlas::bundled().unwrap();
    let mut scenario = neuro_domain::CaffeineScenario::load(&path, &atlas).unwrap();
    let expected = neuro_engine::simulate_caffeine(scenario.clone(), &atlas).unwrap();
    let csv = temp.0.join("pk.csv");
    let result = cli()
        .arg("caffeine")
        .arg(&path)
        .arg("--csv")
        .arg(&csv)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read_to_string(csv).unwrap(), expected.csv());
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        report["trace"]["scenario"]["provenance_id"],
        neuro_domain::CAFFEINE_PROVENANCE_ID
    );
    assert!(report["network_comparison"].is_null());
    assert!(
        !cli()
            .arg("caffeine")
            .arg(&path)
            .arg("--network-at-min")
            .arg("60")
            .output()
            .unwrap()
            .status
            .success()
    );
    scenario.neural_hypothesis.enabled = true;
    let project = neuro_domain::CaffeineProject::new(neuro_domain::BrainScene::default(), scenario);
    project.save(&path, &atlas).unwrap();
    let bytes = fs::read(&path).unwrap();
    assert!(
        !cli()
            .arg("caffeine")
            .arg(&path)
            .arg("--csv")
            .arg(&path)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let result = cli()
        .arg("caffeine")
        .arg(&path)
        .arg("--network-at-min")
        .arg("60")
        .output()
        .unwrap();
    assert!(result.status.success());
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["network_comparison"]["pk_time_min"], 60.0);
    assert!(
        report["network_comparison"]["hypothesis"]
            .as_str()
            .unwrap()
            .contains("Непроверенная")
    );
}
