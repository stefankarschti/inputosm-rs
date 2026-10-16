#![allow(dead_code)]
use inputosm::{Error, Result, Tag};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
};

pub fn directory(path: &str) -> Result<()> {
    fs::create_dir(path)?;
    Ok(())
}
pub fn output(directory: &str, name: &str) -> Result<BufWriter<File>> {
    Ok(BufWriter::with_capacity(
        1024 * 1024,
        File::create_new(Path::new(directory).join(name))?,
    ))
}
pub fn finish(output: &mut BufWriter<File>) -> Result<()> {
    output.flush()?;
    output.get_ref().sync_all()?;
    Ok(())
}
pub fn count(output: &mut impl Write, count: usize) -> Result<()> {
    let count = i16::try_from(count)
        .map_err(|_| Error::Invalid("The binary format permits at most 32767 values"))?;
    output.write_all(&count.to_le_bytes())?;
    Ok(())
}
pub fn terminated(output: &mut impl Write, text: &[u8]) -> Result<()> {
    if text.contains(&0) {
        return Err(Error::Invalid(
            "The binary format cannot store embedded NUL bytes",
        ));
    }
    output.write_all(text)?;
    output.write_all(&[0])?;
    Ok(())
}
pub fn binary_tags(output: &mut impl Write, tags: &[Tag<'_>]) -> Result<()> {
    count(output, tags.len())?;
    for tag in tags {
        terminated(output, tag.key)?;
        terminated(output, tag.value)?;
    }
    Ok(())
}
pub fn csv_text(output: &mut impl Write, text: &[u8]) -> Result<()> {
    output.write_all(text)?;
    Ok(())
}
pub fn csv_tags(output: &mut impl Write, tags: &[Tag<'_>]) -> Result<()> {
    output.write_all(b"'")?;
    for (i, tag) in tags.iter().enumerate() {
        if i > 0 {
            output.write_all(b",")?;
        }
        output.write_all(b"\"")?;
        csv_text(output, tag.key)?;
        output.write_all(b"\"=>\"")?;
        csv_text(output, tag.value)?;
        output.write_all(b"\"")?;
    }
    output.write_all(b"'")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_unrepresentable_data_and_report_write_errors() {
        let mut bytes = Vec::new();
        assert!(count(&mut bytes, 32768).is_err());
        assert!(terminated(&mut bytes, b"a\0b").is_err());
        let mut full = &mut [0u8; 0][..];
        assert!(count(&mut full, 1).is_err());
        count(&mut bytes, 32767).unwrap();
        assert_eq!(bytes, [255, 127]);
    }
}
