use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::{AppError, AppResult};
use crate::sync_format::{BundleInstanceRecord, BundleManifest};

pub fn read_bundle(bundle_path: &Path) -> AppResult<(BundleManifest, Vec<BundleInstanceRecord>)> {
    let file = File::open(bundle_path).map_err(|error| {
        AppError::config(format!("failed to open {}: {error}", bundle_path.display()))
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        AppError::config(format!(
            "failed to read zip archive {}: {error}",
            bundle_path.display()
        ))
    })?;

    let manifest = {
        let mut file = archive.by_name("manifest.json").map_err(|error| {
            AppError::config(format!(
                "bundle {} is missing manifest.json: {error}",
                bundle_path.display()
            ))
        })?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).map_err(|error| {
            AppError::config(format!(
                "failed to read manifest.json from {}: {error}",
                bundle_path.display()
            ))
        })?;
        serde_json::from_str(&contents).map_err(|error| {
            AppError::config(format!(
                "failed to parse manifest.json in {}: {error}",
                bundle_path.display()
            ))
        })?
    };

    let records = {
        let file = archive.by_name("instances.jsonl").map_err(|error| {
            AppError::config(format!(
                "bundle {} is missing instances.jsonl: {error}",
                bundle_path.display()
            ))
        })?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(|error| {
                AppError::config(format!(
                    "failed to read instances.jsonl from {}: {error}",
                    bundle_path.display()
                ))
            })?;
            if line.trim().is_empty() {
                continue;
            }
            records.push(
                serde_json::from_str::<BundleInstanceRecord>(&line).map_err(|error| {
                    AppError::config(format!(
                        "failed to parse bundle instance line in {}: {error}",
                        bundle_path.display()
                    ))
                })?,
            );
        }
        records
    };

    Ok((manifest, records))
}

pub fn write_bundle(
    bundle_path: &Path,
    manifest: &BundleManifest,
    records: &[BundleInstanceRecord],
) -> AppResult<()> {
    if let Some(parent) = bundle_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppError::config(format!(
                "failed to create bundle directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    let file = File::create(bundle_path).map_err(|error| {
        AppError::config(format!(
            "failed to create {}: {error}",
            bundle_path.display()
        ))
    })?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    writer
        .start_file("manifest.json", options)
        .map_err(|error| {
            AppError::config(format!(
                "failed to start manifest.json in {}: {error}",
                bundle_path.display()
            ))
        })?;
    let manifest_json = serde_json::to_vec_pretty(manifest).map_err(|error| {
        AppError::general(format!("failed to serialize bundle manifest: {error}"))
    })?;
    writer.write_all(&manifest_json).map_err(|error| {
        AppError::config(format!(
            "failed to write manifest.json in {}: {error}",
            bundle_path.display()
        ))
    })?;

    writer
        .start_file("instances.jsonl", options)
        .map_err(|error| {
            AppError::config(format!(
                "failed to start instances.jsonl in {}: {error}",
                bundle_path.display()
            ))
        })?;
    for record in records {
        let line = serde_json::to_string(record).map_err(|error| {
            AppError::general(format!("failed to serialize bundle record: {error}"))
        })?;
        writer.write_all(line.as_bytes()).map_err(|error| {
            AppError::config(format!(
                "failed to write instances.jsonl in {}: {error}",
                bundle_path.display()
            ))
        })?;
        writer.write_all(b"\n").map_err(|error| {
            AppError::config(format!(
                "failed to finalize instances.jsonl in {}: {error}",
                bundle_path.display()
            ))
        })?;
    }

    writer.finish().map_err(|error| {
        AppError::config(format!(
            "failed to finish bundle {}: {error}",
            bundle_path.display()
        ))
    })?;
    Ok(())
}
