# Performance

How grid resolution, saved work and search spacing affect speed, memory use and precision.
For commands and examples, see [USAGE.md][usage].

## Balancing speed, memory and precision

A detailed population map gives the search more places to put a circle's centre, but also more work to
do and more data to keep in memory. The aim is to keep the detail you need while avoiding unnecessary
work. The program deals with this in three ways:

- **Choosing grid resolution.** Combining map cells reduces memory use and search time, at the cost of
  less precise centre placement. In one measured search, a coarser grid took 18 seconds instead of
  78 minutes and placed the centre about 0.6 km from the full-resolution result. Both found the same
  radius.
- **Preparing and reusing work.** A population lookup table makes population counts quick. A ledger
  saves completed searches at particular radii. Both cost work to create, then save work on later uses.
- **Reducing the search work.** The program rules out groups of possible centres without checking each
  one. Spacing controls the starting group size. It changes the amount of work, not the answer.

The question you ask also matters. Counting people inside one circle takes less work than finding the
best centre across the map. Finding the smallest circle for a population target requires searches at
several radii. Larger circles can make each of those searches more expensive.

## Choosing grid resolution

### How resolution affects precision

The search places circle centres at the centres of map cells. A finer grid offers more possible
locations; a coarser grid offers fewer. Combining cells therefore reduces the work, but can move the
best centre and change the population counted inside its circle. The smallest radius reaching a target
can change too, although it stayed the same in the example below.

The source population map has 43200 × 21600 cells, about 933 million in total, each roughly 1 km across
at the equator. Decimation combines groups of these cells, adding their populations together. A value
of `10` combines each 10 × 10 group into one cell, leaving a 4320 × 2160 map.

Grid resolution is controlled by `--decimate`. For example, a value of `2` combines each 2 × 2 group of
source cells into one cell, leaving a quarter as many cells. Fewer cells mean a smaller lookup table,
fewer possible centres and fewer map rows to read for each circle. See [Choosing the inputs][inputs]
for practical guidance.

Two other limits matter when interpreting precision. Smallest-circle searches test radii in whole
kilometres, so they establish that one integer radius reaches the target while the kilometre below it
does not. A finer grid gives more centre locations, but does not make these radius steps smaller.

Adding many population values also introduces small rounding errors. The program calculates an error
bound; in the half-world example at `--decimate 2` (1 arcmin cells), it is about 0.06 of a person out of
3.9 billion. That is much smaller than the effect of grid resolution in this example. When rounding
uncertainty could affect whether a radius reaches a target, the result reports the uncertainty rather
than claiming a definite smallest radius. See [the smallest-circle example in USAGE][smallest] for
how to read that output.

### When the table exceeds available memory

A larger table can slow the search by much more than its extra cell count suggests. The saved table is
read through memory: the operating system keeps parts of it in memory and loads others from disk as
needed. When the table fits in available memory, repeated reads are fast. When it does not, the program
may keep waiting for data to arrive from disk.

In the full-resolution run below, the table was 7 GB and the search spent about 94% of its time waiting
rather than calculating. The 1.8 GB table fit in memory on that machine. Available memory depends on
what the operating system and other applications are using, as well as the machine's total memory.

### Measured trade-offs

These historical measurements cover building a table and finding the smallest circle containing half
of the world's population. They were made on a 16 GB M2 Pro (6 performance and 4 efficiency cores,
internal SSD) under macOS 26.6. They illustrate the trade-off; they are not timing guarantees for another
machine or search.

| `--decimate` | Cell size | Table | Build | Search | Radius | Centre difference |
| --- | --- | --- | --- | --- | --- | --- |
| 10 | 9.3 km | 71 MB | 11 s | 0.9 s | 3360 km | 6.3 km |
| 4 | 3.7 km | 445 MB | 15 s | 5.0 s | 3360 km | 1.8 km |
| **2** | **1.9 km** | **1.8 GB** | **16 s** | **18 s** | **3360 km** | **0.6 km** |
| 1 | 0.9 km | 7.0 GB | 21 s | 78 min | 3360 km | — (the reference) |

Cell sizes are approximate widths at the equator. Centre differences are distances from the
full-resolution centre, rather than errors against a known true location between cells.

All four settings found the same 3360 km radius. Moving from `--decimate 2` to full resolution took
roughly 250 times longer for a centre shift of about 0.6 km, because the larger table no longer fit in
available memory. The full-resolution run tested 24 radii, taking 100–275 seconds per radius; its output
is the [recorded full-resolution result][world-half-result].

## Preparing and reusing work

Reducing resolution makes each search cheaper. Saving work helps when the same data or search results
will be needed again. The lookup table and ledger save work at different stages.

### How the population lookup table saves time

A search repeatedly needs the population inside an area. Adding the original map cells every time
would repeat much of the same work. The lookup table, also called a summation table, stores running
population totals. Those totals let the program calculate the population in a rectangle with four table
lookups, regardless of how many cells the rectangle contains.

A circle is counted as a series of narrow rectangles, one per map row it crosses. The table therefore
helps both a single population query and the many circle counts needed during a search.

Building the table takes time and disk space, but it can serve many commands without being rebuilt.
The [table instructions in USAGE][table-build] cover building and using it.

### Reusing a table safely

Search and query commands check that a saved table's grid and decimation match the requested settings.
The build also prints a digest, an identifier for the population cells in that table. Pass it with
`--digest` to require a matching identifier. Without `--digest`, the program trusts the saved table's
header and does not reopen the source raster to check it. Use the digest when you need to confirm that
a command is using a particular table.

While a table is being written, the program creates a temporary `.payload.bin.tmp` file. If that file
already exists, a new build stops rather than risking two builds writing to the same cache or
publishing a partial table. This can mean a build is still running or was interrupted. Once you have
confirmed that no build is running, remove the temporary file and run the build again.

### How the ledger avoids repeated searches

Finding the smallest circle for a population target means trying several radii. At each radius, the
program finds the circle containing the most people, then checks whether that population reaches the
target. A ledger saves the best centre and population found at each completed radius.

That result is useful for any population target on the same table. For example, once the best circle
at 1000 km is known, both a 10% search and a 30% search can compare their targets against it without
searching the map at that radius again. A sweep shares these results across its requested targets.
In one historical run at `--decimate 10`, ten population targets needed only 28 distinct radius searches
and took 7.0 seconds in total.

Completed radius results are saved before the next radius is searched, so later runs can reuse them
after an interruption. Changing spacing does not invalidate them, because spacing does not change the
best circle. A different table, or a different country for a country search, requires its own results.
See [the sweep example in USAGE][sweep] for commands and ledger options.

## Reducing the search work

Even with quick population counts, checking every possible centre would be expensive. The search
avoids that by checking whether whole groups of centres can be ruled out. Spacing controls where this
process starts; it does not limit how closely the search examines promising areas.

### Ruling out groups of possible centres

At full resolution, the world map has about 933 million possible cell centres. The search starts by
grouping nearby centres into square blocks.

For each block, it picks a representative centre and calculates how far any centre in the block could
be from it. It then expands the requested circle by that distance. This larger circle covers every
possible circle centred in the block, so its population gives a safe upper limit for the block.

For example, if even the expanded circle contains fewer people than the best circle already found,
none of the centres in that block can improve the answer. The whole block can be skipped.

If the block might match or beat the best result, the search divides it into smaller blocks and checks
those. It continues until each possible centre has either been checked or safely ruled out. Equal
results are kept so the search's tie-breaking rule still applies.

### How circle radius affects the work

A larger circle crosses more map rows, so counting its population needs more rectangle lookups. For
example, a 3300 km circle spans about 356 rows of the grid made with `--decimate 10`, compared with
7130 rows at full resolution.

Large circles can also make groups harder to rule out. An expanded circle may contain most or all of
the world's population, even when none of the requested circles in its block does. Its upper limit
then tells the search little, so the block needs to be split further.

### How automatic spacing is chosen

Spacing is the side length of the starting blocks, measured in map cells. The program chooses it from
the grid's cell size and the requested circle radius.

The starting blocks should be large enough to group many centres, but not so large that their expanded
circles cover the whole globe. A circle with a radius of about 20,000 km, half the Earth's circumference,
can reach every point on Earth. Its population limit cannot help rule out any block.

The program therefore chooses the largest block size whose expanded circle stays below that limit.
To estimate the expansion safely for blocks anywhere on Earth, it adds the block's north-south reach
to its east-west reach, using the equator where east-west cells are widest.

A larger requested radius leaves less room for expansion and therefore usually needs smaller starting
blocks. A finer grid can fit more cells into the same ground distance, so its spacing in cells can be
larger. The program recalculates spacing for each tested radius; a smallest-circle search can use
several starting sizes during one run.

If the requested radius already reaches the whole globe, the spacing setting uses one block. The
smallest-circle search can answer that radius directly from the table's total population.

### When to override spacing

Most users can leave `--spacing` unset. An override is useful when comparing how block sizes affect
runtime on a particular map and radius:

- A small value starts with many small blocks. Their population limits are tighter, but there are more
  blocks to check.
- A large value starts with fewer, larger blocks. Their population limits are broader, so more blocks
  may need to be split and checked further.

There is no value that is always fastest. A value of `1` checks candidate cell centres individually,
while a value larger than the grid starts with one block. Every setting still refines promising blocks
to individual cells, so changing spacing changes the work rather than the result.

When comparing spacing settings, use a radius that needs a fresh search. If a ledger already holds
that radius, the program reuses its result and does not exercise either spacing setting.

<!-- Links -->

[inputs]: USAGE.md#choosing-the-inputs
[smallest]: USAGE.md#the-smallest-circle-holding-a-share
[sweep]: USAGE.md#a-sweep-of-shares-over-one-ledger
[table-build]: USAGE.md#building-a-population-lookup-table
[usage]: USAGE.md
[world-half-result]: results/world-half-30arcsec.json
