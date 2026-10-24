//! Development-only import of the official BodyParts3D 4.0 IS-A OBJ subset.
//! Run from the repository root; see docs/brain-atlas.md for provenance and preparation.
use flate2::read::DeflateDecoder;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, io::Read, path::Path, process::Command};

const ARCHIVE: &str =
    "https://dbarchive.biosciencedbc.jp/data/bodyparts3d/LATEST/isa_BP3D_4.0_obj_99.zip";
const ARCHIVE_SIZE: usize = 142_903_898;
fn u16_at(bytes: &[u8], i: usize) -> usize {
    u16::from_le_bytes(bytes[i..i + 2].try_into().unwrap()) as usize
}
fn u32_at(bytes: &[u8], i: usize) -> usize {
    u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize
}
#[derive(Debug)]
struct Entry {
    name: String,
    offset: usize,
    compressed: usize,
    size: usize,
    crc: u32,
    method: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(".local/brain-source");
    let names = fs::read_to_string(root.join("isa-parts.tsv"))?;
    let elements = fs::read_to_string(root.join("isa-elements.tsv"))?;
    let mut regions = Vec::new();
    let pairs = [
        (
            "superior frontal gyrus",
            "Верхняя лобная извилина",
            "frontal",
            "cortex",
        ),
        (
            "middle frontal gyrus",
            "Средняя лобная извилина",
            "frontal",
            "cortex",
        ),
        (
            "inferior frontal gyrus",
            "Нижняя лобная извилина",
            "frontal",
            "cortex",
        ),
        (
            "precentral gyrus",
            "Предцентральная извилина",
            "frontal",
            "cortex",
        ),
        (
            "postcentral gyrus",
            "Постцентральная извилина",
            "parietal",
            "cortex",
        ),
        (
            "superior parietal lobule",
            "Верхняя теменная долька",
            "parietal",
            "cortex",
        ),
        (
            "supramarginal gyrus",
            "Надкраевая извилина",
            "parietal",
            "cortex",
        ),
        ("angular gyrus", "Угловая извилина", "parietal", "cortex"),
        (
            "middle temporal gyrus",
            "Средняя височная извилина",
            "temporal",
            "cortex",
        ),
        (
            "inferior temporal gyrus",
            "Нижняя височная извилина",
            "temporal",
            "cortex",
        ),
        (
            "fusiform gyrus",
            "Веретенообразная извилина",
            "temporal",
            "cortex",
        ),
        ("occipital lobe", "Затылочная доля", "occipital", "cortex"),
        ("insula", "Островок", "insula", "cortex"),
        ("cingulate gyrus", "Поясная извилина", "medial", "cortex"),
        (
            "parahippocampal gyrus",
            "Парагиппокампальная извилина",
            "medial",
            "cortex",
        ),
        ("hippocampus", "Гиппокамп", "medial", "gray_matter"),
        ("amygdala", "Миндалина", "deep", "gray_matter"),
        ("thalamus", "Таламус", "deep", "gray_matter"),
        ("caudate nucleus", "Хвостатое ядро", "deep", "gray_matter"),
        ("putamen", "Скорлупа", "deep", "gray_matter"),
        ("globus pallidus", "Бледный шар", "deep", "gray_matter"),
        (
            "lateral ventricle",
            "Боковой желудочек",
            "ventricles",
            "cavity",
        ),
    ];
    let mut add = |english: String,
                   russian: String,
                   group: &str,
                   kind: &str,
                   side: &str|
     -> Result<(), Box<dyn std::error::Error>> {
        let fields = names
            .lines()
            .filter_map(|line| {
                let f: Vec<_> = line.split('\t').collect();
                (f.len() == 3 && f[2] == english).then_some(f)
            })
            .next()
            .ok_or_else(|| format!("Unknown source name: {english}"))?;
        let id = fields[0];
        let mesh_ids: Vec<_> = elements
            .lines()
            .filter_map(|line| {
                let f: Vec<_> = line.split('\t').collect();
                (f.len() == 3 && f[0] == id).then(|| f[2].to_string())
            })
            .collect();
        if mesh_ids.is_empty() {
            return Err(format!("No geometry for {english}").into());
        }
        regions.push(json!({"id":id,"name":russian,"name_en":english,"group":group,"kind":kind,"hemisphere":side,"mesh_ids":mesh_ids,"source_id":"bodyparts3d-4.0"}));
        Ok(())
    };
    for (english, russian, group, kind) in pairs {
        for (side, suffix) in [("left", "слева"), ("right", "справа")] {
            add(
                format!("{side} {english}"),
                format!("{russian} · {suffix}"),
                group,
                kind,
                side,
            )?;
        }
    }
    for (side, suffix) in [("left", "слева"), ("right", "справа")] {
        for (part, ru) in [
            ("anterior", "передняя часть"),
            ("posterior", "задняя часть"),
        ] {
            add(
                format!("{part} part of {side} superior temporal gyrus"),
                format!("Верхняя височная извилина · {suffix}, {ru}"),
                "temporal",
                "cortex",
                side,
            )?;
        }
        add(
            format!("white matter of {side} cerebral hemisphere"),
            format!(
                "Белое вещество · {} полушарие",
                if side == "left" {
                    "левое"
                } else {
                    "правое"
                }
            ),
            "white_matter",
            "white_matter",
            side,
        )?;
    }
    for (english, russian, group, kind) in [
        ("hypothalamus", "Гипоталамус", "deep", "gray_matter"),
        ("cerebellum", "Мозжечок", "cerebellum", "cerebellum"),
        ("midbrain", "Средний мозг", "brainstem", "brainstem"),
        ("pons", "Мост", "brainstem", "brainstem"),
        (
            "medulla oblongata",
            "Продолговатый мозг",
            "brainstem",
            "brainstem",
        ),
        (
            "corpus callosum",
            "Мозолистое тело",
            "white_matter",
            "white_matter",
        ),
        (
            "third ventricle",
            "Третий желудочек",
            "ventricles",
            "cavity",
        ),
        (
            "fourth ventricle",
            "Четвёртый желудочек",
            "ventricles",
            "cavity",
        ),
    ] {
        add(english.into(), russian.into(), group, kind, "bilateral")?;
    }

    let tail = fs::read(root.join("isa-tail.bin"))?;
    let tail_offset = ARCHIVE_SIZE - tail.len();
    let eocd = tail
        .windows(4)
        .rposition(|w| w == b"PK\x05\x06")
        .ok_or("No zip index")?;
    let mut position = u32_at(&tail, eocd + 16)
        .checked_sub(tail_offset)
        .ok_or("Tail too short for central directory")?;
    let count = u16_at(&tail, eocd + 10);
    let wanted: std::collections::BTreeSet<String> = regions
        .iter()
        .flat_map(|r| r["mesh_ids"].as_array().unwrap())
        .map(|id| id.as_str().unwrap().to_string())
        .collect();
    let mut entries = BTreeMap::new();
    for _ in 0..count {
        assert_eq!(&tail[position..position + 4], b"PK\x01\x02");
        let name_length = u16_at(&tail, position + 28);
        let name = String::from_utf8(tail[position + 46..position + 46 + name_length].to_vec())?;
        let stem = Path::new(&name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if wanted.contains(&stem) {
            entries.insert(
                stem,
                Entry {
                    name,
                    offset: u32_at(&tail, position + 42),
                    compressed: u32_at(&tail, position + 20),
                    size: u32_at(&tail, position + 24),
                    crc: u32_at(&tail, position + 16) as u32,
                    method: u16_at(&tail, position + 10),
                },
            );
        }
        position += 46 + name_length + u16_at(&tail, position + 30) + u16_at(&tail, position + 32);
    }
    for id in &wanted {
        if !entries.contains_key(id) {
            return Err(format!("Missing source mesh: {id}").into());
        }
    }
    let start = entries.values().map(|e| e.offset).min().unwrap();
    let end = entries
        .values()
        .map(|e| e.offset + 30 + e.name.len() + e.compressed + 65536)
        .max()
        .unwrap()
        .min(ARCHIVE_SIZE);
    let range_path = root.join("brain-range.bin");
    println!(
        "{} regions, {} meshes; official archive byte range {start}..{end} ({} bytes)",
        regions.len(),
        entries.len(),
        end - start
    );
    if !range_path.exists() || fs::metadata(&range_path)?.len() as usize != end - start {
        let status = Command::new("curl")
            .args([
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--max-time",
                "180",
                "--range",
                &format!("{start}-{}", end - 1),
                ARCHIVE,
                "-o",
            ])
            .arg(&range_path)
            .status()?;
        if !status.success() {
            return Err("Range download failed".into());
        }
    }
    let range = fs::read(&range_path)?;
    if range.len() != end - start {
        return Err("Server did not honor byte range".into());
    }
    let mut binary = b"NBRN0001".to_vec();
    binary.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    let mut mesh_manifest = Vec::new();
    let mut total_vertices = 0;
    let mut total_triangles = 0;
    fs::create_dir_all("assets/brain")?;
    for (id, e) in &entries {
        let offset = e.offset - start;
        assert_eq!(&range[offset..offset + 4], b"PK\x03\x04");
        let data_offset = offset + 30 + u16_at(&range, offset + 26) + u16_at(&range, offset + 28);
        let data = &range[data_offset..data_offset + e.compressed];
        let mut text = Vec::new();
        match e.method {
            0 => text.extend_from_slice(data),
            8 => {
                DeflateDecoder::new(data).read_to_end(&mut text)?;
            }
            _ => return Err("Unsupported compression".into()),
        }
        assert_eq!(text.len(), e.size);
        assert_eq!(crc32fast::hash(&text), e.crc);
        let text = String::from_utf8(text)?;
        let mut vertices = Vec::<[f32; 3]>::new();
        let mut indices = Vec::<u32>::new();
        for line in text.lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.first() == Some(&"v") {
                vertices.push([fields[1].parse()?, fields[2].parse()?, fields[3].parse()?]);
            } else if fields.first() == Some(&"f") {
                let face: Vec<u32> = fields[1..]
                    .iter()
                    .map(|v| v.split('/').next().unwrap().parse::<u32>().map(|i| i - 1))
                    .collect::<Result<_, _>>()?;
                for i in 1..face.len() - 1 {
                    indices.extend_from_slice(&[face[0], face[i], face[i + 1]]);
                }
            }
        }
        assert!(!vertices.is_empty() && !indices.is_empty());
        assert!(vertices.iter().flatten().all(|v| v.is_finite()));
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
        binary.extend_from_slice(&(id.len() as u32).to_le_bytes());
        binary.extend_from_slice(id.as_bytes());
        binary.extend_from_slice(&(vertices.len() as u32).to_le_bytes());
        binary.extend_from_slice(&(indices.len() as u32).to_le_bytes());
        for v in &vertices {
            for x in v {
                binary.extend_from_slice(&x.to_le_bytes());
            }
        }
        for i in &indices {
            binary.extend_from_slice(&i.to_le_bytes());
        }
        if ["FJ1833", "FJ1834", "FJ1791", "FJ1792"].contains(&id.as_str()) {
            let mean: [f32; 3] = std::array::from_fn(|axis| {
                vertices.iter().map(|v| v[axis]).sum::<f32>() / vertices.len() as f32
            });
            println!("Orientation anchor {id}: mean source coordinate {mean:?}");
        }
        total_vertices += vertices.len();
        total_triangles += indices.len() / 3;
        mesh_manifest.push(json!({"id":id,"archive_member":e.name,"source_crc32":format!("{:08x}",e.crc),"vertices":vertices.len(),"triangles":indices.len()/3}));
    }
    fs::write("assets/brain/bodyparts3d.bin", &binary)?;
    let atlas = json!({
        "format_version":1,"atlas_id":"bodyparts3d_brain_4.0","atlas_revision":1,
        "title":"Мозг человека · BodyParts3D","species":"Homo sapiens · анатомическая модель взрослого мужчины",
        "coordinate_space":"BodyParts3D source coordinates; no MNI registration",
        "limitations":"Анатомическая визуализация, низкополигональная геометрия. Не индивидуальный мозг и не функциональная парцелляция. Набор структур неполон. Связи, параметры и активность областей не заданы; неизвестное не равно нулю.",
        "source":{"id":"bodyparts3d-4.0","title":"BodyParts3D 4.0 · Database Center for Life Science","url":"https://dbarchive.biosciencedbc.jp/en/bodyparts3d/download.html","archive_url":ARCHIVE,"license_url":"https://dbarchive.biosciencedbc.jp/en/bodyparts3d/lic.html","license":"CC BY 4.0","attribution":"BodyParts3D, © The Database Center for Life Science licensed under CC Attribution 4.0 International","publication_doi":"10.1093/nar/gkn613","checked_on":"2026-10-24","verification":"official_dataset_and_license_verified","changes":"Выбраны структуры мозга; русские названия и группы интерфейса добавлены NeuroLab. OBJ преобразован в индексированные треугольники f32. Исходное положение структур сохранено; нормали и освещение вычисляются при отображении."},
        "regions":regions,"meshes":mesh_manifest,"total_vertices":total_vertices,"total_triangles":total_triangles
    });
    fs::write(
        "data/brain-atlas.json",
        format!("{}\n", serde_json::to_string_pretty(&atlas)?),
    )?;
    let names_subset: Vec<Value> = atlas["regions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| json!({"fma_id":r["id"],"name_en":r["name_en"],"mesh_ids":r["mesh_ids"]}))
        .collect();
    fs::write(
        "assets/brain/source-mapping.json",
        format!("{}\n", serde_json::to_string_pretty(&names_subset)?),
    )?;
    println!(
        "Imported {total_vertices} vertices, {total_triangles} triangles, {} bytes",
        binary.len()
    );
    Ok(())
}
