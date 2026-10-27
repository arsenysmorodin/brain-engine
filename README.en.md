# NeuroLab

**A local Rust environment for exploring brain anatomy, population dynamics and pharmacokinetics.** It combines a native Bevy/egui application, a CPU/f64 calculation library and a reproducible CLI. No server or API keys are required.

[Русский README](README.md) · [Documentation index](docs/README.md) · [Architecture](docs/architecture.md) · [Scientific status](docs/scientific-status.md)

![NeuroLab brain anatomy](docs/images/brain.png)

Anatomy: BodyParts3D, © The Database Center for Life Science, CC BY 4.0. Screenshot of the actual application. Geometric detail is not evidence of physiological validity.

## Features and scope

- 58 named brain structures / 63 meshes, FMA identifiers, layers, surface selection, search, camera and scene files.
- 47 grey-matter regions with 94 Wilson–Cowan E/I populations and 46 **demonstration** directed connections with delays.
- Caffeine, methylliberine and theacrine, single/pair/triple oral protocols, scheduled and repeated doses.
- Regional flow, capillary–tissue exchange, venous return and conserved mass in a closed circulation model.
- Five digitized published plasma curves, three study-specific protocols, peak/decline fitting and explicit error diagnostics.
- A separate caffeine/PET model, illustrative receptor binding/Black–Leff experiments, mechanisms and Russian educational tooltips.
- Versioned JSON, systemic/regional CSV, shared GUI/CLI calculations and numerical verification.

This is a research and educational platform, **not a complete physiological brain model**. Connectivity, regional transport and concentration-to-network coupling contain illustrative assumptions. Receptor occupancy is not activation, E/I is not a behavioural outcome, and plasma fitting does not validate brain tissue predictions or personal dosing/safety.

## Run

The verified native platform is **macOS Apple Silicon / Metal**. Install Xcode Command Line Tools and current stable Rust with rustfmt and Clippy.

```sh
xcode-select --install       # only if not already installed
rustup component add rustfmt clippy
git clone https://github.com/arsenysmorodin/brain-engine.git
cd brain-engine
cargo run -p neuro-app --locked
```

Dependencies require network access initially. Geometry, metadata and fonts are committed and embedded. After building, `./target/debug/neuro-app` runs offline. Linux/Windows native verification is not claimed.

`scripts/cargo-local` is only a helper for an **already installed** isolated toolchain under `.local/`; that ignored directory is absent in a fresh clone. Use normal Cargo on a new installation. [Environment and troubleshooting](docs/development.md) (Russian).

The UI is Russian. Use the turquoise “Запустить симуляцию” button to enter a substance, dose in mg and oral administration time in minutes from scenario start. Add rows for a mixture/repeated dosing. “Кривые исследований” opens study curves and “Запустить этот протокол” loads a complete fitted condition.

## Reproduce without the GUI

```sh
cargo build -p neuro-cli --locked
mkdir -p output/demo
./target/debug/neuro-cli --help
./target/debug/neuro-cli simulate scenarios/brain-network.json --csv output/demo/network.csv > output/demo/network.json
./target/debug/neuro-cli new-circulation output/demo/purines.json
./target/debug/neuro-cli circulation output/demo/purines.json --csv output/demo/pk.csv > output/demo/trace.json
```

`new*` commands require a new output filename. `new-circulation` creates revision 4; the committed `scenarios/purine-circulation.json` intentionally retains historical revision 1. [Complete command reference](docs/cli.md).

## Scientific data status

The fitted set uses five graph-digitized curves from Mondal 2022 and Wang 2020: single M100/C150 and the C150+M100+T50 mg mixture. Participant-level numerical records have not been obtained; standalone theacrine time-series calibration is still unsupported. The full Wang text has not been verified, and the source context/units require further clarification.

Training agreement is not independent validation. Excluded-time prediction error for mixture methylliberine is about 32%; some individual mixture residuals are much larger and several parameters are unstable. These limitations and digitization uncertainty remain visible. Regional brain pharmacokinetics and E/I effects are not calibrated by the plasma fits. [Equations, source audit and results](docs/full-pk-calibration.md).

## Development

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo doc --workspace --no-deps --locked
```

Build jobs are limited to two. Native visual verification is separate from compilation. `cargo build` without `--workspace` omits the GUI through `default-members`.

On macOS, `./scripts/macos-app` produces `output/NeuroLab.app` from the **debug** build, includes notices and applies an ad hoc signature. Dependencies must already be cached because the script builds offline. It is not a notarized release installer. [Development, smoke runs and publication](docs/development.md).

Workspace: `neuro-domain` owns documents/validation; `neuro-engine` owns numerical models; `neuro-app` owns GUI/3D; `neuro-cli` calls the same engine. Data, assets, examples and scientific specifications are tracked separately. [Contribution guide](CONTRIBUTING.md) and detailed manuals are currently Russian.

## Licenses

Own code and documentation: [MIT](LICENSE). BodyParts3D geometry and adapted anatomical metadata: CC BY 4.0. Noto fonts: SIL OFL 1.1. Scientific publications and Rust dependencies retain their respective terms; the project MIT license does not replace them. [Third-party notices](THIRD_PARTY_NOTICES.md), [data inventory](docs/data-and-licenses.md).
