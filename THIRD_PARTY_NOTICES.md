# Third-party notices

The [MIT license](LICENSE) covers NeuroLab's own code and documentation. It does not replace the licenses of embedded assets, adapted anatomical metadata, dependencies or referenced publications.

## BodyParts3D

**BodyParts3D, © The Database Center for Life Science licensed under CC Attribution 4.0 International.**

- Dataset: BodyParts3D 4.0, official IS-A polygon meshes, 99% polygon reduction.
- Source: <https://dbarchive.biosciencedbc.jp/en/bodyparts3d/download.html>
- Source license notice: <https://dbarchive.biosciencedbc.jp/en/bodyparts3d/lic.html>
- License: [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/).
- Publication: Mitsuhashi et al., *BodyParts3D: 3D structure database for anatomical concepts*, DOI [10.1093/nar/gkn613](https://doi.org/10.1093/nar/gkn613).

NeuroLab selects brain and cerebral artery structures, triangulates/converts OBJ coordinates to indexed f32 binary meshes, computes display normals, normalizes display coordinates, and adds Russian names and editorial display groups. Source-relative positions are preserved. These assets contain no physiological activity or connectivity. Adapted geometry and anatomical metadata remain under CC BY 4.0. Screenshots containing these assets retain this attribution.

See [the embedded notice](assets/brain/LICENSE-BodyParts3D.txt), [atlas/import documentation](docs/brain-atlas.md), `data/brain-atlas.json` and `data/vascular-atlas.json` for mapping and provenance. Schematic capillary/venous paths are NeuroLab code, not additional BodyParts3D measurements.

## Noto fonts

| Asset | License notice |
|---|---|
| Noto Sans | [SIL OFL 1.1](assets/fonts/OFL.txt) |
| Noto Sans Math | [SIL OFL 1.1](assets/fonts/NotoSansMath-OFL.txt) |
| Noto Sans Symbols 2 | [SIL OFL 1.1](assets/fonts/NotoSansSymbols2-OFL.txt) |

Copyright notices are preserved in those files. The macOS packager copies these notices and the BodyParts3D notice into `Contents/Resources/licenses`.

## Scientific sources and Rust dependencies

Scientific publications retain their own rights. NeuroLab embeds source metadata, limited numerical facts, graph digitizations with provenance and derived fits; it does not distribute complete source PDFs or original publication figures. Participant-level raw data are not bundled or claimed to have been obtained. Access to a paper is not a blanket redistribution license.

Rust dependencies retain the terms of their upstream packages. Cargo.lock records the resolved versions; a complete commercial-distribution license audit is not asserted here. See [data and license inventory](docs/data-and-licenses.md) and [scientific provenance](docs/provenance.md).
