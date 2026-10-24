use neuro_domain::{Experiment, ResponseScenario, Scenario, atomic_write};
use neuro_engine::{evaluate_experiment, response_benchmark};
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    if let Err(error) = run() {
        eprintln!("Ошибка: {error}");
        std::process::exit(1);
    }
}
fn next_path(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("После {flag} нужен путь"))
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        help();
        return Ok(());
    };
    if command == "--help" || command == "-h" {
        help();
        return Ok(());
    }
    if command == "benchmark-response" {
        if args.next().is_some() {
            return Err("benchmark-response не принимает аргументов".into());
        }
        let report = response_benchmark()?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        if !report.passed {
            return Err("Ошибка превышает допуск эталона".into());
        }
        return Ok(());
    }
    if command == "atlas" {
        let csv = match args.next().as_deref() {
            None => None,
            Some("--csv") => Some(next_path(&mut args, "--csv")?),
            _ => return Err("Используйте atlas [--csv regions.csv]".into()),
        };
        if args.next().is_some() {
            return Err("Лишний аргумент atlas".into());
        }
        let atlas = neuro_domain::BrainAtlas::bundled()?;
        let _geometry = neuro_domain::BrainGeometry::bundled(&atlas)?;
        if let Some(path) = csv {
            atomic_write(&path, atlas.csv().as_bytes())?;
        }
        println!("{}", serde_json::to_string_pretty(&atlas)?);
        return Ok(());
    }
    let path = next_path(&mut args, &command)?;
    match command.as_str() {
        "new" | "new-response" | "new-network" | "new-caffeine" => {
            if args.next().is_some() {
                return Err("Лишний аргумент команды new".into());
            }
            if path.exists() {
                return Err("Файл уже существует; укажите новый путь".into());
            }
            if command == "new-caffeine" {
                let atlas = neuro_domain::BrainAtlas::bundled()?;
                neuro_domain::CaffeineScenario::reference(&atlas)?.save(&path, &atlas)?;
            } else if command == "new-network" {
                let atlas = neuro_domain::BrainAtlas::bundled()?;
                neuro_domain::NetworkScenario::bundled(&atlas)?.save(&path, &atlas)?;
            } else if command == "new-response" {
                ResponseScenario::default().save(&path)?;
            } else {
                Scenario::default().save(&path)?;
            }
            println!("Создан illustrative-сценарий: {}", path.display());
        }
        "run" | "simulate" | "caffeine" => {
            let mut csv = None;
            let mut json = None;
            let mut network_time = None;
            let mut regional_csv = None;
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--csv" if csv.is_none() => csv = Some(next_path(&mut args, &flag)?),
                    "--json" if json.is_none() => json = Some(next_path(&mut args, &flag)?),
                    "--regional-csv" if command == "caffeine" && regional_csv.is_none() => {
                        regional_csv = Some(next_path(&mut args, &flag)?)
                    }
                    "--network-at-min" if command == "caffeine" && network_time.is_none() => {
                        network_time = Some(
                            args.next()
                                .ok_or("После --network-at-min нужно время")?
                                .parse::<f64>()?,
                        );
                    }
                    _ => return Err(format!("Неизвестный или повторный аргумент: {flag}").into()),
                }
            }
            for output in [&csv, &json, &regional_csv].into_iter().flatten() {
                if same_path(&path, output) {
                    return Err("Экспорт не должен перезаписывать входной сценарий".into());
                }
            }
            let outputs: Vec<_> = [&csv, &json, &regional_csv].into_iter().flatten().collect();
            for (i, a) in outputs.iter().enumerate() {
                for b in &outputs[i + 1..] {
                    if same_path(a, b) {
                        return Err("Все файлы экспорта должны иметь разные пути".into());
                    }
                }
            }
            let text = if command == "caffeine" {
                let atlas = neuro_domain::BrainAtlas::bundled()?;
                let scenario = neuro_domain::CaffeineScenario::load(&path, &atlas)?;
                let trace = neuro_engine::simulate_caffeine(scenario, &atlas)?;
                let comparison = network_time
                    .map(|time| neuro_engine::compare_caffeine_network(&trace, time, &atlas))
                    .transpose()?;
                if let Some(path) = regional_csv {
                    let regional = trace
                        .perfusion
                        .as_ref()
                        .ok_or("В сценарии не включена региональная перфузия")?;
                    atomic_write(&path, regional.regional_csv(&trace.scenario).as_bytes())?;
                }
                if let Some(csv) = csv {
                    trace.export_csv(csv)?;
                }
                serde_json::to_string_pretty(
                    &serde_json::json!({"trace":trace,"network_comparison":comparison}),
                )?
            } else if command == "simulate" {
                let atlas = neuro_domain::BrainAtlas::bundled()?;
                let scenario = neuro_domain::NetworkScenario::load(&path, &atlas)?;
                let trace = neuro_engine::simulate_network(scenario, &atlas)?;
                if let Some(csv) = csv {
                    trace.export_csv(csv)?;
                }
                serde_json::to_string_pretty(&trace)?
            } else {
                let evaluation = evaluate_experiment(&Experiment::load(&path)?)?;
                if let Some(csv) = csv {
                    evaluation.export_csv(csv)?;
                }
                serde_json::to_string_pretty(&evaluation)?
            };
            if let Some(json) = json {
                atomic_write(&json, format!("{text}\n").as_bytes())?;
            }
            println!("{text}");
        }
        _ => return Err(format!("Неизвестная команда: {command}. Используйте --help").into()),
    }
    Ok(())
}
fn same_path(a: &Path, b: &Path) -> bool {
    fn normalized(path: &Path) -> PathBuf {
        if let Ok(path) = path.canonicalize() {
            return path;
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf())
            .join(path.file_name().unwrap_or_default())
    }
    normalized(a) == normalized(b)
}
fn help() {
    println!(
        "NeuroLab — анатомия мозга и численные модули\n\n  neuro-cli atlas [--csv regions.csv]\n  neuro-cli new scenario.json\n  neuro-cli new-response response.json\n  neuro-cli new-network network.json\n  neuro-cli new-caffeine caffeine.json\n  neuro-cli caffeine caffeine-or-project.json [--csv pk.csv] [--json report.json] [--regional-csv regions.csv] [--network-at-min 60]\n  neuro-cli simulate network-or-project.json [--csv trace.csv] [--json trace.json]\n  neuro-cli run scenario.json [--csv results.csv] [--json results.json]\n  neuro-cli benchmark-response\n\nrun выбирает модель по model_id. simulate вычисляет временную трассу сети E/I. atlas проверяет геометрию. JSON — stdout, ошибки — stderr и код 1."
    );
}
