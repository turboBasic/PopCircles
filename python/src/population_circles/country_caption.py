from population_circles.circle_document import Answer


def lines_for(answer: Answer) -> list[str]:
    breakdown = answer.caption.country_breakdown
    if breakdown is None:
        return []

    lines = ["Country population on the accounting grid (not land area):"]
    if breakdown.entries:
        lines.extend(
            f"{entry.name} ({entry.id}): {entry.population_inside:,.6g} people; "
            f"{_percent(entry.fraction_of_country_population)} of country population; "
            f"{_percent(entry.fraction_of_circle_population)} of circle population"
            for entry in breakdown.entries
        )
    else:
        lines.append("No country domains intersect this circle on the accounting grid.")

    provenance = breakdown.provenance
    grid = provenance.effective_grid
    lines.extend(
        [
            (
                f"Unassigned: {breakdown.reconciliation.unassigned_population:,.6g} people "
                f"(bound ±{breakdown.reconciliation.bound_persons:,.3g}); cells without one unique "
                "owner, including overlaps, remain here."
            ),
            (
                f"Accounting: {provenance.boundary_dataset}, {provenance.boundary_revision}; "
                f"{grid.width} x {grid.height} cells, decimation k={provenance.decimation}; "
                f"membership/ownership/aggregation rules "
                f"{provenance.membership_rule_version}/{provenance.ownership_rule_version}/"
                f"{provenance.aggregation_order_version}; {provenance.accounting_route}."
            ),
            f"Boundary credit: {provenance.attribution}",
        ]
    )
    return lines


def _percent(fraction: float | None) -> str:
    return "unavailable" if fraction is None else f"{fraction:.1%}"
