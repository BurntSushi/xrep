use std::error;
use std::io::{self, BufRead, Read};
use std::fmt;

use byteorder::{LittleEndian, ReadBytesExt};
use crc::crc32::checksum_ieee;
use flate2::bufread::DeflateDecoder;

mod cp437;

#[derive(Debug)]
pub enum Action {
    Continue,
    Stop,
}

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    UnknownRecord(u32),
    NewerVersionNeeded(u16),
    Encrypted,
    Patched,
    UnknownMethod(u16),
    UnknownDataSize,
    DataDescriptorMismatch,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IoError(ref err) => fmt::Display::fmt(err, f),
            Error::UnknownRecord(sig) =>
                write!(f, "Unknown record type {:#08x}", sig),
            Error::NewerVersionNeeded(ver) =>
                write!(f, "Version needed to extract {}.{}",
                       ver / 10, ver % 10),
            Error::Encrypted =>
                write!(f, "File is encrypted"),
            Error::Patched =>
                write!(f, "File is patched"),
            Error::UnknownMethod(met) =>
                write!(f, "Unknown compression method {}", met),
            Error::UnknownDataSize =>
                write!(f, "Unknown compressed data size"),
            Error::DataDescriptorMismatch =>
                write!(f, "Data descriptor mismatch"),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::IoError(ref err) => err.description(),
            Error::UnknownRecord(..) => "unknown record type",
            Error::NewerVersionNeeded(..) => "newer version needed",
            Error::Encrypted => "file is encrypted",
            Error::Patched => "file is patched",
            Error::UnknownMethod(..) => "unknown compression method",
            Error::UnknownDataSize => "unknown compressed data size",
            Error::DataDescriptorMismatch => "data descriptor mismatch",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::IoError(ref err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

pub fn for_each_entry<R: BufRead, F>(mut zip: R, mut f: F) -> Result<(), Error>
        where F: FnMut(&str, Result<&mut Read, Error>) -> io::Result<Action> {
    loop {
        match try!(zip.read_u32::<LittleEndian>()) {
            0x04034b50 => {}            // local file header
            0x02014b50 | 0x06054b50 |   // central directory
            0x06064b50 | 0x07064b50 |   // zip64 central directory
            0x08064b50 => break,        // extra data
            sig => return Err(Error::UnknownRecord(sig)),
        }

        let version = try!(zip.read_u16::<LittleEndian>());
        let flags = try!(zip.read_u16::<LittleEndian>());
        let method = try!(zip.read_u16::<LittleEndian>());
        let _time = try!(zip.read_u16::<LittleEndian>());
        let _date = try!(zip.read_u16::<LittleEndian>());
        let mut _crc32 = try!(zip.read_u32::<LittleEndian>());
        let mut compressed = try!(zip.read_u32::<LittleEndian>()).into();
        let mut _uncompressed = try!(zip.read_u32::<LittleEndian>()).into();
        let name_len = try!(zip.read_u16::<LittleEndian>());
        let extra_len = try!(zip.read_u16::<LittleEndian>());

        let mut name_buf = vec![0; name_len as usize];
        try!(zip.read_exact(&mut name_buf));

        let mut extra_buf = vec![0; extra_len as usize];
        try!(zip.read_exact(&mut extra_buf));

        let mut extra = Default::default();
        match parse_extra(&extra_buf, &name_buf, &mut extra) {
            Ok(()) => {}
            Err(ref err) if err.kind() == io::ErrorKind::UnexpectedEof => {}
            Err(err) => return Err(err.into()),
        }

        if let Some(ref zip64) = extra.zip64 {
            if _uncompressed == u32::max_value().into() {
                _uncompressed = zip64.uncompressed;
            }
            if compressed == u32::max_value().into() {
                compressed = zip64.compressed;
            }
        }

        let name = if flags & (1 << 11) != 0 {
            String::from_utf8_lossy(&name_buf)
        } else if let Some(unicode_path) = extra.unicode_path {
            String::from_utf8_lossy(unicode_path)
        } else {
            name_buf.into_iter().map(cp437::to_char).collect::<String>().into()
        };

        let method = if version > 45 {
            Err(Error::NewerVersionNeeded(version))
        } else if flags & (1 << 0) != 0 {
            Err(Error::Encrypted)
        } else if flags & (1 << 5) != 0 {
            Err(Error::Patched)
        } else if method == 0 {
            Ok(Method::Store)
        } else if method == 8 {
            Ok(Method::Deflate)
        } else {
            Err(Error::UnknownMethod(method))
        };

        if flags & (1 << 3) != 0 {
            let mut data = zip.by_ref().take(u64::max_value());

            let action = match method {
                Err(err) => {
                    try!(f(&name, Err(err)));
                    return Err(Error::UnknownDataSize);
                }
                Ok(Method::Store) => {
                    try!(f(&name, Err(Error::UnknownDataSize)));
                    return Err(Error::UnknownDataSize);
                }
                Ok(Method::Deflate) => {
                    let mut decoder = DeflateDecoder::new(data.by_ref());
                    let action = try!(f(&name, Ok(decoder.by_ref())));
                    try!(drain(io::BufReader::new(decoder)));
                    action
                }
            };
            if let Action::Stop = action { break; }

            let total = u64::max_value() - data.limit();

            let sig = try!(data.read_u32::<LittleEndian>());
            _crc32 = if sig == 0x08074b50 {
                try!(data.read_u32::<LittleEndian>())
            } else { sig };
            if extra.zip64.is_none() {
                compressed = try!(data.read_u32::<LittleEndian>()).into();
                _uncompressed = try!(data.read_u32::<LittleEndian>()).into();
            } else {
                compressed = try!(data.read_u64::<LittleEndian>());
                _uncompressed = try!(data.read_u64::<LittleEndian>());
            }

            if total != compressed {
                return Err(Error::DataDescriptorMismatch)
            }
        } else {
            let mut data = zip.by_ref().take(compressed);

            let action = try!(match method {
                Err(err) => f(&name, Err(err)),
                Ok(Method::Store) => f(&name, Ok(data.by_ref())),
                Ok(Method::Deflate) =>
                    f(&name, Ok(DeflateDecoder::new(data.by_ref()).by_ref())),
            });
            if let Action::Stop = action { break; }

            try!(drain(data));
        }
    }
    Ok(())
}

#[derive(Debug)]
enum Method {
    Store,
    Deflate,
}

#[derive(Debug)]
struct Zip64Extra {
    uncompressed: u64,
    compressed: u64,
}

#[derive(Debug, Default)]
struct Extra<'a> {
    unicode_path: Option<&'a [u8]>,
    zip64: Option<Zip64Extra>,
}

fn parse_extra<'a>(mut buf: &'a [u8], name_buf: &[u8], extra: &mut Extra<'a>)
        -> io::Result<()> {
    while !buf.is_empty() {
        let tag = try!(buf.read_u16::<LittleEndian>());
        let size = try!(buf.read_u16::<LittleEndian>()).into();
        let mut data = buf.by_ref().take(size);
        match tag {
            0x0001 => {
                let uncompressed = try!(data.read_u64::<LittleEndian>());
                let compressed = try!(data.read_u64::<LittleEndian>());
                extra.zip64 = Some(Zip64Extra {
                    uncompressed: uncompressed,
                    compressed: compressed,
                });
            }
            0x7075 => {
                let version = try!(data.read_u8());
                if version == 1 {
                    let crc32 = try!(data.read_u32::<LittleEndian>());
                    if crc32 == checksum_ieee(name_buf) {
                        extra.unicode_path = Some(
                            &buf[..data.limit() as usize]);
                    }
                }
            }
            _ => {}
        }
        try!(drain(data));
    }
    Ok(())
}

fn drain<R: BufRead>(mut r: R) -> io::Result<()> {
    loop {
        let len = try!(r.fill_buf()).len();
        if len == 0 { break; }
        r.consume(len);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::parse_extra;

    #[test]
    fn zip() {
        for_each_entry(&include_bytes!("tests/hello.zip")[..], |name, rdr| {
            assert_eq!(name, "hello.txt");

            let mut data = Vec::new();
            rdr.unwrap().read_to_end(&mut data).unwrap();
            assert_eq!(data, b"Hello, world!");

            Ok(Action::Continue)
        }).unwrap();
    }

    #[test]
    fn zip_stop() {
        for_each_entry(&include_bytes!("tests/hello.zip")[..], |name, rdr| {
            assert_eq!(name, "hello.txt");

            let mut data = Vec::new();
            rdr.unwrap().read_to_end(&mut data).unwrap();
            assert_eq!(data, b"Hello, world!");

            Ok(Action::Stop)
        }).unwrap();
    }

    #[test]
    fn zip_drain() {
        for_each_entry(&include_bytes!("tests/hello.zip")[..], |name, rdr| {
            assert_eq!(name, "hello.txt");
            assert!(rdr.is_ok());
            Ok(Action::Continue)
        }).unwrap();
    }

    #[test]
    fn zip_drain_stop() {
        for_each_entry(&include_bytes!("tests/hello.zip")[..], |name, rdr| {
            assert_eq!(name, "hello.txt");
            assert!(rdr.is_ok());
            Ok(Action::Stop)
        }).unwrap();
    }

    #[test]
    fn stream_zip() {
        for_each_entry(&include_bytes!("tests/stream.zip")[..], |name, rdr| {
            assert_eq!(name, "-");

            let mut data = Vec::new();
            rdr.unwrap().read_to_end(&mut data).unwrap();
            assert_eq!(data, b"Hello, world!");

            Ok(Action::Continue)
        }).unwrap();
    }

    #[test]
    fn stream_zip_stop() {
        for_each_entry(&include_bytes!("tests/stream.zip")[..], |name, rdr| {
            assert_eq!(name, "-");

            let mut data = Vec::new();
            rdr.unwrap().read_to_end(&mut data).unwrap();
            assert_eq!(data, b"Hello, world!");

            Ok(Action::Stop)
        }).unwrap();
    }

    #[test]
    fn stream_zip_drain() {
        for_each_entry(&include_bytes!("tests/stream.zip")[..], |name, rdr| {
            assert_eq!(name, "-");
            assert!(rdr.is_ok());
            Ok(Action::Continue)
        }).unwrap();
    }

    #[test]
    fn stream_zip_drain_stop() {
        for_each_entry(&include_bytes!("tests/stream.zip")[..], |name, rdr| {
            assert_eq!(name, "-");
            assert!(rdr.is_ok());
            Ok(Action::Stop)
        }).unwrap();
    }

    #[test]
    fn extra_empty() {
        let mut extra = Default::default();
        parse_extra(b"", b"file.txt", &mut extra).unwrap();
        assert!(extra.zip64.is_none());
        assert!(extra.unicode_path.is_none());
    }

    #[test]
    fn extra_unknown() {
        const DATA: &'static [u8] = &[
            0x34, 0x12, 0x04, 0x00, 0xaa, 0xbb, 0xcc, 0xdd,
            0x78, 0x56, 0x02, 0x00, 0xee, 0xff,
        ];
        let mut extra = Default::default();
        parse_extra(DATA, b"file.txt", &mut extra).unwrap();
        assert!(extra.zip64.is_none());
        assert!(extra.unicode_path.is_none());
    }

    #[test]
    fn extra_zip64_short() {
        const DATA: &'static [u8] = &[
            0x01, 0x00, 0x10, 0x00,
            0x21, 0x43, 0x65, 0x87, 0x09, 0x00, 0x00, 0x00,
            0x89, 0x67, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00,
        ];
        let mut extra = Default::default();
        parse_extra(DATA, b"file.txt", &mut extra).unwrap();

        let zip64 = extra.zip64.unwrap();
        assert_eq!(zip64.uncompressed, 0x987654321);
        assert_eq!(zip64.compressed, 0x123456789);
        assert!(extra.unicode_path.is_none());
    }

    #[test]
    fn extra_zip64_long() {
        const DATA: &'static [u8] = &[
            0x01, 0x00, 0x1c, 0x00,
            0x21, 0x43, 0x65, 0x87, 0x09, 0x00, 0x00, 0x00,
            0x89, 0x67, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00,
            0xef, 0xcd, 0xab, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x00,
        ];
        let mut extra = Default::default();
        parse_extra(DATA, b"file.txt", &mut extra).unwrap();

        let zip64 = extra.zip64.unwrap();
        assert_eq!(zip64.uncompressed, 0x987654321);
        assert_eq!(zip64.compressed, 0x123456789);
        assert!(extra.unicode_path.is_none());
    }

    #[test]
    fn extra_unicode_path() {
        const DATA: &'static [u8] = &[
            0x75, 0x70, 0x0d, 0x00,
            0x01, 0x25, 0x16, 0xf7, 0xe0,
            0x61, 0x73, 0x64, 0x66, 0x2e, 0x74, 0x78, 0x74,
        ];
        let mut extra = Default::default();
        parse_extra(DATA, b"file.txt", &mut extra).unwrap();
        assert!(extra.zip64.is_none());
        assert_eq!(extra.unicode_path.unwrap(), b"asdf.txt");
    }

    #[test]
    fn extra_unicode_path_wrong_version() {
        const DATA: &'static [u8] = &[
            0x75, 0x70, 0x0d, 0x00,
            0x02, 0x25, 0x16, 0xf7, 0xe0,
            0x61, 0x73, 0x64, 0x66, 0x2e, 0x74, 0x78, 0x74,
        ];
        let mut extra = Default::default();
        parse_extra(DATA, b"file.txt", &mut extra).unwrap();
        assert!(extra.zip64.is_none());
        assert!(extra.unicode_path.is_none());
    }

    #[test]
    fn extra_unicode_path_wrong_crc32() {
        const DATA: &'static [u8] = &[
            0x75, 0x70, 0x0d, 0x00,
            0x01, 0x25, 0x16, 0xf7, 0xe1,
            0x61, 0x73, 0x64, 0x66, 0x2e, 0x74, 0x78, 0x74,
        ];
        let mut extra = Default::default();
        parse_extra(DATA, b"file.txt", &mut extra).unwrap();
        assert!(extra.zip64.is_none());
        assert!(extra.unicode_path.is_none());
    }

    #[test]
    fn extra_both() {
        const DATA: &'static [u8] = &[
            0x01, 0x00, 0x10, 0x00,
            0x21, 0x43, 0x65, 0x87, 0x09, 0x00, 0x00, 0x00,
            0x89, 0x67, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00,
            0x75, 0x70, 0x0d, 0x00,
            0x01, 0x25, 0x16, 0xf7, 0xe0,
            0x61, 0x73, 0x64, 0x66, 0x2e, 0x74, 0x78, 0x74,
        ];
        let mut extra = Default::default();
        parse_extra(DATA, b"file.txt", &mut extra).unwrap();

        let zip64 = extra.zip64.unwrap();
        assert_eq!(zip64.uncompressed, 0x987654321);
        assert_eq!(zip64.compressed, 0x123456789);
        assert_eq!(extra.unicode_path.unwrap(), b"asdf.txt");
    }

    #[test]
    fn extra_both_reversed() {
        const DATA: &'static [u8] = &[
            0x75, 0x70, 0x0d, 0x00,
            0x01, 0x25, 0x16, 0xf7, 0xe0,
            0x61, 0x73, 0x64, 0x66, 0x2e, 0x74, 0x78, 0x74,
            0x01, 0x00, 0x10, 0x00,
            0x21, 0x43, 0x65, 0x87, 0x09, 0x00, 0x00, 0x00,
            0x89, 0x67, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00,
        ];
        let mut extra = Default::default();
        parse_extra(DATA, b"file.txt", &mut extra).unwrap();

        let zip64 = extra.zip64.unwrap();
        assert_eq!(zip64.uncompressed, 0x987654321);
        assert_eq!(zip64.compressed, 0x123456789);
        assert_eq!(extra.unicode_path.unwrap(), b"asdf.txt");
    }
}
