// The I/O half of the summation table: the header, the payload and the publication that ties the two.
// Every path and every serde derive is kept here, which is what leaves `table.rs` a
// computation its tests reach without a filesystem — the split `raster.rs` and `raster/geotiff.rs`
// already have.
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use memmap2::Mmap;
use serde::{Deserialize, Serialize};

use super::{BuiltTable, Decimation, padded_len};
use crate::geodesy::wrap_lon;
use crate::grid::BOUNDARY_TOLERANCE_DEG;

/// Bumped when a change to the header's fields or the payload's layout is one an older reader would
/// misread rather than refuse.
pub const FORMAT_VERSION: u32 = 3;

const HEADER_SUFFIX: &str = ".header.json";
const PAYLOAD_SUFFIX: &str = ".payload.bin";
/// One suffix for both temporaries, so an interrupted build leaves two names and no more. The payload's
/// is created exclusively and so is also this cache base's lock — [`Cache::writer`].
const TEMPORARY_SUFFIX: &str = ".tmp";

/// The order a payload's cells are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    Little,
    Big,
}

impl ByteOrder {
    /// The order every payload is written in, and the only one cache readers accept.
    /// `bytemuck` casts natively and swaps nothing, so a payload from a host of the other order is
    /// rebuilt rather than reinterpreted, and a format declaring one order while casting in another
    /// would be silently wrong on half the hosts there are.
    pub const HOST: Self = if cfg!(target_endian = "little") {
        Self::Little
    } else {
        Self::Big
    };
}

impl fmt::Display for ByteOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Little => f.write_str("little-endian"),
            Self::Big => f.write_str("big-endian"),
        }
    }
}

/// Every way a cache can fail to be the table a caller asked for.
///
/// One variant per ground, so a refusal says which of them fired: a caller that rebuilds on a digest
/// mismatch and reports a moved payload as an error has to be able to tell the two apart, and a message
/// reading only "stale cache" sends its reader to a hex editor. The same shape, and the same reason, as
/// [`RasterError`](crate::raster::RasterError). The grounds for not being this table are [`Mismatch`]'s,
/// carried by one variant here because the ledger shares them.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("no cache at {}: nothing has been published there", path.display())]
    Absent { path: PathBuf },

    #[error("the cache header at {} could not be read", path.display())]
    HeaderRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the cache header at {} could not be written", path.display())]
    HeaderWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the cache header at {} is not the JSON document this format is", path.display())]
    HeaderSyntax {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("the cache payload at {} could not be read", path.display())]
    PayloadRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the cache payload at {} could not be written", path.display())]
    PayloadWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    // Its own ground rather than a `PayloadWrite`: this is the one refusal a caller can act on without
    // reading the source, and the action is not the same as any other write failure's.
    #[error(
        "a payload temporary is already at {}: either a build is writing this cache now, or one was \
         interrupted and left it — remove it to build again",
        path.display()
    )]
    PayloadTemporaryHeld { path: PathBuf },

    #[error("this build reads format version {expected}; the header declares {found}")]
    FormatVersion { expected: u32, found: u32 },

    #[error("this host is {expected}; the payload was written {found}, so it has to be rebuilt")]
    ByteOrderMismatch {
        expected: ByteOrder,
        found: ByteOrder,
    },

    // The wrapper names the document and the ground says what differed, which is the shared
    // attestation's cost: the four per-ground variants this replaces each named the header in their own message, and a
    // ground shared with the ledger cannot. A caller telling a digest miss from a moved grid matches one
    // level deeper rather than losing the distinction.
    #[error("the cache header does not describe the table wanted: {0}")]
    NotThisTable(Mismatch),

    #[error("the header describes {expected} payload bytes; the payload stops after {found}")]
    PayloadTruncated { expected: usize, found: usize },

    #[error("the header describes {expected} payload bytes; the payload carries {found}")]
    PayloadTrailing { expected: usize, found: usize },

    // The allocator's doing rather than the file's, and the reason it is a variant at all: a payload
    // read into a `Vec<u8>` is aligned to whatever the allocator returned, which every real one over-
    // aligns and none of them promises. A mapping cannot reach this — it starts at offset 0 of a page.
    #[error(
        "the payload was read into a buffer that is not aligned for {}-byte cells",
        align_of::<f64>()
    )]
    PayloadAlignment,
}

/// What a caller requires of a cache before it will read one: the cells the table was built from, and
/// how coarsely they were folded.
///
/// Both are what a build already produces, so a caller falling back to building has nothing extra to
/// compute, and the grid the table is over travels with the factor rather than beside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Identity {
    pub digest: u64,
    pub decimation: Decimation,
}

impl From<&BuiltTable> for Identity {
    fn from(built: &BuiltTable) -> Self {
        Self {
            digest: built.digest,
            decimation: built.decimation,
        }
    }
}

/// Which of the grounds a document and an [`Identity`] disagree on.
///
/// The messages name what was wanted and what was found and no document, because both a cache header and
/// a radius ledger wrap this and each names itself: the noun goes in the wrapper so a
/// ground is added in one place rather than phrased twice.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum Mismatch {
    #[error("wanted the table of the cells digesting to {wanted:#018x}; found {found:#018x}")]
    Digest { wanted: u64, found: u64 },

    #[error("wanted a {wanted}-column table; found {found}")]
    Width { wanted: u32, found: u32 },

    #[error("wanted a {wanted}-row table; found {found}")]
    Height { wanted: u32, found: u32 },

    #[error("wanted a table decimated by {wanted}; found {found}")]
    DecimationFactor { wanted: u32, found: u32 },

    #[error("wanted a grid whose origin latitude is {wanted}; found {found}")]
    OriginLat { wanted: f64, found: f64 },

    #[error("wanted a grid whose origin longitude is {wanted}; found {found}")]
    OriginLon { wanted: f64, found: f64 },

    #[error("wanted a grid whose longitude step is {wanted}; found {found}")]
    LonStep { wanted: f64, found: f64 },

    #[error("wanted a grid whose latitude step is {wanted}; found {found}")]
    LatStep { wanted: f64, found: f64 },
}

/// What a document claims the table beside it is: the cells it was built from, how coarsely they were
/// folded, and the whole grid it resolves coordinates against.
///
/// One type, serialised into both the cache header and the radius ledger with
/// `#[serde(flatten)]`, so the comparison and its tolerance exist once. The grid recorded is the table's
/// own and not the source's: given the factor the source's is determined, and the coarser one is what a
/// query resolves against.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Attestation {
    pub digest: u64,
    pub width: u32,
    pub height: u32,
    pub decimation: u32,
    pub origin_lat: f64,
    pub origin_lon: f64,
    pub lon_step: f64,
    pub lat_step: f64,
}

impl Attestation {
    #[must_use]
    pub fn new(identity: &Identity) -> Self {
        let grid = identity.decimation.grid();
        let origin = grid.origin();
        Self {
            digest: identity.digest,
            width: grid.width(),
            height: grid.height(),
            decimation: identity.decimation.factor(),
            origin_lat: origin.lat,
            origin_lon: origin.lon,
            lon_step: grid.lon_step(),
            lat_step: grid.lat_step(),
        }
    }

    /// The digest, the dimensions and the factor exactly; the four geometry numbers within
    /// `BOUNDARY_TOLERANCE_DEG`, longitude through [`wrap_lon`].
    ///
    /// A tolerance rather than the bit equality a JSON round trip would in fact give, and for the
    /// raster reader's reason: it compares a file's geotransform against a declared grid by exactly
    /// this rule, so an exact comparison here would refuse a cache built over a raster that reader had
    /// accepted.
    ///
    /// # Errors
    /// [`Mismatch`], whose variants are the grounds, naming the first field that differs.
    pub fn check(&self, wanted: &Identity) -> Result<(), Mismatch> {
        self.check_attestation(&Self::new(wanted))
    }

    /// Checks another attestation using the same geometry tolerance as [`Self::check`].
    ///
    /// # Errors
    /// Returns [`Mismatch`] for the first field that differs.
    pub fn check_attestation(&self, wanted: &Self) -> Result<(), Mismatch> {
        if self.digest != wanted.digest {
            return Err(Mismatch::Digest {
                wanted: wanted.digest,
                found: self.digest,
            });
        }

        if self.width != wanted.width {
            return Err(Mismatch::Width {
                wanted: wanted.width,
                found: self.width,
            });
        }
        if self.height != wanted.height {
            return Err(Mismatch::Height {
                wanted: wanted.height,
                found: self.height,
            });
        }
        if self.decimation != wanted.decimation {
            return Err(Mismatch::DecimationFactor {
                wanted: wanted.decimation,
                found: self.decimation,
            });
        }

        if (self.origin_lat - wanted.origin_lat).abs() > BOUNDARY_TOLERANCE_DEG {
            return Err(Mismatch::OriginLat {
                wanted: wanted.origin_lat,
                found: self.origin_lat,
            });
        }
        // Through the seam, for the reader's reason: -180 and 180 are one meridian, and a document
        // spelling the origin either way describes the same columns.
        if wrap_lon(self.origin_lon - wanted.origin_lon).abs() > BOUNDARY_TOLERANCE_DEG {
            return Err(Mismatch::OriginLon {
                wanted: wanted.origin_lon,
                found: self.origin_lon,
            });
        }
        if (self.lon_step - wanted.lon_step).abs() > BOUNDARY_TOLERANCE_DEG {
            return Err(Mismatch::LonStep {
                wanted: wanted.lon_step,
                found: self.lon_step,
            });
        }
        if (self.lat_step - wanted.lat_step).abs() > BOUNDARY_TOLERANCE_DEG {
            return Err(Mismatch::LatStep {
                wanted: wanted.lat_step,
                found: self.lat_step,
            });
        }
        Ok(())
    }
}

/// The one field a header of any version carries, so the version can be compared before the rest of the
/// document is parsed at all.
///
/// This rests on a default rather than a declaration: serde ignores keys a struct
/// does not name, which is what lets this read a header of a shape this build has never seen. A
/// `deny_unknown_fields` here would therefore refuse every real header, and
/// `the_version_is_read_out_of_a_document_carrying_more` is the test that says so.
#[derive(Debug, Clone, Copy, Deserialize)]
struct HeaderVersion {
    format_version: u32,
}

/// What the payload beside it is a table of.
///
/// `format_version` is declared first because serde emits struct fields in declaration order, so the
/// field a reader needs before it can trust any other is the first one it meets — the same reason
/// [`Envelope`](crate::report::Envelope) leads with its schema version. The table's identity is the
/// flattened [`Attestation`], so the header stays the flat object a person can read with `cat` and the
/// comparison is not written here; `byte_order` is last and is the header's alone, because it describes a
/// payload of raw f64 and a ledger's numbers are JSON text with no order to disagree about.
///
/// `dataset` is what the cells came from rather than what they are, which is why it is second and why
/// [`Self::check`] does not compare it: the digest already binds the cells, so a name is a fact about
/// provenance that travels with the table rather than a ground for refusing one. Absent — the key omitted,
/// not null — when a build named none, which is every build driven through this crate's own API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Header {
    format_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dataset: Option<String>,
    #[serde(flatten)]
    attestation: Attestation,
    byte_order: ByteOrder,
}

impl Header {
    fn new(identity: &Identity, dataset: Option<&str>) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            dataset: dataset.map(str::to_owned),
            attestation: Attestation::new(identity),
            byte_order: ByteOrder::HOST,
        }
    }

    /// The byte order comes before the identity: a payload in the other order is not a number this host
    /// can compare against anything. The version is not compared here — [`Cache::checked_header`] reads
    /// it out of the document before this struct is parsed, because a header of another version need not
    /// parse into this shape at all.
    fn check(&self, wanted: &Identity) -> Result<(), CacheError> {
        if self.byte_order != ByteOrder::HOST {
            return Err(CacheError::ByteOrderMismatch {
                expected: ByteOrder::HOST,
                found: self.byte_order,
            });
        }
        self.attestation
            .check(wanted)
            .map_err(CacheError::NotThisTable)
    }
}

/// The checked identity and provenance travel with the payload length so opening it needs no second
/// header read.
#[derive(Debug)]
struct Described {
    cells: usize,
    dataset: Option<String>,
    identity: Identity,
}

/// The two files a cache is, at one location.
///
/// A header and a payload rather than one self-describing file: a header of any size
/// that is not a page multiple leaves the payload unaligned in a mapping, and a JSON sidecar is
/// something a person debugging a stale cache can read with `cat`.
#[derive(Debug, Clone)]
pub struct Cache {
    header: PathBuf,
    payload: PathBuf,
    header_temporary: PathBuf,
    payload_temporary: PathBuf,
}

impl Cache {
    /// `base` names both files: it is a path with the suffixes this module owns appended, not a
    /// directory and not a file.
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

    #[must_use]
    pub fn header_path(&self) -> &Path {
        &self.header
    }

    #[must_use]
    pub fn payload_path(&self) -> &Path {
        &self.payload
    }

    /// Opens the payload a build writes its rows into, exclusively.
    ///
    /// The temporary is this cache base's lock, and the name being deterministic is what makes it one:
    /// two builds into one base cannot hold the payload's inode between them, so neither can publish a
    /// length the header agrees with over bytes the other wrote. The digest could not do this job — a
    /// build produces it, so at this point no caller has one.
    ///
    /// The cost is an interrupted build's temporary refusing the next one, because no writer can tell a
    /// temporary nobody holds from one being written into.
    ///
    /// # Errors
    /// [`CacheError::PayloadTemporaryHeld`] when a temporary is already there;
    /// [`CacheError::PayloadWrite`] when it cannot be created for any other reason.
    pub fn writer(&self) -> Result<Writer<'_>, CacheError> {
        let file = File::create_new(&self.payload_temporary).map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                CacheError::PayloadTemporaryHeld {
                    path: self.payload_temporary.clone(),
                }
            } else {
                CacheError::PayloadWrite {
                    path: self.payload_temporary.clone(),
                    source,
                }
            }
        })?;
        Ok(Writer {
            cache: self,
            file,
            cells: 0,
        })
    }

    /// The payload behind a header that describes the table `wanted`, mapped rather than read.
    ///
    /// The path for a table that does not fit in memory, which at full resolution is every table:
    /// [`read`](Self::read) materialises 8 bytes a cell and this does not.
    ///
    /// # Errors
    /// [`CacheError::Absent`] when no header is there, and one variant per ground when what is there
    /// is not this table: see [`CacheError`].
    pub fn open(&self, wanted: &Identity) -> Result<Mapped, CacheError> {
        self.open_resolved(wanted.decimation, Some(wanted.digest))
            .map(|(mapped, _)| mapped)
    }

    /// An omitted digest selects the table at this path; it does not verify the source raster or
    /// recompute the decoded digest. The requested geometry and factor are still checked.
    ///
    /// # Errors
    /// As [`open`](Self::open), including a digest mismatch when an expectation was supplied.
    pub fn open_resolved(
        &self,
        decimation: Decimation,
        expected_digest: Option<u64>,
    ) -> Result<(Mapped, Identity), CacheError> {
        let described = self.checked_header(decimation, expected_digest)?;
        let identity = described.identity;
        let mapped = Mapped::open(&self.payload, described)?;
        Ok((mapped, identity))
    }

    /// The payload behind a header that describes the table `wanted`, read into memory.
    ///
    /// # Errors
    /// As [`open`](Self::open).
    pub fn read(&self, wanted: &Identity) -> Result<Payload, CacheError> {
        let cells = self
            .checked_header(wanted.decimation, Some(wanted.digest))?
            .cells;
        let bytes = fs::read(&self.payload).map_err(|source| CacheError::PayloadRead {
            path: self.payload.clone(),
            source,
        })?;
        let payload = Payload { bytes, cells };
        // The cast here and not only in the accessor: a payload the header does not describe is a
        // refusal a caller can act on at the point it asked for a table, rather than one arriving at
        // whichever query first looked.
        payload.cells()?;
        Ok(payload)
    }

    /// The header, checked against what the caller wants, and what a reader takes from it: the payload
    /// length it implies, the resolved identity and the dataset it names. Every read path goes through
    /// here so none can skip a refusal ground.
    fn checked_header(
        &self,
        decimation: Decimation,
        expected_digest: Option<u64>,
    ) -> Result<Described, CacheError> {
        let document = fs::read(&self.header).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                CacheError::Absent {
                    path: self.header.clone(),
                }
            } else {
                CacheError::HeaderRead {
                    path: self.header.clone(),
                    source,
                }
            }
        })?;
        // The version out of the document before the document: a header of
        // another version is not required to parse into this build's `Header`, so without this probe a
        // bumped format reports as "not the JSON document this format is" and the constant is decoration.
        let probed: HeaderVersion =
            serde_json::from_slice(&document).map_err(|source| CacheError::HeaderSyntax {
                path: self.header.clone(),
                source,
            })?;
        if probed.format_version != FORMAT_VERSION {
            return Err(CacheError::FormatVersion {
                expected: FORMAT_VERSION,
                found: probed.format_version,
            });
        }

        let header: Header =
            serde_json::from_slice(&document).map_err(|source| CacheError::HeaderSyntax {
                path: self.header.clone(),
                source,
            })?;
        let identity = Identity {
            digest: expected_digest.unwrap_or(header.attestation.digest),
            decimation,
        };
        header.check(&identity)?;
        Ok(Described {
            cells: padded_len(decimation.grid()),
            dataset: header.dataset,
            identity,
        })
    }

    /// The commit record, and the last thing publication does: a reader that finds a header finds a
    /// complete payload behind it.
    fn commit_header(&self, identity: &Identity, dataset: Option<&str>) -> Result<(), CacheError> {
        let document = serde_json::to_vec(&Header::new(identity, dataset)).map_err(|source| {
            CacheError::HeaderWrite {
                path: self.header.clone(),
                source: io::Error::other(source),
            }
        })?;
        let write = |path: &PathBuf, source| CacheError::HeaderWrite {
            path: path.clone(),
            source,
        };

        // Truncating rather than exclusive, where the payload's temporary is the opposite: this runs
        // only for a caller that already holds that one, so the race it would guard against is already
        // lost by whoever does not.
        let mut file = File::create(&self.header_temporary)
            .map_err(|source| write(&self.header_temporary, source))?;
        file.write_all(&document)
            .map_err(|source| write(&self.header_temporary, source))?;
        file.sync_all()
            .map_err(|source| write(&self.header_temporary, source))?;
        drop(file);
        fs::rename(&self.header_temporary, &self.header)
            .map_err(|source| write(&self.header, source))
    }
}

/// A build's payload, under a temporary name until [`Writer::publish`] renames it into place.
#[derive(Debug)]
pub struct Writer<'a> {
    cache: &'a Cache,
    file: File,
    cells: usize,
}

impl Writer<'_> {
    /// One completed padded row, in the order [`build`](super::build) emits them.
    ///
    /// # Errors
    /// [`CacheError::PayloadWrite`] when the row cannot be written.
    pub fn write_row(&mut self, row: &[f64]) -> Result<(), CacheError> {
        // f64 to bytes cannot fail — eight divides eight and the alignment falls — and `must_cast_slice`
        // is the form that says so at compile time instead of panicking if it ever stopped being true.
        self.file
            .write_all(bytemuck::must_cast_slice(row))
            .map_err(|source| CacheError::PayloadWrite {
                path: self.cache.payload_temporary.clone(),
                source,
            })?;
        self.cells += row.len();
        Ok(())
    }

    /// Publishes the payload, then the header.
    ///
    /// That order is ADR 0005's ground, not tidiness: no file is written into in place, so a rename
    /// replaces a directory entry rather than an inode, and a mapping already established keeps the
    /// bytes it mapped while a fresh build publishes over the same path.
    ///
    /// `dataset` is what the cells came from, for a caller that resolved one; this crate resolves none.
    ///
    /// # Errors
    /// [`CacheError::PayloadTruncated`] or [`CacheError::PayloadTrailing`] when the rows written are not
    /// the table `built` describes; [`CacheError::PayloadWrite`] and [`CacheError::HeaderWrite`] when a
    /// sync or a rename fails.
    pub fn publish(self, built: &BuiltTable, dataset: Option<&str>) -> Result<(), CacheError> {
        let cache = self.cache;
        let identity = Identity::from(built);
        self.commit_payload(&identity)?;
        cache.commit_header(&identity, dataset)
    }

    fn commit_payload(self, identity: &Identity) -> Result<(), CacheError> {
        let Self { cache, file, cells } = self;
        // The one thing a writer can be wrong about, refused before a header claiming otherwise is
        // published: neither file is ever trusted to describe the other.
        let expected = payload_bytes(padded_len(identity.decimation.grid()));
        let found = payload_bytes(cells);
        if found < expected {
            return Err(CacheError::PayloadTruncated { expected, found });
        }
        if found > expected {
            return Err(CacheError::PayloadTrailing { expected, found });
        }

        let write = |path: &PathBuf, source| CacheError::PayloadWrite {
            path: path.clone(),
            source,
        };
        file.sync_all()
            .map_err(|source| write(&cache.payload_temporary, source))?;
        drop(file);
        fs::rename(&cache.payload_temporary, &cache.payload)
            .map_err(|source| write(&cache.payload, source))
    }
}

/// A payload mapped into the address space, and the cell count [`Cache::open`] validated.
///
/// It keeps the mapping and not the [`File`]: a mapping outlives the descriptor that created it, so a
/// held descriptor would be a resource nothing reads. It hands out `&[f64]` for
/// [`Table`](super::Table) to borrow, which ties the lifetime of every query to the mapping through the
/// compiler rather than through a rule someone has to remember.
#[derive(Debug)]
pub struct Mapped {
    map: Mmap,
    cells: usize,
    dataset: Option<String>,
}

impl Mapped {
    /// The dataset the header names, for a caller publishing where an answer came from. Absent when the
    /// build that wrote this cache named none.
    #[must_use]
    pub fn dataset(&self) -> Option<&str> {
        self.dataset.as_deref()
    }

    // The one exception in this workspace: `Cargo.toml` says why `unsafe_code` is `deny` rather than
    // `forbid`, and the `single-unsafe-allow` hook is what asserts this is still the only one. The hook
    // counts the attribute below as text, so naming it a second time anywhere under `crates/` fires it.
    #[allow(unsafe_code)]
    fn open(path: &Path, described: Described) -> Result<Self, CacheError> {
        let read = |source| CacheError::PayloadRead {
            path: path.to_path_buf(),
            source,
        };
        let file = File::open(path).map_err(read)?;

        // What makes this sound is [`Writer::publish`]'s immutable publication, not this crate's
        // authorship of the file. A rename replaces a directory entry rather than an inode, and no
        // payload is ever written into one already in place, so a fresh build publishing over this path
        // leaves the bytes a mapping already holds untouched. The residual is a third party truncating
        // the cache directory by hand: mmap has no defence against it, the failure is a fault on access
        // rather than a wrong number taken for a right one, and the payload is a derived artefact a
        // build replaces from the raster in about fifteen seconds.
        // SAFETY: no writer this project contains can shrink or rewrite a payload already in place.
        let map = unsafe { Mmap::map(&file) }.map_err(read)?;
        drop(file);

        let mapped = Self {
            map,
            cells: described.cells,
            dataset: described.dataset,
        };
        // The checked cast at open, once, for [`Cache::read`]'s reason.
        mapped.cells()?;
        Ok(mapped)
    }

    /// The mapping as cells, for [`Table::new`](super::Table::new) to borrow.
    ///
    /// # Errors
    /// As [`Payload::cells`], and unreachable for the same reason.
    pub fn cells(&self) -> Result<&[f64], CacheError> {
        view(&self.map, self.cells)
    }
}

/// A payload read into memory, and the cell count the header said it would be.
///
/// It owns bytes rather than cells because the cast is what checks them: an owner of `Vec<f64>` would
/// have had to copy or to cast somewhere it could not report the failure.
#[derive(Debug)]
pub struct Payload {
    bytes: Vec<u8>,
    cells: usize,
}

impl Payload {
    /// The payload as cells, for [`Table::new`](super::Table::new) to borrow.
    ///
    /// # Errors
    /// [`CacheError::PayloadTruncated`], [`CacheError::PayloadTrailing`] or
    /// [`CacheError::PayloadAlignment`] — every one of which [`Cache::read`] has already refused, so a
    /// caller holding a [`Payload`] is asking a question that has been answered.
    pub fn cells(&self) -> Result<&[f64], CacheError> {
        view(&self.bytes, self.cells)
    }
}

/// The one place bytes become cells, so the mapped path and the resident one refuse the same files.
fn view(bytes: &[u8], cells: usize) -> Result<&[f64], CacheError> {
    let expected = payload_bytes(cells);
    if bytes.len() < expected {
        return Err(CacheError::PayloadTruncated {
            expected,
            found: bytes.len(),
        });
    }
    if bytes.len() > expected {
        return Err(CacheError::PayloadTrailing {
            expected,
            found: bytes.len(),
        });
    }
    bytemuck::try_cast_slice(bytes).map_err(|_| CacheError::PayloadAlignment)
}

/// Saturating for [`padded_len`]'s reason: a cell count too large for this host's addresses is a length
/// no payload matches, rather than a smaller one some payload does.
fn payload_bytes(cells: usize) -> usize {
    cells.saturating_mul(size_of::<f64>())
}

fn suffixed(base: &Path, suffix: &str) -> PathBuf {
    let mut path = base.as_os_str().to_owned();
    path.push(suffix);
    PathBuf::from(path)
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is here because the round trip's claim is
// bit-identity, which a tolerance would not check; cast_precision_loss because the fixture's cells are
// small integers, exact in f32.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::geodesy::LatLon;
    use crate::grid::Grid;
    use crate::raster::Synthetic;
    use crate::table::{ColSpan, RowBand, Table, build};

    /// The registry raster's sentinel, so the fixtures below get the sanitising the real reader does.
    const NODATA: f32 = -3.402_823e38;

    fn grid(width: u32, height: u32) -> Grid {
        Grid::new(
            width,
            height,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            360.0 / f64::from(width),
            -180.0 / f64::from(height),
        )
        .expect("a whole-globe grid of any shape is valid")
    }

    /// Cells no two of which are equal, so a payload written or read at the wrong offset shows up as a
    /// wrong sum rather than as a coincidence.
    fn cells_of(grid: Grid) -> Vec<Vec<f32>> {
        (0..grid.height())
            .map(|row| {
                (0..grid.width())
                    .map(|col| (row * grid.width() + col) as f32 + 0.5)
                    .collect()
            })
            .collect()
    }

    fn source(grid: Grid) -> Synthetic {
        Synthetic::new(grid, NODATA, cells_of(grid)).expect("the fixture is the grid's shape")
    }

    /// Builds a table straight into the cache and commits both files, returning what the build settled.
    fn publish(cache: &Cache, decimation: Decimation) -> BuiltTable {
        publish_named(cache, decimation, None)
    }

    fn publish_named(cache: &Cache, decimation: Decimation, dataset: Option<&str>) -> BuiltTable {
        let mut writer = cache.writer().unwrap();
        let built = build(source(*decimation.source()), decimation, &mut (), |row| {
            writer.write_row(row)
        })
        .expect("a synthetic source cannot fail and the sink writes to a temporary directory");
        writer.publish(&built, dataset).unwrap();
        built
    }

    /// The payload alone, which is what an interrupted publication leaves behind.
    fn publish_payload_only(cache: &Cache, decimation: Decimation) -> BuiltTable {
        let mut writer = cache.writer().unwrap();
        let built = build(source(*decimation.source()), decimation, &mut (), |row| {
            writer.write_row(row)
        })
        .expect("a synthetic source cannot fail and the sink writes to a temporary directory");
        writer.commit_payload(&Identity::from(&built)).unwrap();
        built
    }

    fn cache_in(directory: &TempDir) -> Cache {
        Cache::new(directory.path().join("gpw-v4"))
    }

    fn header_of(built: &BuiltTable) -> Header {
        Header::new(&Identity::from(built), None)
    }

    fn write_header(cache: &Cache, header: &Header) {
        fs::write(cache.header_path(), serde_json::to_vec(header).unwrap()).unwrap();
    }

    /// Every rectangle of a table, in one order: two of these are equal only if the tables agree over
    /// the whole grid, wrapped spans and full turns included.
    fn every_rectangle(table: &Table<'_>) -> Vec<f64> {
        let grid = *table.grid();
        let mut sums = Vec::new();
        for north in 0..grid.height() {
            for south in north..grid.height() {
                let band = RowBand::new(grid.row(north).unwrap(), grid.row(south).unwrap());
                sums.push(table.population(band, ColSpan::FullTurn));
                for west in 0..grid.width() {
                    for east in 0..grid.width() {
                        let span = ColSpan::Through {
                            west: grid.col(west).unwrap(),
                            east: grid.col(east).unwrap(),
                        };
                        sums.push(table.population(band, span));
                    }
                }
            }
        }
        sums
    }

    fn other_order() -> ByteOrder {
        match ByteOrder::HOST {
            ByteOrder::Little => ByteOrder::Big,
            ByteOrder::Big => ByteOrder::Little,
        }
    }

    const DIGEST: u64 = 0xf17a_a802_a689_0f0c;

    /// A grid that does not run pole to pole, which is what a fixture needs to move an origin latitude or
    /// a step at all: `Grid::new` pins the origin of a whole-globe grid to the pole within the boundary
    /// tolerance, so `grid(w, h)` is refused by the constructor before any comparison is reached.
    fn sub_globe_grid(origin_lat: f64) -> Grid {
        Grid::new(
            4,
            3,
            LatLon {
                lat: origin_lat,
                lon: -180.0,
            },
            90.0,
            -10.0,
        )
        .expect("three ten-degree rows below 45 north stay on the globe")
    }

    fn identity_over(grid: Grid) -> Identity {
        Identity {
            digest: DIGEST,
            decimation: Decimation::none(grid),
        }
    }

    /// What a build settled, asked for over a grid of the same shape whose columns start half a turn
    /// away — the one geometry a caller can reach today, and the only field that differs.
    fn half_turn_from(built: &BuiltTable) -> Identity {
        let shifted = Grid::new(
            4,
            3,
            LatLon {
                lat: 90.0,
                lon: 0.0,
            },
            90.0,
            -60.0,
        )
        .expect("a 4 x 3 whole-globe grid starting at the prime meridian is valid");
        Identity {
            digest: built.digest,
            decimation: Decimation::none(shifted),
        }
    }

    #[test]
    fn an_attestation_accepts_the_identity_it_was_built_from() {
        for grid in [grid(4, 3), sub_globe_grid(45.0)] {
            let identity = identity_over(grid);
            assert_eq!(Attestation::new(&identity).check(&identity), Ok(()));
        }
    }

    /// One ground's case: the field to move, and what moving it has to be reported as.
    type Ground = (fn(&mut Attestation), Mismatch);

    #[test]
    fn each_ground_is_reported_as_the_field_that_differs() {
        let identity = identity_over(sub_globe_grid(45.0));
        let attested = Attestation::new(&identity);

        // One case per variant, so a ground added to `Mismatch` without a comparison behind it leaves
        // this list short of the enum rather than passing.
        let cases: [Ground; 8] = [
            (
                |a| a.digest ^= 1,
                Mismatch::Digest {
                    wanted: DIGEST,
                    found: DIGEST ^ 1,
                },
            ),
            (
                |a| a.width += 1,
                Mismatch::Width {
                    wanted: 4,
                    found: 5,
                },
            ),
            (
                |a| a.height += 1,
                Mismatch::Height {
                    wanted: 3,
                    found: 4,
                },
            ),
            (
                |a| a.decimation = 2,
                Mismatch::DecimationFactor {
                    wanted: 1,
                    found: 2,
                },
            ),
            (
                |a| a.origin_lat = 46.0,
                Mismatch::OriginLat {
                    wanted: 45.0,
                    found: 46.0,
                },
            ),
            (
                |a| a.origin_lon = -90.0,
                Mismatch::OriginLon {
                    wanted: -180.0,
                    found: -90.0,
                },
            ),
            (
                |a| a.lon_step = 45.0,
                Mismatch::LonStep {
                    wanted: 90.0,
                    found: 45.0,
                },
            ),
            (
                |a| a.lat_step = -20.0,
                Mismatch::LatStep {
                    wanted: -10.0,
                    found: -20.0,
                },
            ),
        ];

        for (perturb, expected) in cases {
            let mut found = attested;
            perturb(&mut found);
            assert_eq!(found.check(&identity), Err(expected));
        }
    }

    #[test]
    fn a_geometry_within_the_readers_tolerance_is_the_same_grid() {
        let identity = identity_over(sub_globe_grid(45.0));

        // The registry raster's own spelling of its origin latitude is 1.16e-11 off the pole, and the
        // raster reader accepts a file at that distance from a declared grid — so refusing a cache for
        // the same distance would be two answers to one question.
        let mut near = Attestation::new(&identity);
        near.origin_lat += 1.16e-11;
        assert_eq!(near.check(&identity), Ok(()));

        let mut far = Attestation::new(&identity);
        far.origin_lat += 1e-8;
        assert_eq!(
            far.check(&identity),
            Err(Mismatch::OriginLat {
                wanted: 45.0,
                found: 45.0 + 1e-8
            })
        );
    }

    #[test]
    fn an_origin_longitude_is_compared_through_the_seam() {
        // -180 and 180 are one meridian, so the two spellings describe the same columns and neither is
        // a stale cache.
        let identity = identity_over(grid(4, 3));
        let mut attested = Attestation::new(&identity);
        attested.origin_lon = 180.0;
        assert_eq!(attested.check(&identity), Ok(()));
    }

    #[test]
    fn a_grid_differing_only_in_its_origin_latitude_is_refused() {
        // Two grids the constructor accepts, identical but for the one number, which is the case a
        // country mask makes reachable and a whole-globe fixture cannot express.
        let attested = Attestation::new(&identity_over(sub_globe_grid(45.0)));
        assert_eq!(
            attested.check(&identity_over(sub_globe_grid(35.0))),
            Err(Mismatch::OriginLat {
                wanted: 35.0,
                found: 45.0
            })
        );
    }

    #[test]
    fn the_header_leads_with_its_format_version() {
        // On the text rather than on a parsed value, for `report.rs`'s reason: what needs pinning is
        // that a reader streaming the document meets the version before anything it might not
        // understand.
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        publish(&cache, Decimation::none(grid(4, 3)));
        let document = fs::read_to_string(cache.header_path()).unwrap();
        assert!(
            document.starts_with(r#"{"format_version":3,"#),
            "{document}"
        );
    }

    #[test]
    fn a_table_published_with_no_dataset_names_none_and_carries_no_key_for_it() {
        // The absent case as text rather than as a parsed value: what the skip promises is that the key is
        // not there at all, so a consumer distinguishing absent from null reads the document.
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let document = fs::read_to_string(cache.header_path()).unwrap();
        assert!(!document.contains("dataset"), "{document}");
        assert_eq!(cache.open(&Identity::from(&built)).unwrap().dataset(), None);
    }

    #[test]
    fn the_dataset_a_build_named_is_what_a_reader_of_that_cache_gets_back() {
        // The whole point of the field: a name resolved once, where a dataset is resolved, and read back by
        // every later command answering from this table rather than passed to each of them.
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let identity = Identity::from(&publish_named(
            &cache,
            Decimation::none(grid(4, 3)),
            Some("population-count-2020-30arcsec"),
        ));

        assert_eq!(
            cache.open(&identity).unwrap().dataset(),
            Some("population-count-2020-30arcsec")
        );
        // And it is not part of the identity: the same table is still this table, whatever it was named.
        assert!(cache.read(&identity).is_ok());
    }

    #[test]
    fn every_rectangle_survives_the_round_trip() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let decimation = Decimation::none(grid(5, 4));

        let mut as_built = Vec::new();
        let mut writer = cache.writer().unwrap();
        let built = build(source(*decimation.source()), decimation, &mut (), |row| {
            as_built.extend_from_slice(row);
            writer.write_row(row)
        })
        .unwrap();
        writer.publish(&built, None).unwrap();

        let payload = cache.read(&Identity::from(&built)).unwrap();
        let reloaded = Table::new(*decimation.grid(), payload.cells().unwrap()).unwrap();
        let original = Table::new(*decimation.grid(), &as_built).unwrap();

        // Bit-identical, not close: a payload is bytes this host wrote and bytes this host read, so
        // anything but equality is a byte that moved.
        assert_eq!(every_rectangle(&reloaded), every_rectangle(&original));
    }

    #[test]
    fn the_mapped_payload_and_the_resident_one_are_the_same_table() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let decimation = Decimation::none(grid(5, 4));
        let built = publish(&cache, decimation);
        let wanted = Identity::from(&built);

        let resident = cache.read(&wanted).unwrap();
        let mapped = cache.open(&wanted).unwrap();
        // The claim the `unsafe` has to earn: the two differ in how the bytes reach the query and in
        // nothing else, over the same pair of files.
        assert_eq!(
            every_rectangle(&Table::new(*decimation.grid(), mapped.cells().unwrap()).unwrap()),
            every_rectangle(&Table::new(*decimation.grid(), resident.cells().unwrap()).unwrap())
        );
    }

    #[test]
    fn resolving_a_digest_preserves_the_identity_cells_and_dataset() {
        for dataset in [None, Some("population-count-2020-30arcsec")] {
            let directory = TempDir::new().unwrap();
            let cache = cache_in(&directory);
            let decimation = Decimation::new(grid(6, 4), 2).unwrap();
            let built = publish_named(&cache, decimation, dataset);
            let wanted = Identity::from(&built);
            let resident = cache.read(&wanted).unwrap();

            for digest in [None, Some(built.digest)] {
                let (mapped, identity) = cache.open_resolved(decimation, digest).unwrap();
                assert_eq!(identity, wanted);
                assert_eq!(mapped.cells().unwrap(), resident.cells().unwrap());
                assert_eq!(mapped.dataset(), dataset);
            }

            assert!(matches!(
                cache.open_resolved(decimation, Some(built.digest ^ 1)),
                Err(CacheError::NotThisTable(Mismatch::Digest { wanted, found }))
                    if wanted == built.digest ^ 1 && found == built.digest
            ));
        }
    }

    fn assert_all_openers_refuse(cache: &Cache, wanted: &Identity) {
        let refusal = cache.read(wanted).unwrap_err().to_string();
        assert_eq!(cache.open(wanted).unwrap_err().to_string(), refusal);
        for digest in [None, Some(wanted.digest)] {
            assert_eq!(
                cache
                    .open_resolved(wanted.decimation, digest)
                    .unwrap_err()
                    .to_string(),
                refusal
            );
        }
    }

    #[test]
    fn resolving_a_digest_preserves_header_refusals() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));
        let wanted = Identity::from(&built);
        let original = serde_json::to_value(header_of(&built)).unwrap();
        for (field, value) in [
            ("width", serde_json::json!(5)),
            ("height", serde_json::json!(4)),
            ("decimation", serde_json::json!(2)),
            ("origin_lat", serde_json::json!(80)),
            ("origin_lon", serde_json::json!(0)),
            ("lon_step", serde_json::json!(80)),
            ("lat_step", serde_json::json!(-50)),
            ("byte_order", serde_json::to_value(other_order()).unwrap()),
        ] {
            let mut header = original.clone();
            header[field] = value;
            fs::write(cache.header_path(), serde_json::to_vec(&header).unwrap()).unwrap();
            assert_all_openers_refuse(&cache, &wanted);
        }
        for document in [
            r#"{"format_version":1}"#.to_string(),
            format!(r#"{{"format_version":{}}}"#, FORMAT_VERSION + 1),
            r#"{"format_version":3}"#.to_string(),
            r#"{"format_version": "#.to_string(),
        ] {
            fs::write(cache.header_path(), document).unwrap();
            assert_all_openers_refuse(&cache, &wanted);
        }
        write_header(&cache, &header_of(&built));
        assert_all_openers_refuse(&cache, &half_turn_from(&built));
        fs::remove_file(cache.header_path()).unwrap();
        assert_all_openers_refuse(&cache, &wanted);
    }

    #[test]
    fn resolving_a_digest_preserves_payload_length_refusals() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));
        let wanted = Identity::from(&built);
        let original = fs::read(cache.payload_path()).unwrap();
        fs::write(cache.payload_path(), &original[..original.len() - 3]).unwrap();
        assert_all_openers_refuse(&cache, &wanted);
        let mut trailing = original;
        trailing.extend_from_slice(&[0, 0, 0]);
        fs::write(cache.payload_path(), trailing).unwrap();
        assert_all_openers_refuse(&cache, &wanted);
    }

    #[test]
    fn a_digest_from_other_cells_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.attestation.digest ^= 1;
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::NotThisTable(Mismatch::Digest { wanted, found }))
                if wanted == built.digest && found == built.digest ^ 1
        ));
    }

    #[test]
    fn a_width_that_is_not_the_tables_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.attestation.width += 1;
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::NotThisTable(Mismatch::Width {
                wanted: 4,
                found: 5
            }))
        ));
    }

    #[test]
    fn a_height_that_is_not_the_tables_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.attestation.height += 1;
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::NotThisTable(Mismatch::Height {
                wanted: 3,
                found: 4
            }))
        ));
    }

    #[test]
    fn a_decimation_factor_that_is_not_the_tables_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.attestation.decimation = 2;
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::NotThisTable(Mismatch::DecimationFactor {
                wanted: 1,
                found: 2
            }))
        ));
    }

    #[test]
    fn a_format_version_from_another_release_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.format_version = FORMAT_VERSION + 1;
        // The digest goes too, and the refusal still names the version: a header of another version is
        // not a document whose other fields this build knows how to compare.
        header.attestation.digest ^= 1;
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::FormatVersion { expected, found })
                if expected == FORMAT_VERSION && found == FORMAT_VERSION + 1
        ));
    }

    #[test]
    fn a_v1_header_is_refused_for_its_version_and_not_for_its_syntax() {
        // The document verbatim, because that is the whole of what is on disk from before the geometry joined the key and
        // no struct in this build can spell it. Parsed into the widened `Header` it fails with
        // `missing field origin_lat` — `HeaderSyntax`, raised before any version is looked at — which is
        // the failure the probe exists to prevent and the one issue #45 forbids.
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let v1 = format!(
            r#"{{"format_version":1,"digest":{},"width":4,"height":3,"decimation":1,"byte_order":"little"}}"#,
            built.digest
        );
        fs::write(cache.header_path(), v1).unwrap();

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::FormatVersion {
                expected: 3,
                found: 1
            })
        ));
    }

    #[test]
    fn the_version_is_read_out_of_a_document_carrying_more() {
        // The property the probe rests on, named where it can fail: serde ignores keys a struct does not
        // declare, so `HeaderVersion` reads a header of any shape. `deny_unknown_fields` on it would
        // refuse every real document — and would fail this test, which says why, rather than a dozen
        // that do not.
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        publish(&cache, Decimation::none(grid(4, 3)));

        let document = fs::read(cache.header_path()).unwrap();
        let probed: HeaderVersion = serde_json::from_slice(&document).unwrap();
        assert_eq!(probed.format_version, FORMAT_VERSION);
        // And the document it read is one the probe declares eight keys less of.
        assert!(document.len() > br#"{"format_version":2}"#.len());
    }

    #[test]
    fn a_grid_the_table_was_not_built_over_is_refused() {
        // The reachable case, and the one nothing caught before the geometry joined the key: same width, same height, same
        // steps, same factor and the digest the build itself reported — with every column half a turn
        // from where the table's are.
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        assert!(matches!(
            cache.read(&half_turn_from(&built)),
            Err(CacheError::NotThisTable(Mismatch::OriginLon {
                wanted,
                found
            })) if wanted == 0.0 && found == -180.0
        ));
    }

    #[test]
    fn a_refusal_names_the_cache_header_and_what_differed() {
        // The wrapper's noun and the ground's numbers in one sentence, which is what the four collapsed
        // variants used to say on their own and what nothing else now pins.
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.attestation.width += 1;
        write_header(&cache, &header);
        let dimension = cache.read(&Identity::from(&built)).unwrap_err().to_string();
        assert!(dimension.contains("cache header"), "{dimension}");
        assert!(dimension.contains("wanted a 4-column table"), "{dimension}");
        assert!(dimension.contains("found 5"), "{dimension}");

        write_header(&cache, &header_of(&built));
        let geometry = cache.read(&half_turn_from(&built)).unwrap_err().to_string();
        assert!(geometry.contains("cache header"), "{geometry}");
        assert!(geometry.contains("origin longitude is 0"), "{geometry}");
        assert!(geometry.contains("found -180"), "{geometry}");
    }

    #[test]
    fn a_payload_from_a_host_of_the_other_byte_order_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.byte_order = other_order();
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::ByteOrderMismatch { expected, found })
                if expected == ByteOrder::HOST && found == other_order()
        ));
    }

    #[test]
    fn a_payload_truncated_mid_element_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut bytes = fs::read(cache.payload_path()).unwrap();
        let full = bytes.len();
        bytes.truncate(full - 3);
        fs::write(cache.payload_path(), &bytes).unwrap();

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::PayloadTruncated { expected, found })
                if expected == full && found == full - 3
        ));
    }

    #[test]
    fn a_payload_carrying_trailing_bytes_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut bytes = fs::read(cache.payload_path()).unwrap();
        let full = bytes.len();
        bytes.extend_from_slice(&[0, 0, 0]);
        fs::write(cache.payload_path(), &bytes).unwrap();

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::PayloadTrailing { expected, found })
                if expected == full && found == full + 3
        ));
    }

    #[test]
    fn a_header_that_is_not_json_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        fs::write(cache.header_path(), b"{\"format_version\": ").unwrap();

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::HeaderSyntax { .. })
        ));
    }

    #[test]
    fn a_header_whose_dimensions_disagree_with_the_payload_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        // The header and the caller agree on a wider table than the payload holds, so neither the
        // dimensions nor the digest catch it and only the payload's own length can.
        let wanted = Identity {
            digest: built.digest,
            decimation: Decimation::none(grid(6, 3)),
        };
        write_header(&cache, &Header::new(&wanted, None));

        assert!(matches!(
            cache.read(&wanted),
            Err(CacheError::PayloadTruncated {
                expected: 224,
                found: 160
            })
        ));
    }

    #[test]
    fn a_payload_published_under_an_earlier_builds_header_is_not_that_table() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let first = publish(&cache, Decimation::none(grid(4, 3)));
        // A second build's payload renamed into place while the first build's header is still there:
        // the state the publication order leaves for as long as it takes to write the header, and the
        // reason the header is the commit record rather than the payload.
        publish_payload_only(&cache, Decimation::none(grid(6, 3)));

        assert!(matches!(
            cache.read(&Identity::from(&first)),
            Err(CacheError::PayloadTrailing {
                expected: 160,
                found: 224
            })
        ));
    }

    #[test]
    fn an_interrupted_publication_is_a_missing_cache_and_its_orphan_refuses_the_next_build() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let decimation = Decimation::none(grid(4, 3));
        let built = publish_payload_only(&cache, decimation);

        // Missing rather than corrupt: nothing has committed a header, so there is no table to be
        // wrong about, and a caller's answer is to build.
        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::Absent { .. })
        ));

        // And the other half of an interruption — a temporary that never got renamed — refuses the next
        // build rather than being deleted by it: no writer can tell a temporary nobody holds from one
        // being written into.
        let orphan = cache.payload_temporary.clone();
        let leftover = b"a payload a crashed build was part way through";
        fs::write(&orphan, leftover).unwrap();

        assert!(matches!(
            cache.writer(),
            Err(CacheError::PayloadTemporaryHeld { path }) if path == orphan
        ));
        // Untouched by the refusal, which is the point: whoever is told to remove it can look at it first.
        assert_eq!(fs::metadata(&orphan).unwrap().len(), leftover.len() as u64);

        fs::remove_file(&orphan).unwrap();
        publish(&cache, decimation);
        assert!(!orphan.exists());
        assert!(cache.read(&Identity::from(&built)).is_ok());
    }

    #[test]
    fn two_builds_into_one_cache_cannot_hold_the_payload_between_them() {
        // The wrong-number path this refusal exists for: two builds on one inode write at two offsets,
        // whichever renames first publishes its own length over a mix of both their cells, and the header
        // attests the identity rather than the bytes, so nothing downstream can notice.
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);

        let first = cache.writer().unwrap();
        assert!(matches!(
            cache.writer(),
            Err(CacheError::PayloadTemporaryHeld { path }) if path == cache.payload_temporary
        ));

        // Dropping a writer publishes nothing and clears nothing, so the base stays refused until the
        // build holding it either finishes or is cleaned up by hand.
        drop(first);
        assert!(cache.writer().is_err());
    }

    #[test]
    fn a_writer_refuses_to_publish_a_payload_of_another_shape() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let decimation = Decimation::none(grid(4, 3));

        let built = build(source(*decimation.source()), decimation, &mut (), |_row| {
            Ok::<(), CacheError>(())
        })
        .unwrap();

        let mut writer = cache.writer().unwrap();
        writer.write_row(&[0.0; 5]).unwrap();

        assert!(matches!(
            writer.publish(&built, None),
            Err(CacheError::PayloadTruncated {
                expected: 160,
                found: 40
            })
        ));
    }
}
