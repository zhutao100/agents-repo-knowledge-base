use std::cmp::Ordering;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use serde::Serialize;

pub fn stable_sort_by_key<T, K: Ord>(values: &mut [T], key_fn: impl Fn(&T) -> K) {
    values.sort_by_key(key_fn);
}

pub fn stable_sort_by<T>(values: &mut [T], compare: impl Fn(&T, &T) -> Ordering) {
    values.sort_by(compare);
}

pub fn write_jsonl_to_writer<T: Serialize>(
    writer: &mut impl Write,
    records: &[T],
) -> io::Result<()> {
    for record in records {
        serde_json::to_writer(&mut *writer, record)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

pub fn write_jsonl_file<T: Serialize>(path: &Path, records: &[T]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = io::BufWriter::new(file);
    write_jsonl_to_writer(&mut writer, records)?;
    writer.flush()?;
    Ok(())
}

pub fn write_jsonl_file_sorted<T: Serialize>(
    path: &Path,
    records: &mut [T],
    compare: impl Fn(&T, &T) -> Ordering,
) -> io::Result<()> {
    stable_sort_by(records, compare);
    write_jsonl_file(path, records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize)]
    struct Row<'a> {
        a: &'a str,
    }

    #[test]
    fn jsonl_always_ends_with_newline() {
        let mut buf = Vec::new();
        let rows = vec![Row { a: "x" }, Row { a: "y" }];
        write_jsonl_to_writer(&mut buf, &rows).expect("write jsonl");
        assert!(buf.ends_with(b"\n"));
    }
}
