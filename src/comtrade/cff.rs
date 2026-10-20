//! Splitter for the COMTRADE 2013 combined file format (`.cff`).
//!
//! A `.cff` file contains the sections of a recording, each one introduced
//! by a marker line such as `--- file type: CFG ---` or
//! `--- file type: DAT BINARY: 1234 ---` (binary data with its byte count).

use crate::error::{Error, Result};

/// Sections of a `.cff` file, borrowed from the file content.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Sections<'a> {
    /// Configuration section.
    pub cfg: Option<&'a [u8]>,
    /// Information section.
    pub inf: Option<&'a [u8]>,
    /// Header section.
    pub hdr: Option<&'a [u8]>,
    /// Data section.
    pub dat: Option<&'a [u8]>,
}

enum Kind {
    Cfg,
    Inf,
    Hdr,
    Dat,
}

/// Splits `data` into its sections.
pub fn split(data: &[u8]) -> Result<Sections<'_>> {
    let mut out = Sections::default();
    let mut current: Option<(Kind, usize)> = None;
    let mut pos = 0;

    while pos < data.len() {
        let end = data[pos..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(data.len(), |i| pos + i + 1);
        let line = String::from_utf8_lossy(&data[pos..end]);
        let Some(marker) = parse_marker(&line)? else {
            pos = end;
            continue;
        };
        if let Some((kind, start)) = current.take() {
            store(&mut out, kind, &data[start..pos]);
        }
        match marker {
            Marker::Text(kind) => current = Some((kind, end)),
            Marker::BinaryDat(len) => {
                let stop = end.checked_add(len).filter(|&s| s <= data.len()).ok_or_else(|| {
                    Error::Unsupported(format!(
                        "cff binary data section declares {len} bytes, file is too short"
                    ))
                })?;
                out.dat = Some(&data[end..stop]);
                pos = stop;
                continue;
            }
        }
        pos = end;
    }
    if let Some((kind, start)) = current {
        store(&mut out, kind, &data[start..]);
    }
    if out.cfg.is_none() {
        return Err(Error::MissingFile("CFG section in .cff file".into()));
    }
    Ok(out)
}

enum Marker {
    Text(Kind),
    BinaryDat(usize),
}

fn parse_marker(line: &str) -> Result<Option<Marker>> {
    let t = line.trim();
    let Some(inner) = t.strip_prefix("---").and_then(|s| s.strip_suffix("---")) else {
        return Ok(None);
    };
    let inner = inner.trim();
    let Some(rest) = inner
        .get(..10)
        .filter(|p| p.eq_ignore_ascii_case("file type:"))
        .map(|_| inner[10..].trim())
    else {
        return Ok(None);
    };
    let upper = rest.to_ascii_uppercase();
    let marker = match upper.split_whitespace().next() {
        Some("CFG") => Marker::Text(Kind::Cfg),
        Some("INF") => Marker::Text(Kind::Inf),
        Some("HDR") => Marker::Text(Kind::Hdr),
        Some("DAT") if upper.contains("ASCII") => Marker::Text(Kind::Dat),
        Some("DAT") => {
            let len = upper
                .rsplit(':')
                .next()
                .and_then(|n| n.trim().parse().ok())
                .ok_or_else(|| Error::Unsupported(format!("cff data section without byte count: '{t}'")))?;
            Marker::BinaryDat(len)
        }
        _ => return Ok(None),
    };
    Ok(Some(marker))
}

fn store<'a>(out: &mut Sections<'a>, kind: Kind, bytes: &'a [u8]) {
    let slot = match kind {
        Kind::Cfg => &mut out.cfg,
        Kind::Inf => &mut out.inf,
        Kind::Hdr => &mut out.hdr,
        Kind::Dat => &mut out.dat,
    };
    *slot = Some(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_text_and_binary_sections() {
        let mut data = b"--- file type: CFG ---\r\nline1\r\n--- file type: HDR ---\r\nhello\r\n\
--- file type: DAT BINARY: 4 ---\r\n"
            .to_vec();
        data.extend([0xFF, b'\n', 0, 1]);
        data.extend(b"--- file type: INF ---\nx=1\n");
        let s = split(&data).unwrap();
        assert_eq!(s.cfg, Some(&b"line1\r\n"[..]));
        assert_eq!(s.hdr, Some(&b"hello\r\n"[..]));
        assert_eq!(s.dat, Some(&[0xFF, b'\n', 0, 1][..]));
        assert_eq!(s.inf, Some(&b"x=1\n"[..]));
    }

    #[test]
    fn ascii_dat_and_missing_cfg() {
        let s = split(b"--- file type: CFG ---\na\n--- file type: DAT ASCII ---\n1,0,5\n").unwrap();
        assert_eq!(s.dat, Some(&b"1,0,5\n"[..]));
        assert!(split(b"--- file type: HDR ---\nx\n").is_err());
    }
}
