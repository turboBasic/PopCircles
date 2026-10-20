// Grid geometry with no raster behind it: the six declared numbers, checked and described.

use popcircles::report::{Envelope, GridSummary};

use crate::args::DatasetGridArgs;
use crate::commands::serialised;
use crate::failure::Failure;

pub(crate) fn describe_grid(args: &DatasetGridArgs) -> Result<String, Failure> {
    let grid = args.grid()?;
    serialised(serde_json::to_string(&Envelope::new(GridSummary::from(
        &grid,
    ))))
}
