//! The published shape of a result, and the only place in this crate a serde derive appears. ADR 0004:
//! the domain types change when the search changes, so what is serialised is a separate
//! representation with its own version, and a field here is a promise to two renderers and two command
//! surfaces.
//!
//! # The envelope
//!
//! Every document is an [`Envelope`], whose keys come in declaration order because that is the order serde
//! emits them in. `schema_version` is first so a consumer reads the version before anything it might not
//! understand, and `provenance` precedes `result` for the same reason one step out: what produced a
//! document is read before the document.
//!
//! # The earth model, and why it is published
//!
//! [`EarthModel`] attests **which sphere every distance in this document was measured on**. A radius, a
//! great-circle distance and a cap's boundary are all that number's, so a consumer drawing the answer on
//! another earth model is drawing a different figure — and drawing it without complaint, which is why the
//! model is a field rather than a thing a renderer assumes. `geodesy.rs` owns the model
//! ([`EARTH_RADIUS_KM`]); this block is a publication of that owner, so a consumer needs no copy of it.
//!
//! # Provenance, and what it does not attest
//!
//! [`Provenance`] is where a document's **identity** lives: which table it was answered from, where that
//! table sits, and which registered dataset its cells came from. It is absent — the key omitted, not null —
//! from a document whose command read no cached table.
//!
//! A payload's own `digest` or `grid` is a different thing and not a second answer to the same question.
//! [`TableQueryReport`] is the one place the distinction is visible: it carries both in its payload and
//! carries no provenance, because there the table is what the command is *about* rather than what it was
//! answered from.
//!
//! **The `grid` in provenance is attested, in the same sense `digest` and `decimation` are.** The cache
//! header binds the whole geometry, per ADR 0005, so opening one compares it and a table built over
//! another grid is refused rather than answered from. What a document publishes is the caller's spelling
//! of that geometry, which the header accepted within the tolerance the raster reader grants.
//!
//! # The documents
//!
//! `kind` is the envelope's `document` key, which is what a consumer branches on before it reads anything
//! under `result`. [`Document`] is where a payload type declares it.
//!
//! | Payload | `kind` | What it answers |
//! | --- | --- | --- |
//! | [`DistanceReport`] | `distance` | a great-circle distance between two coordinates |
//! | [`GridSummary`] | `grid` | the geometry of a declared grid |
//! | [`TableBuildReport`] | `table-build` | what a summation table build settled, and where it went |
//! | [`TableQueryReport`] | `table-query` | the population of one rectangle of a table |
//! | [`CircleReport`] | `circle` | the population inside one circle a caller named |
//! | [`MostPopulousReport`] | `most-populous` | the most populous circle of a fixed radius |
//! | [`SmallestReport`] | `smallest-circle` | one smallest circle, with no ledger around it |
//! | [`SmallestDocument`] | `smallest` | the smallest circle reaching one share |
//! | [`SweepDocument`] | `sweep` | the smallest circle for each of a range of shares |
//! | [`CountryListDocument`] | `country-list` | the available country identifiers and names |
//! | [`CountrySmallestDocument`] | `country-smallest` | one country's smallest circle and its source identity |
//!
//! `SmallestDocument` and `SweepDocument` wrap their circles because a ledger belongs to the run, not to
//! one circle. `CountrySmallestDocument` also carries the feature identity that produced its answer.
//! `smallest-circle` is the circle inside these documents, published on its own by a snapshot rather than
//! by a command, and it carries a kind because a payload type that can be wrapped is one a consumer can be
//! handed.
//!
//! **A [`SweepDocument`]'s `records` ascend by `target.share`.** That is part of the contract and held by
//! construction, so a renderer plotting share against radius may read them in the order it gets them.
//!
//! # What these numbers are accurate to
//!
//! This sits here rather than in a document of its own: what a published figure is
//! accurate to is what a consumer of this format needs to know, and every field it composes is one this
//! module publishes. Nothing below is a tolerance this crate applied — no candidate is ever discarded by
//! one — so each is a bound on the arithmetic beneath an answer, not a margin around it.
//!
//! **A population is accurate to the slack the document carries.** One rectangle query is within 4 ulp of
//! the magnitude it sums, which is about 4e-6 persons at the registry raster's
//! 7.76e9. A circle is a sum of one such query per grid row it spans, added in
//! [`crate::circle::population`]'s fixed order, so the error composes rather than cancelling:
//! [`crate::smallest::predicate_slack_persons`] is that composition, and it is what
//! `predicate_slack_persons` publishes. Measured on this dataset's half-world circle it is 0.012 persons
//! over the 5 arcmin grid and 0.12 over the 30 arcsec one — against populations of 3.9e9, which is eleven
//! significant figures.
//!
//! **`tolerance_persons` is zero, and it means what it says.** The fixed-radius search reports the exact
//! maximum over the grid's cell centres: refinement runs to single cells, the pruning bound is rounded
//! outward and no tie is discarded (issue #6). So the figure is not "zero because nobody measured" — it is
//! the statement that nothing was traded away, and what separates a reported maximum from the truth is the
//! summation slack above and nothing else.
//!
//! **A radius is a whole kilometre, and the answer is a bracket rather than a point.** The search over
//! radius steps in kilometres, so `radius_km` is the smallest whole kilometre that reaches the target and
//! `short_below` is the kilometre beneath it that does not. Both were measured. Where a probe's margin
//! falls inside the slack the comparison could have gone either way, and then `ambiguity` is present and
//! names the span of probed radii that cannot be separated — a floor on it, because the climb doubles and
//! the radii between two probes were never measured, which is ADR 0007. An absent `ambiguity` is the
//! stronger statement.
//!
//! **The centre is a cell centre, and that is a property of the question rather than an error in the
//! answer.** Every search maximises over the grid's cell centres, so a circle whose centre lies between
//! them is not a candidate: at 30 arcsec that is a 926 m mesh at the equator, at 5 arcmin a 9.3 km one. A
//! consumer wanting the continuum's answer should read a published centre as the best cell and the grid in
//! `provenance` as the resolution that claim is made at. [`CircleReport`] is where the distinction is
//! visible, publishing the coordinate asked for beside the cell it was snapped to.
//!
//! **Distances are on the sphere `earth_model` names**, which is the section above and the reason that
//! field is not optional.
//!
//! # Growth
//!
//! The format grows **additively**: a new field or a new payload type owes no version bump, and
//! [`SCHEMA_VERSION`] rises only for a change an existing consumer would misread — a renamed or removed
//! field, or one whose meaning moved. A consumer should therefore ignore keys it does not know rather than
//! refuse a document carrying them.
//!
//! A present `country_breakdown` lists intersecting domains by descending population contribution,
//! then canonical identifier. Its ratios are unit fractions: inside / country total and inside / circle
//! population. Reconciliation carries the circle-specific remainder and arithmetic bound; provenance
//! carries boundary credit and membership, ownership and aggregation context. An empty `entries` list
//! means accounting ran and found no intersecting domain.

use std::path::Path;

use serde::Serialize;

... (+2000 lines) [see remaining: rtk proxy git -c core.fsmonitor=false show HEAD:crates/popcircles/src/report.rs | tail -n +122]
