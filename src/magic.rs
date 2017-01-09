use std::cmp::min;
use std::io::{ErrorKind, Read, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Magic {
    Unknown,
    Gzip,
    Zip,
}

impl Magic {
    pub fn from_slice(buf: &[u8]) -> Magic {
        if buf.starts_with(b"\x1f\x8b") {
            Magic::Gzip
        } else if buf.starts_with(b"\x50\x4b\x03\x04") {
            // Local file header
            Magic::Zip
        } else if buf.starts_with(b"\x50\x4b\x05\x06") {
            // End of central directory (empty ZIP file)
            Magic::Zip
        } else {
            Magic::Unknown
        }
    }
}

/// Wrapper around `Read` which peeks the first few bytes for a magic number.
#[derive(Debug)]
pub struct MagicReader<R> {
    /// The inner reader.
    inner: R,
    /// Position of the first byte within `buf` which has not yet been returned
    /// by `read()`.
    pos: usize,
    /// Number of valid bytes within `buf`.
    lim: usize,
    /// Copy of the first few bytes obtained from the inner reader.
    buf: [u8; 4],
}

impl<R: Read> MagicReader<R> {
    /// Creates a new `MagicReader`, reading and buffering the magic number.
    pub fn new(mut inner: R) -> Result<MagicReader<R>> {
        let mut buf = [0; 4];
        let lim = try!(read_all(&mut inner, &mut buf));

        Ok(MagicReader {
            inner: inner,
            pos: 0,
            lim: lim,
            buf: buf,
        })
    }

    pub fn magic(&self) -> Magic {
        Magic::from_slice(&self.buf[..self.lim])
    }
}

impl<R: Read> Read for MagicReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.pos < self.lim {
            let count = min(self.lim - self.pos, buf.len());
            buf[..count].copy_from_slice(&self.buf[self.pos..self.pos + count]);
            self.pos += count;

            // Attempt to fill the whole buffer (is_binary depends on this).
            if count < buf.len() {
                match self.inner.read(&mut buf[count..]) {
                    Ok(read) => Ok(count + read),
                    Err(..) => Ok(count),
                }
            } else {
                Ok(count)
            }
        } else {
            self.inner.read(buf)
        }
    }
}

fn read_all<R: Read>(reader: &mut R, mut buf: &mut [u8]) -> Result<usize> {
    let mut read = 0;
    while !buf.is_empty() {
        match reader.read(buf) {
            Ok(0) => break,
            Ok(n) => {
                read += n;
                let tmp = buf;
                buf = &mut tmp[n..];
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(read)
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use super::{Magic, MagicReader};

    #[test]
    fn empty() {
        const DATA: &'static [u8] = &[];
        assert_eq!(Magic::from_slice(DATA), Magic::Unknown);

        let mut rdr = MagicReader::new(DATA).unwrap();
        assert_eq!(rdr.magic(), Magic::Unknown);

        let mut buf = vec![0; DATA.len()];
        assert_eq!(rdr.read(&mut buf).unwrap(), DATA.len());
        assert_eq!(&buf[..], DATA);
    }

    #[test]
    fn gzip() {
        const DATA: &'static [u8] = &[
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x03, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00
        ];
        assert_eq!(Magic::from_slice(DATA), Magic::Gzip);

        let mut rdr = MagicReader::new(DATA).unwrap();
        assert_eq!(rdr.magic(), Magic::Gzip);

        let mut buf = vec![0; DATA.len()];
        assert_eq!(rdr.read(&mut buf).unwrap(), DATA.len());
        assert_eq!(&buf[..], DATA);
    }

    #[test]
    fn zip() {
        const DATA: &'static [u8] = &[
            0x50, 0x4b, 0x03, 0x04, 0x0a, 0x00, 0x00, 0x00,
        ];
        assert_eq!(Magic::from_slice(DATA), Magic::Zip);

        let mut rdr = MagicReader::new(DATA).unwrap();
        assert_eq!(rdr.magic(), Magic::Zip);

        let mut buf = vec![0; DATA.len()];
        assert_eq!(rdr.read(&mut buf).unwrap(), DATA.len());
        assert_eq!(&buf[..], DATA);
    }

    #[test]
    fn zip_empty() {
        const DATA: &'static [u8] = &[
            0x50, 0x4b, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(Magic::from_slice(DATA), Magic::Zip);

        let mut rdr = MagicReader::new(DATA).unwrap();
        assert_eq!(rdr.magic(), Magic::Zip);

        let mut buf = vec![0; DATA.len()];
        assert_eq!(rdr.read(&mut buf).unwrap(), DATA.len());
        assert_eq!(&buf[..], DATA);
    }
}
