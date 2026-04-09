use std::io::{self, Write};

use serde::Serialize;

pub fn write_json_to_writer<T: Serialize>(writer: &mut impl Write, value: &T) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    Ok(())
}

pub fn write_json_stdout<T: Serialize>(value: &T) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    write_json_to_writer(&mut stdout, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize)]
    struct VersionJson<'a> {
        name: &'a str,
        version: &'a str,
    }

    #[test]
    fn json_writer_is_minified_and_stable() {
        let value = VersionJson {
            name: "kb",
            version: "0.0.0",
        };
        let mut buf = Vec::new();
        write_json_to_writer(&mut buf, &value).expect("write json");
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "{\"name\":\"kb\",\"version\":\"0.0.0\"}"
        );
    }
}
