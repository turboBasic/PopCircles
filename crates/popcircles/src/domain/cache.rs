//! Versioned persistence for domain row intervals.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{ColumnInterval, Domain, FeatureId};
#[cfg(test)]
use crate::geodesy::LatLon;
use crate::grid::Grid;

pub const FORMAT_VERSION: u32 = 1;
pub const MEMBERSHIP_RULE_VERSION: u32 = 1;

const HEADER_SUFFIX: &str = ".header.json";
const PAYLOAD_SUFFIX: &str = ".payload.bin";
const TEMPORARY_SUFFIX: &str = ".tmp";

#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    source_sha256: String,
    feature_id: FeatureId,
    grid: Grid,
    decimation: u32,
    membership_rule_version: u32,
}

impl Identity {
    /// # Errors
    /// Rejects a source digest that is not a 64-character hexadecimal SHA-256 or a zero
    /// decimation factor.
    pub fn new(
        source_sha256: impl Into<String>,
        feature_id: FeatureId,
        grid: Grid,
        decimation: u32,
        membership_rule_version: u32,
    ) -> Result<Self, CacheError> {
        let source_sha256 = source_sha256.into();
        if source_sha256.len() != 64 || !source_sha256.bytes().all(is_hex_character) {
            return Err(CacheError::InvalidIdentity(
                "source SHA-256 must be 64 hex characters",
            ));
        }
        if decimation == 0 {
            return Err(CacheError::InvalidIdentity(
                "decimation factor must be positive",
            ));
        }
        Ok(Self {
            source_sha256: source_sha256.to_ascii_lowercase(),
            feature_id,
            grid,
            decimation,
            membership_rule_version,
        })
    }
}

fn is_hex_character(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) || (b'A'..=b'F').contains(&byte)
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("invalid domain cache identity: {0}")]
    InvalidIdentity(&'static str),
    #[error("domain cache at {} could not be read", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("domain cache at {} could not be written", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("domain cache header at {} is not valid JSON", path.display())]
    HeaderSyntax {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("domain cache format version {found} is unsupported; this build reads {expected}")]
    FormatVersion { expected: u32, found: u32 },
    #[error("domain cache does not match the requested {field}: wanted {wanted}, found {found}")]
    Mismatch {
        field: &'static str,
        wanted: String,
        found: String,
    },
    #[error("domain cache payload describes {expected} bytes but contains {found}")]
    PayloadLength { expected: u64, found: u64 },
    #[error("domain cache payload is malformed: {0}")]
    Payload(String),
    #[error("domain cache already exists at {}", path.display())]
    Exists { path: PathBuf },
    #[error("a domain cache write is already in progress or was interrupted at {}", path.display())]
    TemporaryHeld { path: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Header {
    format_version: u32,
    source_sha256: String,
    feature_id: String,
    width: u32,
    height: u32,
    origin_lat: f64,
    origin_lon: f64,
    lon_step: f64,
    lat_step: f64,
    decimation: u32,
    membership_rule_version: u32,
    payload_bytes: u64,
}

impl Header {
    fn new(identity: &Identity, payload_bytes: u64) -> Self {
        let origin = identity.grid.origin();
        Self {
            format_version: FORMAT_VERSION,
            source_sha256: identity.source_sha256.clone(),
            feature_id: identity.feature_id.as_str().to_owned(),
            width: identity.grid.width(),
            height: identity.grid.height(),
            origin_lat: origin.lat,
            origin_lon: origin.lon,
            lon_step: identity.grid.lon_step(),
            lat_step: identity.grid.lat_step(),
            decimation: identity.decimation,
            membership_rule_version: identity.membership_rule_version,
            payload_bytes,
        }
    }

    fn check(&self, identity: &Identity) -> Result<(), CacheError> {
        let origin = identity.grid.origin();
        let fields = [
            (
                "source digest",
                identity.source_sha256.clone(),
                self.source_sha256.clone(),
            ),
            (
                "feature identity",
                identity.feature_id.as_str().to_owned(),
                self.feature_id.clone(),
            ),
            (
                "grid width",
                identity.grid.width().to_string(),
                self.width.to_string(),
            ),
            (
                "grid height",
                identity.grid.height().to_string(),
                self.height.to_string(),
            ),
            (
                "grid origin latitude",
                origin.lat.to_string(),
                self.origin_lat.to_string(),
            ),
            (
                "grid origin longitude",
                origin.lon.to_string(),
                self.origin_lon.to_string(),
            ),
            (
                "grid longitude step",
                identity.grid.lon_step().to_string(),
                self.lon_step.to_string(),
            ),
            (
                "grid latitude step",
                identity.grid.lat_step().to_string(),
                self.lat_step.to_string(),
            ),
            (
                "decimation factor",
                identity.decimation.to_string(),
                self.decimation.to_string(),
            ),
            (
                "membership rule version",
                identity.membership_rule_version.to_string(),
                self.membership_rule_version.to_string(),
            ),
        ];
        for (field, wanted, found) in fields {
            if wanted != found {
                return Err(CacheError::Mismatch {
                    field,
                    wanted,
                    found,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct HeaderVersion {
    format_version: u32,
}

/// A pair of versioned header and compact row-interval payload files.
#[derive(Debug, Clone)]
pub struct Cache {
    header: PathBuf,
    payload: PathBuf,
    header_temporary: PathBuf,
    payload_temporary: PathBuf,
}

impl Cache {
    /// `base` is a path stem; this cache owns the suffixes appended to it.
    #[must_use]
    pub fn new(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref();
        let header = suffixed(base, HEADER_SUFFIX);
        let payload = suffixed(base, PAYLOAD_SUFFIX);
        Self {
            header_temporary: suffixed(&header, TEMPORARY_SUFFIX),
            payload_temporary: suffixed(&payload, TEMPORARY_SUFFIX),
            header,
            payload,
        }
    }

    /// Opens a domain payload only after its full identity matches the requested one.
    ///
    /// # Errors
    /// Returns an error naming malformed headers, unsupported versions, changed identity fields,
    /// incorrect payload lengths or invalid row intervals.
    pub fn load(&self, identity: &Identity) -> Result<Domain, CacheError> {
        let document = fs::read(&self.header).map_err(|source| CacheError::Read {
            path: self.header.clone(),
            source,
        })?;
        let version: HeaderVersion =
            serde_json::from_slice(&document).map_err(|source| CacheError::HeaderSyntax {
                path: self.header.clone(),
                source,
            })?;
        if version.format_version != FORMAT_VERSION {
            return Err(CacheError::FormatVersion {
                expected: FORMAT_VERSION,
                found: version.format_version,
            });
        }
        let header: Header =
            serde_json::from_slice(&document).map_err(|source| CacheError::HeaderSyntax {
                path: self.header.clone(),
                source,
            })?;
        header.check(identity)?;
        let payload = fs::read(&self.payload).map_err(|source| CacheError::Read {
            path: self.payload.clone(),
            source,
        })?;
        let found = u64::try_from(payload.len()).unwrap_or(u64::MAX);
        if found != header.payload_bytes {
            return Err(CacheError::PayloadLength {
                expected: header.payload_bytes,
                found,
            });
        }
        let rows = decode_rows(&payload, identity.grid.height())?;
        Domain::from_intervals(identity.feature_id.clone(), identity.grid, rows)
            .map_err(|error| CacheError::Payload(error.to_string()))
    }

    /// Publishes a domain's canonical interval encoding, refusing to overwrite an existing pair.
    ///
    /// The header is renamed last. If its publication fails, the newly published payload is removed,
    /// so no final partial pair remains.
    ///
    /// # Errors
    /// Returns an error if the domain identity differs, a cache already exists, or any write/rename fails.
    pub fn publish(&self, domain: &Domain, identity: &Identity) -> Result<(), CacheError> {
        if domain.feature_id != identity.feature_id {
            return Err(CacheError::Mismatch {
                field: "feature identity",
                wanted: identity.feature_id.as_str().to_owned(),
                found: domain.feature_id.as_str().to_owned(),
            });
        }
        if domain.grid != identity.grid {
            return Err(CacheError::Mismatch {
                field: "grid geometry",
                wanted: format!("{} x {}", identity.grid.width(), identity.grid.height()),
                found: format!("{} x {}", domain.grid.width(), domain.grid.height()),
            });
        }
        for path in [&self.header, &self.payload] {
            if path.exists() {
                return Err(CacheError::Exists { path: path.clone() });
            }
        }
        if let Some(parent) = self
            .header
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| CacheError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let payload = encode_rows(&domain.rows)?;
        let header = serde_json::to_vec(&Header::new(
            identity,
            u64::try_from(payload.len()).unwrap_or(u64::MAX),
        ))
        .map_err(|source| CacheError::HeaderSyntax {
            path: self.header.clone(),
            source,
        })?;

        let mut owns_payload_temporary = false;
        let mut owns_header_temporary = false;
        let mut payload_was_published = false;
        let publish_result = (|| {
            let mut payload_file = create_temporary(&self.payload_temporary)?;
            owns_payload_temporary = true;
            write_temporary(&mut payload_file, &self.payload_temporary, &payload)?;
            drop(payload_file);
            let mut header_file = create_temporary(&self.header_temporary)?;
            owns_header_temporary = true;
            write_temporary(&mut header_file, &self.header_temporary, &header)?;
            drop(header_file);
            fs::rename(&self.payload_temporary, &self.payload).map_err(|source| {
                CacheError::Write {
                    path: self.payload.clone(),
                    source,
                }
            })?;
            payload_was_published = true;
            fs::rename(&self.header_temporary, &self.header).map_err(|source| {
                CacheError::Write {
                    path: self.header.clone(),
                    source,
                }
            })?;
            Ok(())
        })();
        if publish_result.is_err() {
            if owns_payload_temporary {
                let _ = fs::remove_file(&self.payload_temporary);
            }
            if owns_header_temporary {
                let _ = fs::remove_file(&self.header_temporary);
            }
            if payload_was_published {
                let _ = fs::remove_file(&self.payload);
            }
        }
        publish_result
    }
}

fn encode_rows(rows: &[Vec<ColumnInterval>]) -> Result<Vec<u8>, CacheError> {
    let mut bytes = Vec::new();
    for row in rows {
        let count = u32::try_from(row.len())
            .map_err(|_| CacheError::Payload("one row has too many intervals".to_owned()))?;
        bytes.extend_from_slice(&count.to_le_bytes());
        for interval in row {
            bytes.extend_from_slice(&interval.start().to_le_bytes());
            bytes.extend_from_slice(&interval.end().to_le_bytes());
        }
    }
    Ok(bytes)
}

fn decode_rows(payload: &[u8], height: u32) -> Result<Vec<Vec<ColumnInterval>>, CacheError> {
    let mut cursor = 0usize;
    let mut rows = Vec::new();
    rows.try_reserve_exact(usize::try_from(height).unwrap_or(usize::MAX))
        .map_err(|error| CacheError::Payload(format!("cannot allocate row index: {error}")))?;
    for _ in 0..height {
        let count = read_u32(payload, &mut cursor)?;
        let remaining_intervals = payload.len().saturating_sub(cursor) / 8;
        if usize::try_from(count).unwrap_or(usize::MAX) > remaining_intervals {
            return Err(CacheError::Payload(
                "interval count exceeds the remaining payload".to_owned(),
            ));
        }
        let mut intervals = Vec::new();
        intervals
            .try_reserve_exact(usize::try_from(count).unwrap_or(usize::MAX))
            .map_err(|error| {
                CacheError::Payload(format!("cannot allocate row intervals: {error}"))
            })?;
        for _ in 0..count {
            let start = read_u32(payload, &mut cursor)?;
            let end = read_u32(payload, &mut cursor)?;
            intervals.push(ColumnInterval { start, end });
        }
        rows.push(intervals);
    }
    if cursor != payload.len() {
        return Err(CacheError::Payload(format!(
            "payload has {} trailing bytes",
            payload.len() - cursor
        )));
    }
    Ok(rows)
}

fn read_u32(payload: &[u8], cursor: &mut usize) -> Result<u32, CacheError> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| CacheError::Payload("payload offset overflow".to_owned()))?;
    let bytes = payload
        .get(*cursor..end)
        .ok_or_else(|| CacheError::Payload("payload ends inside an interval row".to_owned()))?;
    let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    *cursor = end;
    Ok(value)
}

fn create_temporary(path: &Path) -> Result<File, CacheError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                CacheError::TemporaryHeld {
                    path: path.to_path_buf(),
                }
            } else {
                CacheError::Write {
                    path: path.to_path_buf(),
                    source,
                }
            }
        })
}

fn write_temporary(file: &mut File, path: &Path, bytes: &[u8]) -> Result<(), CacheError> {
    file.write_all(bytes).map_err(|source| CacheError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    file.sync_all().map_err(|source| CacheError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
#[allow(clippy::expect_used)] // Cache fixtures fail at the operation under test, not during setup.
mod tests {
    use super::*;

    fn make_grid(width: u32, height: u32, origin: LatLon, x_step: f64, y_step: f64) -> Grid {
        Grid::new(width, height, origin, x_step, y_step).expect("test grid is valid")
    }

    fn make_identity(
        source: &str,
        feature: &str,
        grid: Grid,
        decimation: u32,
        rule: u32,
    ) -> Identity {
        Identity::new(
            source,
            FeatureId::new(feature).expect("feature ID is nonempty"),
            grid,
            decimation,
            rule,
        )
        .expect("cache identity is valid")
    }

    fn domain(grid: Grid, feature: &str) -> Domain {
        let rows = (0..grid.height())
            .map(|row| {
                if row == 0 {
                    vec![ColumnInterval { start: 1, end: 3 }]
                } else {
                    vec![]
                }
            })
            .collect();
        Domain::from_intervals(FeatureId::new(feature).expect("feature ID"), grid, rows)
            .expect("fixture intervals fit the grid")
    }

    #[test]
    fn identical_identity_round_trips_canonical_intervals() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let grid = make_grid(
            4,
            2,
            LatLon {
                lat: 2.0,
                lon: -2.0,
            },
            1.0,
            -1.0,
        );
        let identity = make_identity(&"a".repeat(64), "AA", grid, 10, MEMBERSHIP_RULE_VERSION);
        let cache = Cache::new(directory.path().join("domain"));
        let expected = domain(grid, "AA");
        cache
            .publish(&expected, &identity)
            .expect("cache publishes");
        assert_eq!(cache.load(&identity).expect("identity matches"), expected);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One assertion table covers every persisted identity field.
    fn every_identity_component_is_checked() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let grid = make_grid(
            4,
            2,
            LatLon {
                lat: 2.0,
                lon: -2.0,
            },
            1.0,
            -1.0,
        );
        let identity = make_identity(&"a".repeat(64), "AA", grid, 10, 1);
        let cache = Cache::new(directory.path().join("domain"));
        cache
            .publish(&domain(grid, "AA"), &identity)
            .expect("cache publishes");
        let changed = [
            make_identity(&"b".repeat(64), "AA", grid, 10, 1),
            make_identity(&"a".repeat(64), "BB", grid, 10, 1),
            make_identity(
                &"a".repeat(64),
                "AA",
                make_grid(
                    5,
                    2,
                    LatLon {
                        lat: 2.0,
                        lon: -2.0,
                    },
                    1.0,
                    -1.0,
                ),
                10,
                1,
            ),
            make_identity(
                &"a".repeat(64),
                "AA",
                make_grid(
                    4,
                    3,
                    LatLon {
                        lat: 2.0,
                        lon: -2.0,
                    },
                    1.0,
                    -1.0,
                ),
                10,
                1,
            ),
            make_identity(
                &"a".repeat(64),
                "AA",
                make_grid(
                    4,
                    2,
                    LatLon {
                        lat: 3.0,
                        lon: -2.0,
                    },
                    1.0,
                    -1.0,
                ),
                10,
                1,
            ),
            make_identity(
                &"a".repeat(64),
                "AA",
                make_grid(
                    4,
                    2,
                    LatLon {
                        lat: 2.0,
                        lon: -1.0,
                    },
                    1.0,
                    -1.0,
                ),
                10,
                1,
            ),
            make_identity(
                &"a".repeat(64),
                "AA",
                make_grid(
                    4,
                    2,
                    LatLon {
                        lat: 2.0,
                        lon: -2.0,
                    },
                    0.5,
                    -1.0,
                ),
                10,
                1,
            ),
            make_identity(
                &"a".repeat(64),
                "AA",
                make_grid(
                    4,
                    2,
                    LatLon {
                        lat: 2.0,
                        lon: -2.0,
                    },
                    1.0,
                    -0.5,
                ),
                10,
                1,
            ),
            make_identity(&"a".repeat(64), "AA", grid, 1, 1),
            make_identity(&"a".repeat(64), "AA", grid, 10, 2),
        ];
        for changed in changed {
            assert!(matches!(
                cache.load(&changed),
                Err(CacheError::Mismatch { .. })
            ));
        }
    }

    #[test]
    fn unsupported_version_and_malformed_truncated_or_trailing_payloads_are_refused() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let grid = make_grid(
            4,
            2,
            LatLon {
                lat: 2.0,
                lon: -2.0,
            },
            1.0,
            -1.0,
        );
        let identity = make_identity(&"a".repeat(64), "AA", grid, 10, 1);
        let cache = Cache::new(directory.path().join("domain"));
        cache
            .publish(&domain(grid, "AA"), &identity)
            .expect("cache publishes");
        let payload = fs::read(&cache.payload).expect("payload reads");
        fs::write(cache.payload.clone(), [0]).expect("truncated payload writes");
        assert!(matches!(
            cache.load(&identity),
            Err(CacheError::PayloadLength { .. })
        ));

        let mut header: Header =
            serde_json::from_slice(&fs::read(&cache.header).expect("header reads"))
                .expect("header parses");
        let mut trailing = payload.clone();
        trailing.push(0);
        header.payload_bytes = u64::try_from(trailing.len()).expect("payload length fits u64");
        fs::write(&cache.payload, trailing).expect("trailing payload writes");
        fs::write(
            &cache.header,
            serde_json::to_vec(&header).expect("header serializes"),
        )
        .expect("updated payload length writes");
        assert!(matches!(cache.load(&identity), Err(CacheError::Payload(_))));

        let mut malformed = payload;
        malformed[8..12].copy_from_slice(&0_u32.to_le_bytes());
        fs::write(&cache.payload, malformed).expect("malformed interval payload writes");
        header.payload_bytes = fs::metadata(&cache.payload).expect("payload stats").len();
        fs::write(
            &cache.header,
            serde_json::to_vec(&header).expect("header serializes"),
        )
        .expect("malformed payload length writes");
        assert!(matches!(cache.load(&identity), Err(CacheError::Payload(_))));

        header.format_version = FORMAT_VERSION + 1;
        fs::write(
            &cache.header,
            serde_json::to_vec(&header).expect("header serializes"),
        )
        .expect("version header writes");
        assert!(matches!(
            cache.load(&identity),
            Err(CacheError::FormatVersion { .. })
        ));
    }

    #[test]
    fn a_held_header_temporary_leaves_no_final_cache_or_owned_payload_temporary() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let grid = make_grid(
            4,
            2,
            LatLon {
                lat: 2.0,
                lon: -2.0,
            },
            1.0,
            -1.0,
        );
        let identity = make_identity(&"a".repeat(64), "AA", grid, 10, 1);
        let base = directory.path().join("domain");
        let cache = Cache::new(&base);
        fs::write(&cache.header_temporary, b"held").expect("header temporary is held");
        assert!(cache.publish(&domain(grid, "AA"), &identity).is_err());
        assert!(!cache.payload.exists());
        assert!(!cache.payload_temporary.exists());
        assert!(cache.header_temporary.exists());
    }

    #[test]
    fn interval_shape_is_validated_after_decode() {
        let malformed = [0, 0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0];
        assert!(matches!(
            decode_rows(&malformed, 2),
            Err(CacheError::Payload(_))
        ));
    }
}
