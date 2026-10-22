//! Private, single-dataset Lite/Studio edit protocol v1. No executable paths.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::Path,
};

pub const CAPABILITY: &str = "INSTPLOT_STUDIO_EDIT_PROTOCOL_V1";
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub name: String,
    pub values: Vec<Option<f64>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transfer {
    pub schema_version: u32,
    pub session_id: String,
    pub target_id: String,
    pub base_sha256: String,
    pub label: String,
    pub x_column: String,
    pub y_column: String,
    pub columns: Vec<Column>,
    pub alive: Vec<bool>,
}

impl Transfer {
    pub fn validate(&self) -> Result<(), String> {
        let hex = |s: &str| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit());
        if self.schema_version != 1
            || !hex(&self.session_id)
            || !hex(&self.base_sha256)
            || self.target_id.is_empty()
            || self.target_id.len() > 256
            || self.label.len() > 2048
        {
            return Err("invalid edit session identity/version".to_owned());
        }
        if self.columns.len() < 2 || self.columns.len() > 128 || self.alive.len() > 1_000_000 {
            return Err("edit dataset exceeds limits".to_owned());
        }
        let mut names = BTreeSet::new();
        for column in &self.columns {
            if column.name.is_empty()
                || column.name.len() > 512
                || !names.insert(&column.name)
                || column.values.len() != self.alive.len()
                || column.values.iter().flatten().any(|v| !v.is_finite())
            {
                return Err("invalid edit columns/rows".to_owned());
            }
        }
        let x = self.columns.iter().find(|c| c.name == self.x_column);
        let y = self.columns.iter().find(|c| c.name == self.y_column);
        let (Some(x), Some(y)) = (x, y) else {
            return Err("missing X/Y columns".to_owned());
        };
        if x.name == y.name
            || self
                .alive
                .iter()
                .enumerate()
                .filter(|(i, live)| **live && x.values[*i].is_some() && y.values[*i].is_some())
                .count()
                < 2
        {
            return Err("at least two live, valid X/Y pairs are required".to_owned());
        }
        Ok(())
    }

    pub fn matches_session(&self, original: &Self) -> bool {
        self.session_id == original.session_id
            && self.target_id == original.target_id
            && self.base_sha256 == original.base_sha256
    }
}

pub fn read_transfer(path: &Path) -> Result<Transfer, String> {
    let parent = path.parent().ok_or("missing edit directory")?;
    if parent.canonicalize().map_err(|e| e.to_string())? != parent
        || fs::symlink_metadata(parent)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err("edit directory must be canonical and not a link".to_owned());
    }
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("edit transfer must be a bounded regular file".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(parent)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err("edit directory must be private".to_owned());
        }
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("edit transfer too large".to_owned());
    }
    let transfer: Transfer = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    transfer.validate()?;
    Ok(transfer)
}

/// Publish complete bytes once. A failed/repeated send never overwrites a result.
pub fn write_transfer(path: &Path, transfer: &Transfer) -> Result<(), String> {
    transfer.validate()?;
    let bytes = serde_json::to_vec(transfer).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("edit transfer too large".to_owned());
    }
    let part = path.with_extension("partial");
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&part).map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::hard_link(&part, path)?;
        Ok::<_, std::io::Error>(())
    })();
    // Only this invocation's create_new staging file is removed.
    let _ = fs::remove_file(&part);
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Transfer {
        Transfer {
            schema_version: 1,
            session_id: "a".repeat(64),
            target_id: "source-3".into(),
            base_sha256: "b".repeat(64),
            label: "3 nm".into(),
            x_column: "x".into(),
            y_column: "y".into(),
            columns: vec![
                Column {
                    name: "x".into(),
                    values: vec![Some(0.0), Some(1.0), None],
                },
                Column {
                    name: "y".into(),
                    values: vec![Some(2.0), Some(3.0), None],
                },
            ],
            alive: vec![true; 3],
        }
    }
    #[test]
    fn validates_identity_masks_limits_and_paired_data() {
        let original = sample();
        original.validate().unwrap();
        let mut edit = original.clone();
        edit.columns[1].values[0] = Some(5.0);
        assert!(edit.matches_session(&original));
        edit.session_id = "c".repeat(64);
        assert!(!edit.matches_session(&original));
        edit.columns[0].values.pop();
        assert!(edit.validate().is_err());
        let mut edit = original.clone();
        edit.columns[1].name = "x".into();
        assert!(edit.validate().is_err());
        let mut edit = original.clone();
        edit.alive[0] = false;
        assert!(edit.validate().is_err());
        let mut edit = original;
        edit.schema_version = 2;
        assert!(edit.validate().is_err());
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("instplot-edit-test-{}-{id}", std::process::id()));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let mut builder = builder;
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&root).unwrap();
        root
    }
    #[test]
    fn complete_file_publishes_once_rejects_links_and_bad_json() {
        let root = root();
        let path = root.join("request.json");
        let transfer = Transfer {
            schema_version: 1,
            session_id: "a".repeat(64),
            target_id: "3nm".into(),
            base_sha256: "b".repeat(64),
            label: "3 nm 电阻".into(),
            x_column: "x".into(),
            y_column: "y".into(),
            columns: vec![
                Column {
                    name: "x".into(),
                    values: vec![Some(0.0), Some(1.0)],
                },
                Column {
                    name: "y".into(),
                    values: vec![Some(2.0), Some(3.0)],
                },
            ],
            alive: vec![true; 2],
        };
        write_transfer(&path, &transfer).unwrap();
        assert_eq!(read_transfer(&path).unwrap(), transfer);
        assert!(write_transfer(&path, &transfer).is_err());
        assert_eq!(read_transfer(&path).unwrap(), transfer);
        #[cfg(unix)]
        {
            let link = root.join("link.json");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(read_transfer(&link).is_err());
            fs::remove_file(link).unwrap();
        }
        fs::write(&path, b"{ broken").unwrap();
        assert!(read_transfer(&path).is_err());
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&root).unwrap();
    }
}
