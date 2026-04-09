use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;

use crate::error::KbError;
use crate::index::artifacts::{KbMeta, KbSchema};
use crate::io::json::write_json_to_writer;

pub fn write_kb_meta(gen_dir: &Path) -> Result<(), KbError> {
    let mut schemas = vec![
        KbSchema {
            name: "kb/gen/tree.jsonl".to_string(),
            version: 1,
            required: true,
        },
        KbSchema {
            name: "kb/gen/symbols.jsonl".to_string(),
            version: 1,
            required: true,
        },
        KbSchema {
            name: "kb/gen/deps.jsonl".to_string(),
            version: 1,
            required: true,
        },
        KbSchema {
            name: "kb/gen/xrefs.jsonl".to_string(),
            version: 1,
            required: false,
        },
    ];
    schemas.sort_by(|a, b| a.name.cmp(&b.name));

    let meta = KbMeta {
        kb_format_version: 1,
        schemas,
    };

    std::fs::create_dir_all(gen_dir)
        .map_err(|err| KbError::internal(err, "failed to create kb/gen"))?;
    let file = File::create(gen_dir.join("kb_meta.json"))
        .map_err(|err| KbError::internal(err, "failed to write kb_meta.json"))?;
    let mut writer = io::BufWriter::new(file);
    write_json_to_writer(&mut writer, &meta)
        .map_err(|err| KbError::internal(err, "failed to write kb_meta.json"))?;
    writer
        .flush()
        .map_err(|err| KbError::internal(err, "failed to flush kb_meta.json"))?;
    Ok(())
}
