// This module defines the types we use for JSON serialization. We specifically
// omit deserialization, partially because there isn't a clear use case for
// them at this time, but also because deserialization will complicate things.
// Namely, the types below are designed in a way that permits JSON
// serialization with little or no allocation. Allocation is often quite
// convenient for deserialization however, so these types would become a bit
// more complex.

use std::borrow::Cow;
use std::path::Path;
use std::str;

use base64;
use serde::{Serialize, Serializer};

use stats::Stats;

#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum Message<'a> {
    Begin(Begin<'a>),
    End(End<'a>),
    Match(Match<'a>),
    Context(Context<'a>),
}

#[derive(Serialize)]
pub struct Begin<'a> {
    #[serde(serialize_with = "ser_path")]
    pub path: Option<&'a Path>,
}

#[derive(Serialize)]
pub struct End<'a> {
    #[serde(serialize_with = "ser_path")]
    pub path: Option<&'a Path>,
    pub binary_offset: Option<u64>,
    pub stats: Stats,
}

#[derive(Serialize)]
pub struct Match<'a> {
    #[serde(serialize_with = "ser_path")]
    pub path: Option<&'a Path>,
    #[serde(serialize_with = "ser_bytes")]
    pub lines: &'a [u8],
    pub line_number: Option<u64>,
    pub absolute_offset: u64,
    pub submatches: &'a [SubMatch<'a>],
}

#[derive(Serialize)]
pub struct Context<'a> {
    #[serde(serialize_with = "ser_path")]
    pub path: Option<&'a Path>,
    #[serde(serialize_with = "ser_bytes")]
    pub lines: &'a [u8],
    pub line_number: Option<u64>,
    pub absolute_offset: u64,
    pub submatches: &'a [SubMatch<'a>],
}

#[derive(Serialize)]
pub struct SubMatch<'a> {
    #[serde(rename = "match")]
    #[serde(serialize_with = "ser_bytes")]
    pub m: &'a [u8],
    pub start: usize,
    pub end: usize,
}

/// Data represents things that look like strings, but may actually not be
/// valid UTF-8. To handle this, `Data` is serialized as an object with one
/// of two keys: `text` (for valid UTF-8) or `bytes` (for invalid UTF-8).
///
/// The happy path is valid UTF-8, which streams right through as-is, since
/// it is natively supported by JSON. When invalid UTF-8 is found, then it is
/// represented as arbitrary bytes and base64 encoded.
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize)]
#[serde(untagged)]
enum Data<'a> {
    Text { text: Cow<'a, str> },
    Bytes {
        #[serde(serialize_with = "to_base64")]
        bytes: &'a [u8],
    },
}

impl<'a> Data<'a> {
    fn from_bytes(bytes: &[u8]) -> Data {
        match str::from_utf8(bytes) {
            Ok(text) => Data::Text { text: Cow::Borrowed(text) },
            Err(_) => Data::Bytes { bytes },
        }
    }

    #[cfg(unix)]
    fn from_path(path: &Path) -> Data {
        use std::os::unix::ffi::OsStrExt;

        match path.to_str() {
            Some(text) => Data::Text { text: Cow::Borrowed(text) },
            None => Data::Bytes { bytes: path.as_os_str().as_bytes() },
        }
    }

    #[cfg(not(unix))]
    fn from_path(path: &Path) -> Data {
        // Using lossy conversion means some paths won't round trip precisely,
        // but it's not clear what we should actually do. Serde rejects
        // non-UTF-8 paths, and OsStr's are serialized as a sequence of UTF-16
        // code units on Windows. Neither seem appropriate for this use case,
        // so we do the easy thing for now.
        Data::Text { text: path.to_string_lossy() }
    }

    // Unused deserialization routines.

    /*
    fn into_bytes(self) -> Vec<u8> {
        match self {
            Data::Text { text } => text.into_bytes(),
            Data::Bytes { bytes } => bytes,
        }
    }

    #[cfg(unix)]
    fn into_path_buf(&self) -> PathBuf {
        use std::os::unix::ffi::OsStrExt;

        match self {
            Data::Text { text } => PathBuf::from(text),
            Data::Bytes { bytes } => {
                PathBuf::from(OsStr::from_bytes(bytes))
            }
        }
    }

    #[cfg(not(unix))]
    fn into_path_buf(&self) -> PathBuf {
        match self {
            Data::Text { text } => PathBuf::from(text),
            Data::Bytes { bytes } => {
                PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
            }
        }
    }
    */
}

fn to_base64<T, S>(
    bytes: T,
    ser: S,
) -> Result<S::Ok, S::Error>
where T: AsRef<[u8]>,
      S: Serializer
{
    ser.serialize_str(&base64::encode(&bytes))
}

fn ser_bytes<T, S>(
    bytes: T,
    ser: S,
) -> Result<S::Ok, S::Error>
where T: AsRef<[u8]>,
      S: Serializer
{
    Data::from_bytes(bytes.as_ref()).serialize(ser)
}

fn ser_path<P, S>(
    path: &Option<P>,
    ser: S,
) -> Result<S::Ok, S::Error>
where P: AsRef<Path>,
      S: Serializer
{
    path.as_ref().map(|p| Data::from_path(p.as_ref())).serialize(ser)
}

// The following are some deserialization helpers, in case we decide to support
// deserialization of the above types.

/*
fn from_base64<'de, D>(
    de: D,
) -> Result<Vec<u8>, D::Error>
where D: Deserializer<'de>
{
    let encoded = String::deserialize(de)?;
    let decoded = base64::decode(encoded.as_bytes())
        .map_err(D::Error::custom)?;
    Ok(decoded)
}

fn deser_bytes<'de, D>(
    de: D,
) -> Result<Vec<u8>, D::Error>
where D: Deserializer<'de>
{
    Data::deserialize(de).map(|datum| datum.into_bytes())
}

fn deser_path<'de, D>(
    de: D,
) -> Result<Option<PathBuf>, D::Error>
where D: Deserializer<'de>
{
    Option::<Data>::deserialize(de)
        .map(|opt| opt.map(|datum| datum.into_path_buf()))
}
*/
