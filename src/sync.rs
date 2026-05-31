use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;
use walkdir::WalkDir;

use crate::cli::{ReadOutput, SyncCommand, SyncDirCommand, SyncPushCommand, SyncServeCommand};
use crate::error::{AppError, AppResult};
use crate::output::print_json;
use crate::sync_bundle::{read_bundle, write_bundle};
use crate::sync_format::{
    BundleInstanceRecord, BundleManifest, ProjectManifest, SyncResult, XupInstance, XzupFile,
};

const PROJECT_MANIFEST_FILE: &str = "rbxup.project.json";

pub async fn run_sync(command: SyncCommand) -> AppResult<()> {
    match command {
        SyncCommand::Pull(args) => run_sync_pull(args),
        SyncCommand::Push(args) => run_sync_push(args),
        SyncCommand::Diff(args) => run_sync_diff(args),
        SyncCommand::Serve(args) => run_sync_serve(args),
        SyncCommand::Doctor(args) => run_sync_doctor(args),
    }
}

fn run_sync_pull(args: SyncDirCommand) -> AppResult<()> {
    let report = ensure_project_layout(&args.dir)?;
    let sample_bundle_path = ensure_sample_bundle(&args.dir)?;
    let payload = SyncActionReport {
        action: "pull".to_string(),
        project_dir: args.dir.display().to_string(),
        manifest_path: report.manifest_path,
        service_count: report.service_count,
        xup_count: report.xup_count,
        xzup_count: report.xzup_count,
        bundle_count: report.bundle_count,
        warnings: report.warnings,
        notes: vec![
            "Early sync scaffold only validates and seeds the project structure.".to_string(),
            format!("Sample bundle ready at {}", sample_bundle_path.display()),
        ],
    };
    print_sync_report(&payload, args.output)
}

fn run_sync_push(args: SyncPushCommand) -> AppResult<()> {
    let report = validate_project(&args.dir)?;
    let payload = SyncActionReport {
        action: if args.delete {
            "push-delete".to_string()
        } else {
            "push".to_string()
        },
        project_dir: args.dir.display().to_string(),
        manifest_path: report.manifest_path,
        service_count: report.service_count,
        xup_count: report.xup_count,
        xzup_count: report.xzup_count,
        bundle_count: report.bundle_count,
        warnings: report.warnings,
        notes: vec![
            "Safe patch-mode push is not connected to Studio yet.".to_string(),
            "Use `sync serve` with the plugin bridge once the Studio side is loaded.".to_string(),
        ],
    };
    print_sync_report(&payload, args.output)
}

fn run_sync_diff(args: SyncDirCommand) -> AppResult<()> {
    let report = validate_project(&args.dir)?;
    let payload = SyncActionReport {
        action: "diff".to_string(),
        project_dir: args.dir.display().to_string(),
        manifest_path: report.manifest_path,
        service_count: report.service_count,
        xup_count: report.xup_count,
        xzup_count: report.xzup_count,
        bundle_count: report.bundle_count,
        warnings: report.warnings,
        notes: vec![
            "Early diff mode only validates disk state.".to_string(),
            "Roblox Studio object-level diffing is still a TODO.".to_string(),
        ],
    };
    print_sync_report(&payload, args.output)
}

fn run_sync_doctor(args: SyncDirCommand) -> AppResult<()> {
    let report = validate_project(&args.dir)?;
    let payload = SyncActionReport {
        action: "doctor".to_string(),
        project_dir: args.dir.display().to_string(),
        manifest_path: report.manifest_path,
        service_count: report.service_count,
        xup_count: report.xup_count,
        xzup_count: report.xzup_count,
        bundle_count: report.bundle_count,
        warnings: report.warnings,
        notes: vec![
            "Doctor currently validates manifests, bundle pointers, and duplicate IDs on disk."
                .to_string(),
        ],
    };
    print_sync_report(&payload, args.output)
}

fn run_sync_serve(args: SyncServeCommand) -> AppResult<()> {
    let report = validate_project(&args.dir)?;
    let listener = TcpListener::bind(("127.0.0.1", args.port)).map_err(|error| {
        AppError::general(format!(
            "failed to bind sync bridge server on 127.0.0.1:{}: {error}",
            args.port
        ))
    })?;

    eprintln!(
        "sync bridge server listening on http://127.0.0.1:{} for {}",
        args.port,
        args.dir.display()
    );

    if let Ok((mut stream, _)) = listener.accept() {
        let mut buffer = [0u8; 8192];
        let bytes_read = stream.read(&mut buffer).map_err(|error| {
            AppError::general(format!("failed to read sync bridge request: {error}"))
        })?;
        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
        let first_line = request.lines().next().unwrap_or_default().to_string();
        let request_message = crate::sync_format::SyncMessage {
            kind: "sync-message".to_string(),
            version: 1,
            command: "serve".to_string(),
            payload: json!({
                "requestLine": first_line,
            }),
        };
        let response = SyncResult {
            ok: true,
            warnings: report.warnings.clone(),
            errors: Vec::new(),
            payload: json!({
                "message": "rbxup sync bridge scaffold",
                "projectDir": args.dir.display().to_string(),
                "request": request_message,
                "manifestPath": report.manifest_path,
            }),
        };
        let body = serde_json::to_string_pretty(&response).map_err(|error| {
            AppError::general(format!("failed to serialize bridge response: {error}"))
        })?;
        let http_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(http_response.as_bytes())
            .map_err(|error| {
                AppError::general(format!("failed to write sync bridge response: {error}"))
            })?;
    }

    Ok(())
}

fn ensure_project_layout(project_dir: &Path) -> AppResult<ValidationReport> {
    fs::create_dir_all(project_dir).map_err(|error| {
        AppError::config(format!(
            "failed to create project directory {}: {error}",
            project_dir.display()
        ))
    })?;

    let manifest_path = project_dir.join(PROJECT_MANIFEST_FILE);
    if !manifest_path.exists() {
        let manifest = ProjectManifest::default_manifest();
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).map_err(|error| {
                AppError::general(format!("failed to serialize project manifest: {error}"))
            })?,
        )
        .map_err(|error| {
            AppError::config(format!(
                "failed to write {}: {error}",
                manifest_path.display()
            ))
        })?;
    }

    let manifest = load_manifest(&manifest_path)?;
    for service_path in manifest.services.values() {
        let service_path = project_dir.join(service_path);
        if let Some(parent) = service_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppError::config(format!(
                    "failed to create service directory {}: {error}",
                    parent.display()
                ))
            })?;
        }
        if !service_path.exists() {
            let service_name = service_path
                .parent()
                .and_then(|path| path.file_name())
                .and_then(|value| value.to_str())
                .unwrap_or("Service");
            let xup = XupInstance {
                format: "xup".to_string(),
                version: 1,
                id: format!("inst_{}", service_name.to_ascii_lowercase()),
                class_name: service_name.to_string(),
                name: service_name.to_string(),
                properties: Default::default(),
                attributes: Default::default(),
                tags: Vec::new(),
            };
            fs::write(
                &service_path,
                serde_json::to_vec_pretty(&xup).map_err(|error| {
                    AppError::general(format!("failed to serialize service xup: {error}"))
                })?,
            )
            .map_err(|error| {
                AppError::config(format!(
                    "failed to write {}: {error}",
                    service_path.display()
                ))
            })?;
        }
    }

    validate_project(project_dir)
}

fn validate_project(project_dir: &Path) -> AppResult<ValidationReport> {
    let manifest_path = project_dir.join(PROJECT_MANIFEST_FILE);
    let manifest = load_manifest(&manifest_path)?;
    let mut warnings = Vec::new();
    let mut xup_count = 0usize;
    let mut xzup_count = 0usize;
    let mut bundle_count = 0usize;
    let mut ids = BTreeSet::new();

    for service_path in manifest.services.values() {
        let full_path = project_dir.join(service_path);
        if !full_path.exists() {
            return Err(AppError::config(format!(
                "service entry {} points to missing file {}",
                service_path,
                full_path.display()
            )));
        }
    }

    for entry in WalkDir::new(project_dir) {
        let entry = entry.map_err(|error| {
            AppError::config(format!(
                "failed to scan sync project {}: {error}",
                project_dir.display()
            ))
        })?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        match path.extension().and_then(|value| value.to_str()) {
            Some("xup") => {
                xup_count += 1;
                let xup: XupInstance = read_json(path)?;
                if !ids.insert(xup.id.clone()) {
                    warnings.push(format!(
                        "duplicate stable ID `{}` found at {}",
                        xup.id,
                        path.display()
                    ));
                }
            }
            Some("xzup") => {
                xzup_count += 1;
                let xzup: XzupFile = read_json(path)?;
                let bundle_path = match &xzup {
                    XzupFile::Instance(value) => &value.bundle.path,
                    XzupFile::ChildGroup(value) => &value.bundle.path,
                };
                let bundle_path = normalize_bundle_path(path, bundle_path);
                if !bundle_path.exists() {
                    return Err(AppError::config(format!(
                        "bundle pointer {} references missing bundle {}",
                        path.display(),
                        bundle_path.display()
                    )));
                }
                let (manifest, records) = read_bundle(&bundle_path)?;
                bundle_count += 1;
                if manifest.instance_count != records.len() {
                    warnings.push(format!(
                        "bundle {} declares {} instances but contains {}",
                        bundle_path.display(),
                        manifest.instance_count,
                        records.len()
                    ));
                }
            }
            Some("zip") if path.extension().and_then(|value| value.to_str()) == Some("zip") => {
                if path.to_string_lossy().ends_with(".xbundle.zip") {
                    bundle_count += 1;
                }
            }
            _ => {}
        }
    }

    Ok(ValidationReport {
        manifest_path: manifest_path.display().to_string(),
        service_count: manifest.services.len(),
        xup_count,
        xzup_count,
        bundle_count,
        warnings,
    })
}

fn ensure_sample_bundle(project_dir: &Path) -> AppResult<PathBuf> {
    let bundle_path = project_dir
        .join("bundles")
        .join("00")
        .join("00sample.xbundle.zip");
    if bundle_path.exists() {
        return Ok(bundle_path);
    }

    let manifest = BundleManifest {
        format: "xbundle".to_string(),
        version: 1,
        group_by: "className".to_string(),
        class_name: Some("Part".to_string()),
        parent_id: "inst_workspace".to_string(),
        instance_count: 1,
    };
    let records = vec![BundleInstanceRecord {
        id: "part_sample".to_string(),
        name: "SamplePart".to_string(),
        class_name: Some("Part".to_string()),
        properties: Default::default(),
        attributes: Default::default(),
        tags: Vec::new(),
        children: Vec::new(),
    }];
    write_bundle(&bundle_path, &manifest, &records)?;
    Ok(bundle_path)
}

fn normalize_bundle_path(pointer_file: &Path, bundle_path: &str) -> PathBuf {
    let relative = Path::new(bundle_path);
    if relative.is_absolute() {
        relative.to_path_buf()
    } else {
        pointer_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(relative)
    }
}

fn load_manifest(path: &Path) -> AppResult<ProjectManifest> {
    let manifest: ProjectManifest = read_json(path)?;
    if manifest.format != "rbxup-project" {
        return Err(AppError::config(format!(
            "{} is not an rbxup sync project manifest",
            path.display()
        )));
    }
    Ok(manifest)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> AppResult<T> {
    let bytes = fs::read(path)
        .map_err(|error| AppError::config(format!("failed to read {}: {error}", path.display())))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| AppError::config(format!("failed to parse {}: {error}", path.display())))
}

fn print_sync_report(report: &SyncActionReport, output: ReadOutput) -> AppResult<()> {
    match output {
        ReadOutput::Json => print_json(report),
        ReadOutput::Pretty => {
            println!("Action: {}", report.action);
            println!("Project Dir: {}", report.project_dir);
            println!("Manifest: {}", report.manifest_path);
            println!("Services: {}", report.service_count);
            println!("XUP Files: {}", report.xup_count);
            println!("XZUP Files: {}", report.xzup_count);
            println!("Bundle Files: {}", report.bundle_count);
            if !report.warnings.is_empty() {
                println!("Warnings:");
                for warning in &report.warnings {
                    println!("- {warning}");
                }
            }
            if !report.notes.is_empty() {
                println!("Notes:");
                for note in &report.notes {
                    println!("- {note}");
                }
            }
            Ok(())
        }
    }
}

#[derive(Debug, Serialize)]
struct SyncActionReport {
    action: String,
    #[serde(rename = "projectDir")]
    project_dir: String,
    #[serde(rename = "manifestPath")]
    manifest_path: String,
    #[serde(rename = "serviceCount")]
    service_count: usize,
    #[serde(rename = "xupCount")]
    xup_count: usize,
    #[serde(rename = "xzupCount")]
    xzup_count: usize,
    #[serde(rename = "bundleCount")]
    bundle_count: usize,
    warnings: Vec<String>,
    notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct ValidationReport {
    manifest_path: String,
    service_count: usize,
    xup_count: usize,
    xzup_count: usize,
    bundle_count: usize,
    warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{PROJECT_MANIFEST_FILE, ensure_project_layout, validate_project};

    fn temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rbxup-sync-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    #[test]
    fn seeds_and_validates_sync_layout() {
        let root = temp_dir();
        let report = ensure_project_layout(&root).expect("seed layout");
        assert!(root.join(PROJECT_MANIFEST_FILE).exists());
        assert!(report.service_count >= 1);

        let validated = validate_project(&root).expect("validate");
        assert_eq!(validated.service_count, report.service_count);

        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
