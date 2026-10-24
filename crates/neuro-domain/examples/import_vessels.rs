//! Import official BodyParts3D 4.0 arteries in the brain source coordinate frame.
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
    // Named source concepts, expanded by the official IS-A composition table.
    let concepts = [
        (
            "left internal carotid artery",
            "Внутренняя сонная · слева",
            "left",
            "ica_left",
        ),
        (
            "right internal carotid artery",
            "Внутренняя сонная · справа",
            "right",
            "ica_right",
        ),
        (
            "left vertebral artery",
            "Позвоночная · слева",
            "left",
            "vertebrobasilar",
        ),
        (
            "right vertebral artery",
            "Позвоночная · справа",
            "right",
            "vertebrobasilar",
        ),
        (
            "basilar artery",
            "Базилярная артерия",
            "bilateral",
            "vertebrobasilar",
        ),
        (
            "left anterior cerebral artery",
            "Передняя мозговая · слева",
            "left",
            "aca_left",
        ),
        (
            "right anterior cerebral artery",
            "Передняя мозговая · справа",
            "right",
            "aca_right",
        ),
        (
            "sphenoid part of left middle cerebral artery",
            "Средняя мозговая M1 · слева",
            "left",
            "mca_left",
        ),
        (
            "sphenoid part of right middle cerebral artery",
            "Средняя мозговая M1 · справа",
            "right",
            "mca_right",
        ),
        (
            "insular part of left middle cerebral artery",
            "Средняя мозговая M2 · слева",
            "left",
            "mca_left",
        ),
        (
            "insular part of right middle cerebral artery",
            "Средняя мозговая M2 · справа",
            "right",
            "mca_right",
        ),
        (
            "precommunicating part of left posterior cerebral artery",
            "Задняя мозговая P1 · слева",
            "left",
            "pca_left",
        ),
        (
            "precommunicating part of right posterior cerebral artery",
            "Задняя мозговая P1 · справа",
            "right",
            "pca_right",
        ),
        (
            "postcommunicating part of left posterior cerebral artery",
            "Задняя мозговая P2 · слева",
            "left",
            "pca_left",
        ),
        (
            "postcommunicating part of right posterior cerebral artery",
            "Задняя мозговая P2 · справа",
            "right",
            "pca_right",
        ),
        (
            "anterior communicating artery",
            "Передняя соединительная",
            "bilateral",
            "communicating",
        ),
        (
            "left posterior communicating artery",
            "Задняя соединительная · слева",
            "left",
            "communicating",
        ),
        (
            "right posterior communicating artery",
            "Задняя соединительная · справа",
            "right",
            "communicating",
        ),
        (
            "left superior cerebellar artery",
            "Верхняя мозжечковая · слева",
            "left",
            "vertebrobasilar",
        ),
        (
            "right superior cerebellar artery",
            "Верхняя мозжечковая · справа",
            "right",
            "vertebrobasilar",
        ),
        (
            "left posterior inferior cerebellar artery",
            "Задняя нижняя мозжечковая · слева",
            "left",
            "vertebrobasilar",
        ),
        (
            "right posterior inferior cerebellar artery",
            "Задняя нижняя мозжечковая · справа",
            "right",
            "vertebrobasilar",
        ),
        (
            "anterior inferior cerebellar artery",
            "Передние нижние мозжечковые",
            "bilateral",
            "vertebrobasilar",
        ),
    ];
    for (english, russian, side, territory) in concepts {
        let fields: Vec<_> = names
            .lines()
            .filter_map(|line| {
                let f: Vec<_> = line.split('\t').collect();
                (f.len() == 3 && f[2] == english).then_some(f)
            })
            .next()
            .ok_or_else(|| format!("Unknown artery: {english}"))?;
        let id = fields[0];
        let mesh_ids: Vec<_> = elements
            .lines()
            .filter_map(|line| {
                let f: Vec<_> = line.split('\t').collect();
                (f.len() == 3 && f[0] == id).then(|| f[2].to_string())
            })
            .collect();
        if mesh_ids.is_empty() {
            return Err(format!("No geometry: {english}").into());
        }
        regions.push(json!({"id":id,"name":russian,"name_en":english,"hemisphere":side,"mesh_ids":mesh_ids,"supply":territory}));
    }

    // Include uniquely named left/right cortical and cerebellar branches from
    // official composition sets; never guess a branch identity from a file name.
    let mut sets = BTreeMap::<String, (String, std::collections::BTreeSet<String>)>::new();
    for line in elements.lines() {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() == 3 {
            let e = sets
                .entry(f[0].into())
                .or_insert_with(|| (f[1].into(), Default::default()));
            e.1.insert(f[2].into());
        }
    }
    let mut assigned: std::collections::BTreeSet<String> = regions
        .iter()
        .flat_map(|r| r["mesh_ids"].as_array().unwrap())
        .map(|id| id.as_str().unwrap().to_string())
        .collect();
    for (parent, family) in [
        ("FMA50031", "aca"),
        ("FMA50081", "mca"),
        ("FMA50586", "pca"),
        ("FMA50576", "vertebrobasilar"),
    ] {
        let wanted = &sets.get(parent).ok_or("Missing official branch set")?.1;
        let mut candidates: Vec<_> = sets
            .iter()
            .filter(|(_, (name, ids))| {
                (name.contains("left") || name.contains("right"))
                    && !ids.is_empty()
                    && ids.is_subset(wanted)
            })
            .collect();
        candidates.sort_by_key(|(id, (_, ids))| (ids.len(), *id));
        for (id, (name, ids)) in candidates {
            if !ids.is_disjoint(&assigned) {
                continue;
            }
            let left = name.contains("left");
            let side = if left { "left" } else { "right" };
            let supply = if family == "vertebrobasilar" {
                family.into()
            } else {
                format!("{family}_{side}")
            };
            assigned.extend(ids.iter().cloned());
            regions.push(json!({"id":id,"name":name,"name_en":name,"hemisphere":side,"mesh_ids":ids,"supply":supply}));
        }
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
    let range_path = root.join("vessel-range.bin");
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
    fs::write("assets/brain/arteries.bin", &binary)?;
    let brain: Value = serde_json::from_str(&fs::read_to_string("data/brain-atlas.json")?)?;
    let mut source = brain["source"].clone();
    source["changes"] = json!(
        "Выбраны магистральные артерии и именованные корковые/мозжечковые ветви по официальным таблицам FMA. OBJ триангулирован в f32, без сглаживания и деформации. Геометрия вне границ мозга скрывается только при отображении. Русские названия добавлены NeuroLab. Кровоток, капилляры и венозная сеть в исходных данных не измерены."
    );
    let atlas = json!({"format_version":1,"atlas_id":"bodyparts3d_cerebral_arteries_4.0","atlas_revision":1,"source":source,"vessels":regions,"meshes":mesh_manifest,"total_vertices":total_vertices,"total_triangles":total_triangles});
    fs::write(
        "data/vascular-atlas.json",
        format!("{}\n", serde_json::to_string_pretty(&atlas)?),
    )?;
    println!(
        "Imported {total_vertices} vertices, {total_triangles} triangles, {} bytes",
        binary.len()
    );
    Ok(())
}
