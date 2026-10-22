---
id: 0016
status: accepted
date: 2026-10-22
scope: architecture
tags: [adr, popcircles]
---

# ADR 0016 — Country population queries use cropped prefix tables

## Decision

Country population queries use a masked summation table cropped to the selected domain's bounding
rectangle. Build and consume country tables sequentially, so working storage is set by the largest
domain being processed rather than by all domains together. Country totals and circle populations use
the same membership and source-aggregation policy.

## Context

The route must serve both #18's repeated country searches and #73's fixed-circle composition across
263 GISCO features. On the measured k=10 searches, cropped 100% runs took 5.076 seconds for Indonesia
and 5.006 seconds for Chile, against 261.949 and 353.435 seconds direct; the k=1 #73 crop workload
took 65.23 seconds per run, close to direct streaming's 66.21 seconds. The route comparison, storage
counts and limits are recorded in [#72](https://github.com/turboBasic/PopulationCircles2026/issues/72#issuecomment-6340314761).

The k=1 cropped #73 run retained one 2.17 GB maximum prefix at a time, with 12.76 GB of padded
prefixes across all features if retained. Counted source values, prefix and membership intervals
reached 6.04 GB sequentially; geometry and runtime overhead were not measured as RSS. Direct
streaming used 345,600 bytes for raster rows and matched the one-shot workload, but its country
searches were far slower. A whole-grid prefix is 7.47 GB for one country and 1.96 TB across 263,
before source values and masks. A k=1 per-country smallest-circle search was not measured; #18 asks
for full resolution only on a named handful, not as a sweep.

## Options

### Cropped prefix tables (SELECTED)

- Adopted because: 100% searches were about 52 times faster for Indonesia and 70 times faster for Chile than direct queries.
- Adopted because: one sequential cropped-prefix pass served the full-resolution #73 workload in the same wall-time class as direct streaming.
- Adopted despite: counted source, prefix and membership-interval storage reached 6.04 GB sequentially; RSS was unavailable.
- Adopted despite: retaining every padded crop would use 12.76 GB, so country tables must be consumed sequentially.

### Whole-grid masked prefix tables

- Rejected because: one full-resolution prefix plus the prototype's source values and mask is 15.86 GB; all 263 prefixes total 1.96 TB.
- Rejected because: the k=10 whole-grid fixed-circle query was 21.459 ms against 1.586 ms cropped.
- Rejected despite: one grid-wide layout makes every country query use the same coordinate extent.

### Direct masked queries

- Rejected because: the k=10 direct 100% searches took 261.949 seconds for Indonesia and 353.435 seconds for Chile.
- Rejected despite: direct streaming completed the k=1 #73 workload in 66.21 seconds with 345,600 bytes of raster-row storage.

## Consequences

Country tables are built and released one domain at a time. Their identity must cover every source,
resolution, aggregation and membership choice that changes their cells; a mismatch is rebuilt rather
than reused. The route does not assign unclaimed cells or resolve overlapping GISCO features. Reopen
this choice if full-resolution country workloads show that prefix construction costs more than the
query reuse saves.

## Links

- Issue #72 — route measurements and reproduction.
- ADR 0005 — derived artefacts are keyed on their inputs.
- ADR 0015 — country identities follow GISCO features.
