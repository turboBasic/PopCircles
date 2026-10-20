// What a command is, and the three pieces more than one of them needs. A command takes resolved flags,
// calls the library, and hands back the JSON string `main` prints — so this module and its children hold
// no argument parsing, no exit-code choice and no stream.
//
// `CachedTable` is the one place a cache is opened, which is why a command asks for a table rather than
// assembling the path, the identity and the provenance of one itself.

pub(crate) mod distance;
pub(crate) mod grid;
pub(crate) mod search;
pub(crate) mod table;

use std::fs;
use std::path::{Path, PathBuf};

use popcircles::bracket::Bracket;
use popcircles::grid::Grid;
use popcircles::report::Provenance;
use popcircles::table::cache::{Cache, Identity, Mapped};
use popcircles::table::{Decimation, Table};

use crate::args::CachedTableArgs;
use crate::failure::{EXIT_FAILURE, Failure};

/// A cached table, opened and mapped, with the provenance a document declares it by.
///
/// The mapping is held rather than the cells, because a [`Table`] borrows from it: keeping the two
/// together is what ties the lifetime of every query to the mapping through the compiler. Which is also
/// why [`Self::table`] is a method and not something the resolver returns — a value cannot hold a borrow
/// of its own field.
#[derive(Debug)]
pub(crate) struct CachedTable {
    pub(crate) grid: Grid,
    pub(crate) identity: Identity,
    mapped: Mapped,
    header: PathBuf,
    payload: PathBuf,
}

impl CachedTable {
    /// Resolves the flags to a mapped table and the identity checked by its header.
    ///
    /// The one place a command reads a cache, so a command asks for a table rather than assembling the
    /// path, the identity and the provenance of one for itself.
    pub(crate) fn open(args: &CachedTableArgs) -> Result<Self, Failure> {
        // Box 7's other half of "table build or load". The three `?`s below are exactly why the closing
        // record is `Drop`'s: a cache that is absent still says how long finding that out took.
        let _bracket = Bracket::open(module_path!(), "table load");

        let source = args.grid.grid()?;
        let decimation =
            Decimation::new(source, args.table.decimate).map_err(|error| Failure::table(&error))?;
        let cache = Cache::new(&args.table.cache);
        let (mapped, identity) = cache
            .open_resolved(decimation, args.digest)
            .map_err(|error| Failure::cache(&error))?;

        // Box 6's resolved input, here because this is already the one place a cache is opened. It names
        // what a reader would otherwise reconstruct from four flags: which table, from where, and at what
        // shape after the fold.
        let grid = decimation.grid();
        log::info!(
            "table {:#018x} opened from {}: {} x {} cells, decimated by {}",
            identity.digest,
            args.table.cache.display(),
            grid.width(),
            grid.height(),
            decimation.factor()
        );

        Ok(Self {
            grid: *grid,
            identity,
            mapped,
            header: cache.header_path().to_path_buf(),
            payload: cache.payload_path().to_path_buf(),
        })
    }

    /// The table over the mapping, and the only place in this crate one is constructed.
    pub(crate) fn table(&self) -> Result<Table<'_>, Failure> {
        let cells = self
            .mapped
            .cells()
            .map_err(|error| Failure::cache(&error))?;
        Table::new(self.grid, cells).map_err(|error| Failure::table(&error))
    }

    pub(crate) fn provenance(&self) -> Provenance {
        // The name comes off the header rather than off a flag, which is what lets a command that takes no
        // `--dataset` publish one.
        Provenance::new(
            &self.identity,
            self.mapped.dataset(),
            &self.header,
            &self.payload,
        )
    }
}

/// Makes the directory a file this crate is about to write will live in.
///
/// Resolving where a generated file goes, and making room for it, is the shell's work — the library is
/// handed a path and never asked where one should be. Both the cache and the ledger want it, which is why
/// it is a function rather than a step inside either.
pub(crate) fn make_room_for(file: &Path) -> Result<(), Failure> {
    let Some(parent) = file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|error| Failure {
        code: EXIT_FAILURE,
        message: format!(
            "the directory {} could not be made: {error}",
            parent.display()
        ),
    })
}

pub(crate) fn serialised(json: serde_json::Result<String>) -> Result<String, Failure> {
    json.map_err(|error| Failure::new(EXIT_FAILURE, &error))
}
