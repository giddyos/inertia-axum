use serde::Serialize;
use serde_json::ser::{Formatter, Serializer};
use std::io::{self, Write};

#[derive(Default)]
struct ScriptSafeFormatter;

impl Formatter for ScriptSafeFormatter {
    fn write_string_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut start = 0;
        for (index, character) in fragment.char_indices() {
            let replacement: Option<&[u8]> = match character {
                '<' => Some(b"\\u003C"),
                '>' => Some(b"\\u003E"),
                '&' => Some(b"\\u0026"),
                '\u{2028}' => Some(b"\\u2028"),
                '\u{2029}' => Some(b"\\u2029"),
                _ => None,
            };
            let Some(replacement) = replacement else {
                continue;
            };
            writer.write_all(&fragment.as_bytes()[start..index])?;
            writer.write_all(replacement)?;
            start = index + character.len_utf8();
        }
        writer.write_all(&fragment.as_bytes()[start..])
    }
}

pub(crate) fn to_script_safe_json<T>(value: &T) -> Result<Bytes, serde_json::Error>
where
    T: Serialize + ?Sized,
{
    let mut output = Vec::with_capacity(1024);
    let mut serializer = Serializer::with_formatter(&mut output, ScriptSafeFormatter);
    value.serialize(&mut serializer)?;
    Ok(Bytes::from(output))
}
use bytes::Bytes;
