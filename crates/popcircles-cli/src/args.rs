// Every flag group the parser flattens, and the value parsers those flags name. They sit together
// because that is what a command declaration reaches for: `Command`'s variants flatten these structs
// and cite these parsers, and nothing else in the crate constructs either.
//
// A conversion here calls a library constructor and computes nothing — `GridArgs::grid` hands six
// numbers to `Grid::new` and `WindowArgs::window` hands four to `Window`. Arithmetic on a coordinate
// belongs below this crate, so a method here that grew any would be a defect rather than a helper.

use std::num::NonZeroU32;
use std::path::PathBuf;

use clap::Args;
use log::LevelFilter;
use popcircles::geodesy::{LatLon, RadiusKm};
use popcircles::grid::{Grid, GridError};
use popcircles::smallest::Share;
use popcircles::table::Window;

// `//` rather than `///`, deliberately: clap publishes a flattened struct's description
// when the command declares none, so this reasoning was the first thing `--help` showed. The words are a
// maintainer's and the level's own help text is the field's below.
//
// How much a run says about what it is doing, and the only control over it. There
// is no boolean pair beside it: two flags standing in for a threshold is the shape `FU-04` names, and its
// condition is a sweep over this directory, so spelling those two flags out here would fire it.
//
// `global` sits on the argument rather than on the `#[command(flatten)]` above, because the attribute is
// the argument's: that is what lets every subcommand take the flag after its own name without declaring
// it.
#[derive(Args, Debug, Clone, Copy)]
pub(crate) struct LogArgs {
    /// How much to report on stderr: `error`, `warn`, `info` or `debug`. It does not govern the progress
    /// meter, which answers how far a run has got rather than what happened.
    #[arg(long, global = true, default_value = "info", value_name = "LEVEL", value_parser = parse_log_level)]
    pub(crate) log_level: LevelFilter,
}

/// The four names box 5 of issue #8 gives, and no others.
///
/// Its own parser rather than `LevelFilter`'s `FromStr`, which also accepts `trace` and `off`: a level is
/// accepted here as a threshold a reader asks for, and `trace` adds none that `debug` does not already
/// give — the granularity below `debug`'s is the candidate block and the kernel, which are half a million
/// and sixteen thousand in a measured run and logged at no level.
pub(crate) fn parse_log_level(value: &str) -> Result<LevelFilter, String> {
    match value {
        "error" => Ok(LevelFilter::Error),
        "warn" => Ok(LevelFilter::Warn),
        "info" => Ok(LevelFilter::Info),
        "debug" => Ok(LevelFilter::Debug),
        other => Err(format!(
            "`{other}` is not a level; the levels are error, warn, info and debug"
        )),
    }
}

/// The range of shares a sweep walks, in whole percent.
///
/// Integers rather than fractions, and the walk is over them: a step of a tenth accumulated in f64 reaches
/// `0.30000000000000004` by its third share and publishes it. Dividing each integer by a hundred instead
/// gives the f64 a caller typing the fraction would have got, with no accumulation anywhere.
#[derive(Args, Debug, Clone, Copy)]
pub(crate) struct SweepArgs {
    /// The first share to answer, in whole percent.
    #[arg(long)]
    pub(crate) from: u32,
    /// The last share to answer, in whole percent. A share the step would carry past it is not answered.
    #[arg(long)]
    pub(crate) to: u32,
    /// How much to raise the share by between records, in whole percent.
    #[arg(long)]
    pub(crate) step: u32,
}

/// Where the radii a run settles are kept, so an interrupted run resumes instead of paying for them
/// twice.
///
/// On by default and with no way to turn it off: a ledger describing another table is refused rather than
/// resumed from, so there is nothing an opt-out would protect against.
#[derive(Args, Debug, Clone)]
pub(crate) struct LedgerArgs {
    /// The JSON document every probe's maximum is recorded in. Under `out/` beside the cache, which is
    /// gitignored.
    #[arg(long, default_value = "out/radii.json")]
    pub(crate) ledger: PathBuf,
}

#[derive(Args, Debug, Clone, Copy)]
pub(crate) struct SearchArgs {
    /// Override the initial block side in cells, derived from each radius and the grid when omitted.
    /// Changes runtime only; the answer is the same at every spacing.
    #[arg(long)]
    pub(crate) spacing: Option<NonZeroU32>,
}

/// A table that has already been built, named the way opening one needs: the grid it was declared over,
/// where the cache sits, and the digest that says which table is wanted.
///
/// Flattened into every command that reads a cached table, so those flags have one spelling and one help
/// string rather than a copy per command.
#[derive(Args, Debug, Clone)]
pub(crate) struct CachedTableArgs {
    #[command(flatten)]
    pub(crate) grid: GridArgs,
    #[command(flatten)]
    pub(crate) table: TableArgs,
    /// The digest a build reported, which is what names the table wanted.
    #[arg(long, value_parser = parse_digest)]
    pub(crate) digest: u64,
}

/// The grid a raster is declared to be. The declared grid wins over the file's own tags, which is why
/// every command naming a table asks for the same six numbers.
///
/// Two sources now, and this is the second: `table build` reads them out of a registry row rather than off
/// the command line, so a table's grid is declared once where the dataset is described and repeated on
/// every command that afterwards names that table.
#[derive(Args, Debug, Clone, Copy)]
pub(crate) struct GridArgs {
    #[arg(long)]
    pub(crate) width: u32,
    #[arg(long)]
    pub(crate) height: u32,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) origin_lat: f64,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) origin_lon: f64,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) lon_step: f64,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) lat_step: f64,
}

impl GridArgs {
    pub(crate) fn grid(self) -> Result<Grid, GridError> {
        Grid::new(
            self.width,
            self.height,
            LatLon {
                lat: self.origin_lat,
                lon: self.origin_lon,
            },
            self.lon_step,
            self.lat_step,
        )
    }
}

#[derive(Args, Debug, Clone)]
pub(crate) struct TableArgs {
    /// Both cache files are this path plus a suffix. Under `out/` by default, which is gitignored —
    /// a generated table is never committed.
    #[arg(long, default_value = "out/table")]
    pub(crate) cache: PathBuf,
    /// Fold every k by k block of cells into one table cell. Must divide both grid dimensions.
    #[arg(long, default_value_t = 1)]
    pub(crate) decimate: u32,
}

/// The rectangle a query covers. All four or none: without them the query is the table's whole extent,
/// which is not a window a pair of coordinates can express — a full turn and one column reduce alike.
#[derive(Args, Debug, Clone, Copy)]
#[group(multiple = true, requires_all = ["north", "south", "west", "east"])]
pub(crate) struct WindowArgs {
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) north: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) south: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) west: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) east: Option<f64>,
}

impl WindowArgs {
    pub(crate) fn window(self) -> Option<Window> {
        Some(Window {
            north: self.north?,
            south: self.south?,
            west: self.west?,
            east: self.east?,
        })
    }
}

/// Accepts what a build printed, `0x` and all, because the digest a query needs is copied from one
/// document into one flag.
pub(crate) fn parse_digest(value: &str) -> Result<u64, String> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    u64::from_str_radix(digits, 16)
        .map_err(|error| format!("`{value}` is not a 64-bit hexadecimal digest: {error}"))
}

/// A radius through [`RadiusKm::new`], so a negative or non-finite one is a usage error the parser
/// reports.
///
/// Exit 2 is what clap gives a usage error, which is already `EXIT_BAD_INPUT`, so `RadiusError` needs no
/// arm of its own in the classifiers below.
pub(crate) fn parse_radius(value: &str) -> Result<RadiusKm, String> {
    let km: f64 = value
        .parse()
        .map_err(|error| format!("`{value}` is not a number of kilometres: {error}"))?;
    RadiusKm::new(km).map_err(|error| error.to_string())
}

/// A share through [`Share::from_percent`], so the conversion from percent to fraction is the domain's and
/// this crate divides nothing.
pub(crate) fn parse_share(value: &str) -> Result<Share, String> {
    let percent: u32 = value
        .parse()
        .map_err(|error| format!("`{value}` is not a whole percent: {error}"))?;
    Share::from_percent(percent).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_digest_is_read_back_in_the_spelling_a_build_prints() {
        // `report.rs` prints `{:#018x}`, so this is the round trip between the two documents.
        assert_eq!(
            parse_digest("0x3a5d5e3b082f2fb7"),
            Ok(0x3a5d_5e3b_082f_2fb7)
        );
        assert_eq!(parse_digest("3a5d5e3b082f2fb7"), Ok(0x3a5d_5e3b_082f_2fb7));
        assert!(parse_digest("0x not a digest").is_err());
        // The prefix alone is not a digest either, and neither is one 17 digits long.
        assert!(parse_digest("0x").is_err());
        assert!(parse_digest("0x1f17aa802a6890f0c").is_err());
    }

    #[test]
    fn a_share_outside_a_proportion_is_refused_by_the_parser() {
        // `Share::from_percent` holds the two grounds, so this crate performs no division and no range
        // check of its own — and the message a caller sees is the domain's.
        assert!(parse_share("0").is_err());
        assert!(parse_share("101").is_err());
        assert!(parse_share("-1").is_err());
        assert!(parse_share("12.5").is_err());
    }

    #[test]
    fn a_share_in_whole_percent_is_the_fraction_a_document_publishes() {
        // Exactly, which is the whole reason the flag is percent rather than a fraction: no accumulated
        // residue reaches a published share.
        assert_eq!(parse_share("50").map(Share::get), Ok(0.5));
        assert_eq!(parse_share("100").map(Share::get), Ok(1.0));
        assert_eq!(parse_share("10").map(Share::get), Ok(0.1));
    }

    #[test]
    fn the_four_levels_parse_and_a_fifth_does_not() {
        assert_eq!(parse_log_level("error"), Ok(LevelFilter::Error));
        assert_eq!(parse_log_level("warn"), Ok(LevelFilter::Warn));
        assert_eq!(parse_log_level("info"), Ok(LevelFilter::Info));
        assert_eq!(parse_log_level("debug"), Ok(LevelFilter::Debug));

        // `LevelFilter`'s own `FromStr` takes both of these, which is why the parser is this crate's.
        assert!(parse_log_level("trace").is_err());
        assert!(parse_log_level("off").is_err());
        assert!(parse_log_level("nonsense").is_err());
    }
}
