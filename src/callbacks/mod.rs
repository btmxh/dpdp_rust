use std::{
    fs::{create_dir_all, File},
    path::Path,
};

use serde::Serialize;

use serde_json::ser::{Formatter, PrettyFormatter};
use serde_json::{Serializer, Value};
use std::io::{Result as IoResult, Write};

pub mod log_dispatch;

pub fn dump_json<T>(path: impl AsRef<Path>, value: &T) -> anyhow::Result<()>
where
    T: ?Sized + Serialize,
{
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }

    serde_json::to_string_pretty(value);
    Ok(())
}

struct CompactArrayFormatter {
    inner: PrettyFormatter<'static>,
    max_inline_len: usize,
}

impl CompactArrayFormatter {
    fn new(max_inline_len: usize) -> Self {
        Self {
            inner: PrettyFormatter::with_indent(b"  "),
            max_inline_len,
        }
    }

    fn is_small_array(&self, value: &Value) -> bool {
        match value {
            Value::Array(arr) => {
                arr.len() <= self.max_inline_len
                    && arr
                        .iter()
                        .all(|v| !matches!(v, Value::Array(_) | Value::Object(_)))
            }
            _ => false,
        }
    }
}

impl Formatter for CompactArrayFormatter {
    fn begin_array<W: ?Sized + Write>(&mut self, writer: &mut W) -> IoResult<()> {
        self.inner.begin_array(writer)
    }

    fn end_array<W: ?Sized + Write>(&mut self, writer: &mut W) -> IoResult<()> {
        self.inner.end_array(writer)
    }

    fn begin_array_value<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> IoResult<()> {
        self.inner.begin_array_value(writer, first)
    }

    fn end_array_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> IoResult<()> {
        self.inner.end_array_value(writer)
    }

    fn write_array<W: ?Sized + Write>(&mut self, writer: &mut W, value: &[Value]) -> IoResult<()> {
        if value.len() <= self.max_inline_len
            && value
                .iter()
                .all(|v| !matches!(v, Value::Array(_) | Value::Object(_)))
        {
            write!(writer, "[")?;
            for (i, v) in value.iter().enumerate() {
                if i > 0 {
                    write!(writer, ", ")?;
                }
                write!(writer, "{}", v)?;
            }
            write!(writer, "]")
        } else {
            // fallback to default pretty array
            self.begin_array(writer)?;
            for (i, v) in value.iter().enumerate() {
                self.begin_array_value(writer, i == 0)?;
                v.serialize(&mut Serializer::with_formatter(writer, &mut self.inner))?;
                self.end_array_value(writer)?;
            }
            self.end_array(writer)
        }
    }

    // Delegate the rest to PrettyFormatter
    fn begin_object<W: ?Sized + Write>(&mut self, writer: &mut W) -> IoResult<()> {
        self.inner.begin_object(writer)
    }

    fn end_object<W: ?Sized + Write>(&mut self, writer: &mut W) -> IoResult<()> {
        self.inner.end_object(writer)
    }

    fn begin_object_key<W: ?Sized + Write>(&mut self, writer: &mut W, first: bool) -> IoResult<()> {
        self.inner.begin_object_key(writer, first)
    }

    fn end_object_key<W: ?Sized + Write>(&mut self, writer: &mut W) -> IoResult<()> {
        self.inner.end_object_key(writer)
    }

    fn begin_object_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> IoResult<()> {
        self.inner.begin_object_value(writer)
    }

    fn end_object_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> IoResult<()> {
        self.inner.end_object_value(writer)
    }

    fn write_raw_fragment<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> IoResult<()> {
        self.inner.write_raw_fragment(writer, fragment)
    }
}
