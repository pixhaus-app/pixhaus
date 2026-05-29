//! Portable export/import bundle for composition records (`.pixstyle`).
//!
//! Same `MessagePack` + zstd stack as the `.pixhaus` project file. The header is
//! byte-identical to the Tauri app's `io/src/pixstyle.rs` so packs round-trip
//! across both trees: magic `b"PXST"`, a little-endian `u16` format version,
//! then the zstd-compressed `MessagePack` body.
//!
//! Two halves live here. The format (`StylePack`, `write_pack`/`read_pack`,
//! `read_library_from_project_ai`) is the on-disk contract. The merge logic
//! (`ConflictPolicy`, `merge_pack` and the per-kind merges) is pure and decides
//! how an incoming pack folds into a project's `ProjectAi` tier. Both are
//! consumed by the shell's pack export/import and copy-from-project actions.

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::ProjectAi;
use super::composition::{PromptId, PromptTemplate, Structure, StructureId, Style, StyleId};

const PIXSTYLE_MAGIC: &[u8; 4] = b"PXST";
const PIXSTYLE_FORMAT_VERSION: u16 = 1;

/// Maximum accepted compressed body. A `.pixstyle` is a small set of text
/// records; anything larger is rejected before decompression.
const MAX_COMPRESSED: usize = 8 * 1024 * 1024;
/// Maximum accepted decompressed body, bounding a decompression bomb.
const MAX_DECOMPRESSED: usize = 64 * 1024 * 1024;

/// Error reading or writing a `.pixstyle` bundle.
#[derive(Debug, Error)]
pub enum PixstyleError {
    /// Underlying I/O failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// The stream did not start with the `.pixstyle` magic bytes.
    #[error("bad magic: not a .pixstyle bundle")]
    BadMagic,
    /// The bundle's format version is newer than this build supports.
    #[error("unsupported format version {0}")]
    UnsupportedVersion(u16),
    /// The compressed or decompressed body exceeds the safety cap — guards the
    /// import path against a maliciously crafted decompression bomb.
    #[error("bundle exceeds the maximum allowed size")]
    TooLarge,
    /// `MessagePack` decode failure.
    #[error("decode: {0}")]
    Decode(#[from] rmp_serde::decode::Error),
    /// `MessagePack` encode failure.
    #[error("encode: {0}")]
    Encode(#[from] rmp_serde::encode::Error),
}

/// A portable bundle of composition records.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StylePack {
    /// Bundle format version.
    pub format_version: u16,
    /// Exported Structures.
    pub structures: Vec<Structure>,
    /// Exported Styles.
    pub styles: Vec<Style>,
    /// Exported saved Prompts.
    pub prompts: Vec<PromptTemplate>,
}

/// Writes `pack` to `w` as magic + version + zstd-compressed `MessagePack`.
///
/// # Errors
/// Returns [`PixstyleError`] on I/O, encode, or compression failure.
pub fn write_pack(pack: &StylePack, mut w: impl Write) -> Result<(), PixstyleError> {
    w.write_all(PIXSTYLE_MAGIC)?;
    w.write_all(&PIXSTYLE_FORMAT_VERSION.to_le_bytes())?;
    let body = rmp_serde::to_vec_named(pack)?;
    let compressed = zstd::encode_all(&body[..], 0)?;
    w.write_all(&compressed)?;
    Ok(())
}

/// Reads a [`StylePack`] from `r`, validating the magic and version header.
///
/// # Errors
/// Returns [`PixstyleError`] on bad magic, unsupported version, oversized body,
/// I/O, or decode failure.
pub fn read_pack(mut r: impl Read) -> Result<StylePack, PixstyleError> {
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != PIXSTYLE_MAGIC {
        return Err(PixstyleError::BadMagic);
    }
    let mut ver = [0u8; 2];
    r.read_exact(&mut ver)?;
    let version = u16::from_le_bytes(ver);
    if version != PIXSTYLE_FORMAT_VERSION {
        return Err(PixstyleError::UnsupportedVersion(version));
    }
    // Bound both the compressed read and the decompressed body so a tiny
    // malicious bundle can't exhaust memory before `rmp_serde` ever runs.
    let compressed = read_capped(r, MAX_COMPRESSED)?;
    let decoder = zstd::Decoder::new(&compressed[..])?;
    let body = read_capped(decoder, MAX_DECOMPRESSED)?;
    let pack: StylePack = rmp_serde::from_slice(&body)?;
    Ok(pack)
}

/// Reads at most `max` bytes from `r`; returns [`PixstyleError::TooLarge`] if
/// the source has more than `max` bytes.
fn read_capped(r: impl Read, max: usize) -> Result<Vec<u8>, PixstyleError> {
    let limit = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    let mut buf = Vec::new();
    r.take(limit).read_to_end(&mut buf)?;
    if buf.len() > max {
        return Err(PixstyleError::TooLarge);
    }
    Ok(buf)
}

/// Extracts a [`StylePack`] from an existing project's [`ProjectAi`] for
/// copy-from-project. The source project is read-only and left untouched.
#[must_use]
pub fn read_library_from_project_ai(ai: &ProjectAi) -> StylePack {
    StylePack {
        format_version: PIXSTYLE_FORMAT_VERSION,
        structures: ai.structures.clone(),
        styles: ai.styles.clone(),
        prompts: ai.prompts.clone(),
    }
}

// ── merge logic ─────────────────────────────────────────────────────────────

/// How a `.pixstyle` import resolves an id that already exists in the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// Keep the existing record; drop the incoming one.
    Skip,
    /// Replace the existing record with the incoming one.
    Overwrite,
    /// Add the incoming record under a fresh `.copy`-suffixed id.
    ImportAsCopy,
}

/// Counts of records affected by a merge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    /// Records added (new id, or `.copy` under `ImportAsCopy`).
    pub imported: u32,
    /// Records skipped because their id already existed under `Skip`.
    pub skipped: u32,
    /// Records replaced because their id already existed under `Overwrite`.
    pub overwritten: u32,
}

impl ImportReport {
    /// Adds another report's counts into this one.
    fn add(&mut self, other: ImportReport) {
        self.imported += other.imported;
        self.skipped += other.skipped;
        self.overwritten += other.overwritten;
    }
}

/// Mints a unique `<base>.copy` id (or `.copy.2`, `.copy.3`, …) that `exists`
/// reports as free, so `ImportAsCopy` never produces a duplicate id even when
/// `<base>.copy` is already taken or the same id repeats within one pack.
fn unique_copy_id(base: &str, exists: impl Fn(&str) -> bool) -> String {
    let first = format!("{base}.copy");
    if !exists(&first) {
        return first;
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("{base}.copy.{n}");
        if !exists(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Merges `incoming` Structures into `target` per `policy`, reporting the
/// counts. On `ImportAsCopy` a colliding id gains a unique `.copy` suffix.
#[must_use]
pub fn merge_structures(target: &mut Vec<Structure>, incoming: Vec<Structure>, policy: ConflictPolicy) -> ImportReport {
    let mut report = ImportReport::default();
    for s in incoming {
        match target.iter().position(|t| t.id == s.id) {
            Some(_) if matches!(policy, ConflictPolicy::Skip) => report.skipped += 1,
            Some(i) if matches!(policy, ConflictPolicy::Overwrite) => {
                target[i] = s;
                report.overwritten += 1;
            }
            Some(_) => {
                // ImportAsCopy.
                let mut c = s;
                c.id = StructureId(unique_copy_id(&c.id.0, |id| target.iter().any(|t| t.id.0 == id)));
                target.push(c);
                report.imported += 1;
            }
            None => {
                target.push(s);
                report.imported += 1;
            }
        }
    }
    report
}

/// Merges `incoming` Styles into `target` per `policy`. See [`merge_structures`].
#[must_use]
pub fn merge_styles(target: &mut Vec<Style>, incoming: Vec<Style>, policy: ConflictPolicy) -> ImportReport {
    let mut report = ImportReport::default();
    for s in incoming {
        match target.iter().position(|t| t.id == s.id) {
            Some(_) if matches!(policy, ConflictPolicy::Skip) => report.skipped += 1,
            Some(i) if matches!(policy, ConflictPolicy::Overwrite) => {
                target[i] = s;
                report.overwritten += 1;
            }
            Some(_) => {
                let mut c = s;
                c.id = StyleId(unique_copy_id(&c.id.0, |id| target.iter().any(|t| t.id.0 == id)));
                target.push(c);
                report.imported += 1;
            }
            None => {
                target.push(s);
                report.imported += 1;
            }
        }
    }
    report
}

/// Merges `incoming` Prompts into `target` per `policy`. See [`merge_structures`].
#[must_use]
pub fn merge_prompts(target: &mut Vec<PromptTemplate>, incoming: Vec<PromptTemplate>, policy: ConflictPolicy) -> ImportReport {
    let mut report = ImportReport::default();
    for p in incoming {
        match target.iter().position(|t| t.id == p.id) {
            Some(_) if matches!(policy, ConflictPolicy::Skip) => report.skipped += 1,
            Some(i) if matches!(policy, ConflictPolicy::Overwrite) => {
                target[i] = p;
                report.overwritten += 1;
            }
            Some(_) => {
                let mut c = p;
                c.id = PromptId(unique_copy_id(&c.id.0, |id| target.iter().any(|t| t.id.0 == id)));
                target.push(c);
                report.imported += 1;
            }
            None => {
                target.push(p);
                report.imported += 1;
            }
        }
    }
    report
}

/// Merges every kind of a [`StylePack`] into the project tier, summing reports.
#[must_use]
pub fn merge_pack(ai: &mut ProjectAi, pack: StylePack, policy: ConflictPolicy) -> ImportReport {
    let mut report = ImportReport::default();
    report.add(merge_structures(&mut ai.structures, pack.structures, policy));
    report.add(merge_styles(&mut ai.styles, pack.styles, policy));
    report.add(merge_prompts(&mut ai.prompts, pack.prompts, policy));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::library::composition::{ArtStyleKind, StructureOutput};

    fn structure(id: &str, name: &str) -> Structure {
        Structure {
            id: StructureId(id.into()),
            name: name.into(),
            output: StructureOutput::Single,
            layout_negatives: String::new(),
        }
    }

    fn style(id: &str, name: &str) -> Style {
        Style {
            id: StyleId(id.into()),
            name: name.into(),
            kind: ArtStyleKind::default(),
            modifiers: String::new(),
            look_negatives: String::new(),
            model_pref: None,
            quality: None,
        }
    }

    fn prompt(id: &str, name: &str) -> PromptTemplate {
        PromptTemplate {
            id: PromptId(id.into()),
            name: name.into(),
            text: String::new(),
            variables: Vec::new(),
            default_style: None,
            default_structure: None,
        }
    }

    fn sample_pack() -> StylePack {
        StylePack {
            format_version: PIXSTYLE_FORMAT_VERSION,
            structures: vec![structure("p.s", "P")],
            styles: vec![style("p.style", "Sample")],
            prompts: vec![prompt("p.prompt", "Warrior")],
        }
    }

    // ── format round-trip and header guards ──────────────────────────────────

    #[test]
    fn round_trips_through_bytes() {
        let mut buf = Vec::new();
        write_pack(&sample_pack(), &mut buf).unwrap();
        let back = read_pack(&buf[..]).unwrap();
        assert_eq!(back, sample_pack());
    }

    #[test]
    fn rejects_bad_magic() {
        let err = read_pack(&b"XXXX\x01\x00"[..]).unwrap_err();
        assert!(matches!(err, PixstyleError::BadMagic));
    }

    #[test]
    fn rejects_future_version() {
        let mut buf = Vec::new();
        buf.extend_from_slice(PIXSTYLE_MAGIC);
        buf.extend_from_slice(&99u16.to_le_bytes());
        buf.extend_from_slice(&zstd::encode_all(&rmp_serde::to_vec_named(&sample_pack()).unwrap()[..], 0).unwrap());
        assert!(matches!(read_pack(&buf[..]).unwrap_err(), PixstyleError::UnsupportedVersion(99)));
    }

    #[test]
    fn header_is_byte_identical_to_tauri() {
        // Magic then a little-endian u16 version. Locking these bytes keeps
        // packs interchangeable with the Tauri app's io/src/pixstyle.rs.
        let mut buf = Vec::new();
        write_pack(&StylePack::default(), &mut buf).unwrap();
        assert_eq!(&buf[0..4], b"PXST");
        assert_eq!(&buf[4..6], &1u16.to_le_bytes());
    }

    #[test]
    fn extracts_library_from_project_ai() {
        let mut ai = ProjectAi::default();
        ai.styles.push(style("p.style", "Sample"));
        let pack = read_library_from_project_ai(&ai);
        assert_eq!(pack.styles.len(), 1);
        assert_eq!(pack.format_version, PIXSTYLE_FORMAT_VERSION);
    }

    #[test]
    fn read_capped_rejects_oversized_source() {
        let data = [0u8; 100];
        assert!(matches!(read_capped(&data[..], 10), Err(PixstyleError::TooLarge)));
        // At or under the cap reads fully.
        assert_eq!(read_capped(&data[..], 100).unwrap().len(), 100);
        assert_eq!(read_capped(&data[..], 256).unwrap().len(), 100);
    }

    #[test]
    fn read_pack_rejects_decompression_bomb() {
        // A small compressed body that inflates past the decompressed cap.
        let bomb = vec![0u8; MAX_DECOMPRESSED + 1];
        let mut buf = Vec::new();
        buf.extend_from_slice(PIXSTYLE_MAGIC);
        buf.extend_from_slice(&PIXSTYLE_FORMAT_VERSION.to_le_bytes());
        buf.extend_from_slice(&zstd::encode_all(&bomb[..], 0).unwrap());
        assert!(matches!(read_pack(&buf[..]).unwrap_err(), PixstyleError::TooLarge));
    }

    #[test]
    fn read_pack_rejects_oversized_compressed_body() {
        // A compressed body past MAX_COMPRESSED is rejected before inflation.
        let mut buf = Vec::new();
        buf.extend_from_slice(PIXSTYLE_MAGIC);
        buf.extend_from_slice(&PIXSTYLE_FORMAT_VERSION.to_le_bytes());
        buf.extend_from_slice(&vec![0u8; MAX_COMPRESSED + 1]);
        assert!(matches!(read_pack(&buf[..]).unwrap_err(), PixstyleError::TooLarge));
    }

    // ── merge logic (ported from the Tauri command module) ───────────────────

    #[test]
    fn merge_structures_skip_keeps_existing_and_counts_skipped() {
        let mut target = vec![structure("a", "original")];
        let report = merge_structures(&mut target, vec![structure("a", "incoming")], ConflictPolicy::Skip);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.imported, 0);
        assert_eq!(report.overwritten, 0);
        assert_eq!(target.len(), 1);
        assert_eq!(target[0].name, "original");
    }

    #[test]
    fn merge_structures_overwrite_replaces() {
        let mut target = vec![structure("a", "original")];
        let report = merge_structures(&mut target, vec![structure("a", "incoming")], ConflictPolicy::Overwrite);
        assert_eq!(report.overwritten, 1);
        assert_eq!(report.imported, 0);
        assert_eq!(target.len(), 1);
        assert_eq!(target[0].name, "incoming");
    }

    #[test]
    fn merge_structures_import_as_copy_adds_suffixed_id() {
        let mut target = vec![structure("a", "original")];
        let report = merge_structures(&mut target, vec![structure("a", "incoming")], ConflictPolicy::ImportAsCopy);
        assert_eq!(report.imported, 1);
        assert_eq!(target.len(), 2);
        assert_eq!(target[1].id, StructureId("a.copy".into()));
        assert_eq!(target[1].name, "incoming");
    }

    #[test]
    fn merge_structures_import_as_copy_avoids_existing_copy_id() {
        // `a` and `a.copy` already present: importing `a` must mint `a.copy.2`,
        // and importing the same id twice in one pack must not collide either.
        let mut target = vec![structure("a", "orig"), structure("a.copy", "prev")];
        let report = merge_structures(
            &mut target,
            vec![structure("a", "first"), structure("a", "second")],
            ConflictPolicy::ImportAsCopy,
        );
        assert_eq!(report.imported, 2);
        let ids: Vec<&str> = target.iter().map(|s| s.id.0.as_str()).collect();
        assert_eq!(ids, ["a", "a.copy", "a.copy.2", "a.copy.3"]);
    }

    #[test]
    fn merge_structures_new_id_is_imported() {
        let mut target = vec![structure("a", "original")];
        let report = merge_structures(&mut target, vec![structure("b", "fresh")], ConflictPolicy::Skip);
        assert_eq!(report.imported, 1);
        assert_eq!(target.len(), 2);
    }

    #[test]
    fn merge_styles_import_as_copy_adds_suffixed_id() {
        let mut target = vec![style("a", "original")];
        let report = merge_styles(&mut target, vec![style("a", "incoming")], ConflictPolicy::ImportAsCopy);
        assert_eq!(report.imported, 1);
        assert_eq!(target[1].id, StyleId("a.copy".into()));
    }

    #[test]
    fn merge_styles_overwrite_replaces() {
        let mut target = vec![style("a", "original")];
        let report = merge_styles(&mut target, vec![style("a", "incoming")], ConflictPolicy::Overwrite);
        assert_eq!(report.overwritten, 1);
        assert_eq!(target[0].name, "incoming");
    }

    #[test]
    fn merge_prompts_skip_keeps_existing() {
        let mut target = vec![prompt("a", "original")];
        let report = merge_prompts(&mut target, vec![prompt("a", "incoming")], ConflictPolicy::Skip);
        assert_eq!(report.skipped, 1);
        assert_eq!(target[0].name, "original");
    }

    #[test]
    fn merge_prompts_import_as_copy_adds_suffixed_id() {
        let mut target = vec![prompt("a", "original")];
        let report = merge_prompts(&mut target, vec![prompt("a", "incoming")], ConflictPolicy::ImportAsCopy);
        assert_eq!(report.imported, 1);
        assert_eq!(target[1].id, PromptId("a.copy".into()));
    }

    #[test]
    fn import_report_add_sums_counts() {
        let mut a = ImportReport {
            imported: 1,
            skipped: 2,
            overwritten: 3,
        };
        a.add(ImportReport {
            imported: 10,
            skipped: 20,
            overwritten: 30,
        });
        assert_eq!(a.imported, 11);
        assert_eq!(a.skipped, 22);
        assert_eq!(a.overwritten, 33);
    }

    #[test]
    fn merge_pack_sums_all_kinds_and_overwrites() {
        let mut ai = ProjectAi::default();
        ai.structures.push(structure("a", "orig"));
        ai.styles.push(style("a", "orig"));
        let report = merge_pack(
            &mut ai,
            StylePack {
                format_version: PIXSTYLE_FORMAT_VERSION,
                structures: vec![structure("a", "new"), structure("b", "fresh")],
                styles: vec![style("a", "new")],
                prompts: vec![prompt("p", "new")],
            },
            ConflictPolicy::Overwrite,
        );
        assert_eq!(report.imported, 2);
        assert_eq!(report.overwritten, 2);
        assert_eq!(report.skipped, 0);
        assert_eq!(ai.structures[0].name, "new");
        assert_eq!(ai.styles[0].name, "new");
        assert_eq!(ai.prompts.len(), 1);
    }

    #[test]
    fn merge_pack_skip_reports_skipped_without_duplicating() {
        let mut ai = ProjectAi::default();
        ai.structures.push(structure("a", "orig"));
        let report = merge_pack(
            &mut ai,
            StylePack {
                format_version: PIXSTYLE_FORMAT_VERSION,
                structures: vec![structure("a", "incoming")],
                styles: vec![],
                prompts: vec![],
            },
            ConflictPolicy::Skip,
        );
        assert_eq!(report.skipped, 1);
        assert_eq!(report.imported, 0);
        assert_eq!(ai.structures.len(), 1);
        assert_eq!(ai.structures[0].name, "orig");
    }
}
