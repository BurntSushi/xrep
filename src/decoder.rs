use std::cmp;
use std::io::{self, Read};

use encoding_rs::{Decoder, Encoding, UTF_8};

/// A BOM is at least 2 bytes and at most 3 bytes.
///
/// If fewer than 2 bytes are available to be read at the beginning of a
/// reader, then a BOM is `None`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Bom {
    bytes: [u8; 3],
    len: usize,
}

impl Bom {
    fn as_slice(&self) -> &[u8] {
        &self.bytes[0..self.len]
    }

    fn decoder(&self) -> Option<Decoder> {
        let bom = self.as_slice();
        if bom.len() < 3 {
            return None;
        }
        if let Some((enc, _)) = Encoding::for_bom(bom) {
            if enc != UTF_8 {
                return Some(enc.new_decoder_with_bom_removal());
            }
        }
        None
    }
}

/// `BomPeeker` wraps `R` and satisfies the `io::Read` interface while also
/// providing a peek at the BOM if one exists. Peeking at the BOM does not
/// advance the reader.
struct BomPeeker<R> {
    rdr: R,
    bom: Option<Bom>,
    nread: usize,
}

impl<R: io::Read> BomPeeker<R> {
    /// Create a new BomPeeker.
    ///
    /// The first three bytes can be read using the `peek_bom` method, but
    /// will not advance the reader.
    fn new(rdr: R) -> BomPeeker<R> {
        BomPeeker { rdr: rdr, bom: None, nread: 0 }
    }

    /// Peek at the first three bytes of the underlying reader.
    ///
    /// This does not advance the reader provided by `BomPeeker`.
    ///
    /// If the underlying reader does not have at least two bytes available,
    /// then `None` is returned.
    fn peek_bom(&mut self) -> io::Result<Bom> {
        if let Some(bom) = self.bom {
            return Ok(bom);
        }
        self.bom = Some(Bom { bytes: [0; 3], len: 0 });
        let mut buf = [0u8; 3];
        let bom_len = try!(read_full(&mut self.rdr, &mut buf));
        self.bom = Some(Bom { bytes: buf, len: bom_len });
        Ok(self.bom.unwrap())
    }
}

impl<R: io::Read> io::Read for BomPeeker<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.nread < 3 {
            let bom = try!(self.peek_bom());
            let bom = bom.as_slice();
            if self.nread < bom.len() {
                let rest = &bom[self.nread..];
                let len = cmp::min(buf.len(), rest.len());
                buf[..len].copy_from_slice(&rest[..len]);
                self.nread += len;
                return Ok(len);
            }
        }
        let nread = try!(self.rdr.read(buf));
        self.nread += nread;
        Ok(nread)
    }
}

/// Like `io::Read::read_exact`, except it never returns `UnexpectedEof` and
/// instead returns the number of bytes read if EOF is seen before filling
/// `buf`.
fn read_full<R: io::Read>(
    mut rdr: R,
    mut buf: &mut [u8],
) -> io::Result<usize> {
    let mut nread = 0;
    while !buf.is_empty() {
        match rdr.read(buf) {
            Ok(0) => break,
            Ok(n) => {
                nread += n;
                let tmp = buf;
                buf = &mut tmp[n..];
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(nread)
}

/// A reader that transcodes to UTF-8. The source encoding is determined by
/// inspecting the BOM from the stream read from `R`, if one exists. If a
/// UTF-16 BOM exists, then the source stream is transcoded to UTF-8 with
/// invalid UTF-16 sequences translated to the Unicode replacement character.
/// In all other cases, the underlying reader is passed through unchanged.
///
/// `R` is the type of the underlying reader and `B` is the type of an internal
/// buffer used to store the results of transcoding.
///
/// Note that not all methods on `io::Read` work with this implementation.
/// For example, the `bytes` adapter method attempts to read a single byte at
/// a time, but this implementation requires a buffer of size at least `4`. If
/// a buffer of size less than 4 is given, then an error is returned.
pub struct DecodeReader<R, B> {
    /// The underlying reader, wrapped in a peeker for reading a BOM if one
    /// exists.
    rdr: BomPeeker<R>,
    /// The internal buffer to store transcoded bytes before they are read by
    /// callers.
    buf: B,
    /// The current position in `buf`. Subsequent reads start here.
    pos: usize,
    /// The number of transcoded bytes in `buf`. Subsequent reads end here.
    buflen: usize,
    /// Whether this is the first read or not (in which we inspect the BOM).
    first: bool,
    /// Whether a "last" read has occurred. After this point, EOF will always
    /// be returned.
    last: bool,
    /// The underlying text decoder derived from the BOM, if one exists.
    decoder: Option<Decoder>,
}

impl<R: io::Read, B: AsMut<[u8]>> DecodeReader<R, B> {
    /// Create a new transcoder that converts a source stream to valid UTF-8.
    ///
    /// If an encoding is specified, then it is used to transcode `rdr` to
    /// UTF-8. Otherwise, if no encoding is specified, and if a UTF-16 BOM is
    /// found, then the corresponding UTF-16 encoding is used to transcode
    /// `rdr` to UTF-8. In all other cases, `rdr` is assumed to be at least
    /// ASCII-compatible and passed through untouched.
    ///
    /// Errors in the encoding of `rdr` are handled with the Unicode
    /// replacement character. If no encoding of `rdr` is specified, then
    /// errors are not handled.
    pub fn new(
        rdr: R,
        buf: B,
        enc: Option<&'static Encoding>,
    ) -> DecodeReader<R, B> {
        DecodeReader {
            rdr: BomPeeker::new(rdr),
            buf: buf,
            buflen: 0,
            pos: 0,
            first: enc.is_none(),
            last: false,
            decoder: enc.map(|enc| enc.new_decoder_with_bom_removal()),
        }
    }

    /// Fill the internal buffer from the underlying reader.
    ///
    /// If there are unread bytes in the internal buffer, then we move them
    /// to the beginning of the internal buffer and fill the remainder.
    ///
    /// If the internal buffer is too small to read additional bytes, then an
    /// error is returned.
    #[inline(always)] // massive perf benefit (???)
    fn fill(&mut self) -> io::Result<()> {
        if self.pos < self.buflen {
            if self.buflen >= self.buf.as_mut().len() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "DecodeReader: internal buffer exhausted"));
            }
            let newlen = self.buflen - self.pos;
            let mut tmp = Vec::with_capacity(newlen);
            tmp.extend_from_slice(&self.buf.as_mut()[self.pos..self.buflen]);
            self.buf.as_mut()[..newlen].copy_from_slice(&tmp);
            self.buflen = newlen;
        } else {
            self.buflen = 0;
        }
        self.pos = 0;
        self.buflen +=
            try!(self.rdr.read(&mut self.buf.as_mut()[self.buflen..]));
        Ok(())
    }

    /// Transcode the inner stream to UTF-8 in `buf`. This assumes that there
    /// is a decoder capable of transcoding the inner stream to UTF-8. This
    /// returns the number of bytes written to `buf`.
    ///
    /// When this function returns, exactly one of the following things will
    /// be true:
    ///
    /// 1. A non-zero number of bytes were written to `buf`.
    /// 2. The underlying reader reached EOF.
    /// 3. An error is returned: the internal buffer ran out of room.
    /// 4. An I/O error occurred.
    ///
    /// Note that `buf` must have at least 4 bytes of space.
    fn transcode(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        assert!(buf.len() >= 4);
        if self.last {
            return Ok(0);
        }
        if self.pos >= self.buflen {
            try!(self.fill());
        }
        let mut nwrite = 0;
        loop {
            let (_, nin, nout, _) =
                self.decoder.as_mut().unwrap().decode_to_utf8(
                    &self.buf.as_mut()[self.pos..self.buflen], buf, false);
            self.pos += nin;
            nwrite += nout;
            // If we've written at least one byte to the caller-provided
            // buffer, then our mission is complete.
            if nwrite > 0 {
                break;
            }
            // Otherwise, we know that our internal buffer has insufficient
            // data to transcode at least one char, so we attempt to refill it.
            try!(self.fill());
            // Quit on EOF.
            if self.buflen == 0 {
                self.pos = 0;
                self.last = true;
                let (_, _, nout, _) =
                    self.decoder.as_mut().unwrap().decode_to_utf8(
                        &[], buf, true);
                return Ok(nout);
            }
        }
        Ok(nwrite)
    }

    #[inline(never)] // impacts perf...
    fn detect(&mut self) -> io::Result<()> {
        let bom = try!(self.rdr.peek_bom());
        self.decoder = bom.decoder();
        Ok(())
    }
}

impl<R: io::Read, B: AsMut<[u8]>> io::Read for DecodeReader<R, B> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.first {
            self.first = false;
            try!(self.detect());
        }
        if self.decoder.is_none() {
            return self.rdr.read(buf);
        }
        // When decoding UTF-8, we need at least 4 bytes of space to guarantee
        // that we can decode at least one codepoint. If we don't have it, we
        // can either return `0` for the number of bytes read or return an
        // error. Since `0` would be interpreted as a possibly premature EOF,
        // we opt for an error.
        if buf.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "DecodeReader: byte buffer must have length at least 4"));
        }
        self.transcode(buf)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use encoding_rs::Encoding;

    use super::{Bom, BomPeeker, DecodeReader};

    fn read_to_string<R: Read>(mut rdr: R) -> String {
        let mut s = String::new();
        rdr.read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn peeker_empty() {
        let buf = [];
        let mut peeker = BomPeeker::new(&buf[..]);
        assert_eq!(Bom { bytes: [0; 3], len: 0}, peeker.peek_bom().unwrap());

        let mut tmp = [0; 100];
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_one() {
        let buf = [1];
        let mut peeker = BomPeeker::new(&buf[..]);
        assert_eq!(
            Bom { bytes: [1, 0, 0], len: 1},
            peeker.peek_bom().unwrap());

        let mut tmp = [0; 100];
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_two() {
        let buf = [1, 2];
        let mut peeker = BomPeeker::new(&buf[..]);
        assert_eq!(
            Bom { bytes: [1, 2, 0], len: 2},
            peeker.peek_bom().unwrap());

        let mut tmp = [0; 100];
        assert_eq!(2, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(2, tmp[1]);
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_three() {
        let buf = [1, 2, 3];
        let mut peeker = BomPeeker::new(&buf[..]);
        assert_eq!(
            Bom { bytes: [1, 2, 3], len: 3},
            peeker.peek_bom().unwrap());

        let mut tmp = [0; 100];
        assert_eq!(3, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(2, tmp[1]);
        assert_eq!(3, tmp[2]);
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_four() {
        let buf = [1, 2, 3, 4];
        let mut peeker = BomPeeker::new(&buf[..]);
        assert_eq!(
            Bom { bytes: [1, 2, 3], len: 3},
            peeker.peek_bom().unwrap());

        let mut tmp = [0; 100];
        assert_eq!(3, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(2, tmp[1]);
        assert_eq!(3, tmp[2]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(4, tmp[0]);
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_one_at_a_time() {
        let buf = [1, 2, 3, 4];
        let mut peeker = BomPeeker::new(&buf[..]);

        let mut tmp = [0; 1];
        assert_eq!(0, peeker.read(&mut tmp[..0]).unwrap());
        assert_eq!(0, tmp[0]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(2, tmp[0]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(3, tmp[0]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(4, tmp[0]);
    }

    // In cases where all we have is a bom, we expect the bytes to be
    // passed through unchanged.
    #[test]
    fn trans_utf16_bom() {
        let srcbuf = vec![0xFF, 0xFE];
        let mut dstbuf = vec![0; 8 * (1<<10)];
        let mut rdr = DecodeReader::new(&*srcbuf, vec![0; 8 * (1<<10)], None);
        let n = rdr.read(&mut dstbuf).unwrap();
        assert_eq!(&*srcbuf, &dstbuf[..n]);

        let srcbuf = vec![0xFE, 0xFF];
        let mut rdr = DecodeReader::new(&*srcbuf, vec![0; 8 * (1<<10)], None);
        let n = rdr.read(&mut dstbuf).unwrap();
        assert_eq!(&*srcbuf, &dstbuf[..n]);

        let srcbuf = vec![0xEF, 0xBB, 0xBF];
        let mut rdr = DecodeReader::new(&*srcbuf, vec![0; 8 * (1<<10)], None);
        let n = rdr.read(&mut dstbuf).unwrap();
        assert_eq!(&*srcbuf, &dstbuf[..n]);
    }

    // Test basic UTF-16 decoding.
    #[test]
    fn trans_utf16_basic() {
        let srcbuf = vec![0xFF, 0xFE, 0x61, 0x00];
        let mut rdr = DecodeReader::new(&*srcbuf, vec![0; 8 * (1<<10)], None);
        assert_eq!("a", read_to_string(&mut rdr));

        let srcbuf = vec![0xFE, 0xFF, 0x00, 0x61];
        let mut rdr = DecodeReader::new(&*srcbuf, vec![0; 8 * (1<<10)], None);
        assert_eq!("a", read_to_string(&mut rdr));
    }

    // Test incomplete UTF-16 decoding. This ensures we see a replacement char
    // if the stream ends with an unpaired code unit.
    #[test]
    fn trans_utf16_incomplete() {
        let srcbuf = vec![0xFF, 0xFE, 0x61, 0x00, 0x00];
        let mut rdr = DecodeReader::new(&*srcbuf, vec![0; 8 * (1<<10)], None);
        assert_eq!("a\u{FFFD}", read_to_string(&mut rdr));
    }

    macro_rules! test_trans_simple {
        ($name:ident, $enc:expr, $srcbytes:expr, $dst:expr) => {
            #[test]
            fn $name() {
                let srcbuf = &$srcbytes[..];
                let enc = Encoding::for_label($enc.as_bytes());
                let mut rdr = DecodeReader::new(
                    &*srcbuf, vec![0; 8 * (1<<10)], enc);
                assert_eq!($dst, read_to_string(&mut rdr));
            }
        }
    }

    // This isn't exhaustive obviously, but it lets us test base level support.
    test_trans_simple!(trans_simple_auto, "does not exist", b"\xD0\x96", "Ж");
    test_trans_simple!(trans_simple_utf8, "utf-8", b"\xD0\x96", "Ж");
    test_trans_simple!(trans_simple_utf16le, "utf-16le", b"\x16\x04", "Ж");
    test_trans_simple!(trans_simple_utf16be, "utf-16be", b"\x04\x16", "Ж");
    test_trans_simple!(trans_simple_chinese, "chinese", b"\xA7\xA8", "Ж");
    test_trans_simple!(trans_simple_korean, "korean", b"\xAC\xA8", "Ж");
    test_trans_simple!(
        trans_simple_big5_hkscs, "big5-hkscs", b"\xC7\xFA", "Ж");
    test_trans_simple!(trans_simple_gbk, "gbk", b"\xA7\xA8", "Ж");
    test_trans_simple!(trans_simple_sjis, "sjis", b"\x84\x47", "Ж");
    test_trans_simple!(trans_simple_eucjp, "euc-jp", b"\xA7\xA8", "Ж");
    test_trans_simple!(trans_simple_latin1, "latin1", b"\xA9", "©");
}
