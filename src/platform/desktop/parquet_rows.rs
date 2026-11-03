//! Row-by-row reading of the directories of parquet files the syncer keeps in `.smugmug_db`.
//!
//! The syncer writes these with polars.  The viewer only reads a few columns, so it uses the
//! lighter `parquet` crate's row API with a column projection instead.

use std::{collections::HashMap, fs::File, path::Path};

use anyhow::{Context, Result};
use parquet::{
    file::reader::{FileReader, SerializedFileReader},
    record::Field,
    schema::types::Type,
};

/// One row, by column name
pub struct Row(HashMap<String, Field>);

impl Row {
    pub fn str(&self, name: &str) -> Option<&str> {
        match self.0.get(name)? {
            Field::Str(v) => Some(v.as_str()),
            _ => None,
        }
    }

    pub fn string(&self, name: &str) -> Option<String> {
        self.str(name).map(str::to_string)
    }

    pub fn u64(&self, name: &str) -> Option<u64> {
        match self.0.get(name)? {
            Field::ULong(v) => Some(*v),
            Field::UInt(v) => Some(u64::from(*v)),
            Field::UShort(v) => Some(u64::from(*v)),
            Field::Long(v) => u64::try_from(*v).ok(),
            Field::Int(v) => u64::try_from(*v).ok(),
            _ => None,
        }
    }

    pub fn f64(&self, name: &str) -> Option<f64> {
        match self.0.get(name)? {
            Field::Double(v) => Some(*v),
            Field::Float(v) => Some(f64::from(*v)),
            _ => None,
        }
    }
}

/// Calls `f(file_name, row)` for each row of every parquet file in `dir`.  A missing directory
/// has no rows.
pub fn for_each_row(dir: &Path, columns: &[&str], f: impl FnMut(&str, &Row)) -> Result<()> {
    for_each_row_in(dir, |_| true, columns, f)
}

/// Like [`for_each_row`], limited to the files whose name passes `filter`
pub fn for_each_row_in(
    dir: &Path,
    filter: impl Fn(&str) -> bool,
    columns: &[&str],
    mut f: impl FnMut(&str, &Row),
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let mut files: Vec<_> = entries
        .filter_map(|v| v.ok().map(|v| v.path()))
        .filter(|v| v.extension().is_some_and(|ext| ext == "parquet"))
        .collect();
    files.sort();

    for path in files {
        let name = path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_default();
        if !filter(&name) {
            continue;
        }
        read_file(&path, columns, |row| f(&name, row))
            .with_context(|| format!("reading {}", path.display()))?;
    }
    Ok(())
}

fn read_file(path: &Path, columns: &[&str], mut f: impl FnMut(&Row)) -> Result<()> {
    let reader = SerializedFileReader::new(File::open(path)?)?;
    let schema = reader.metadata().file_metadata().schema();
    let fields = schema
        .get_fields()
        .iter()
        .filter(|v| columns.contains(&v.name()))
        .cloned()
        .collect();
    let projection = Type::group_type_builder(schema.name())
        .with_fields(fields)
        .build()?;

    for row in reader.get_row_iter(Some(projection))? {
        let row = row?;
        let row = Row(row
            .into_columns()
            .into_iter()
            .collect::<HashMap<String, Field>>());
        f(&row);
    }
    Ok(())
}
