export function filterCountries(countries, query, includeZeroPopulation = false) {
  const search = query.trim().toLowerCase();
  return countries.filter(
    (country) =>
      (includeZeroPopulation || !country.missing) &&
      country.name.toLowerCase().includes(search)
  );
}

export function entryFromHash(entries, hash) {
  const anchor = hash.startsWith('#') ? hash.slice(1) : hash;
  return entries.find((entry) => entry.anchor === anchor) ?? null;
}

export function visibleEntries(entries, selectedAnchor, includeMissing = false) {
  return entries
    .filter((entry) => selectedAnchor === null || entry.anchor === selectedAnchor)
    .filter((entry) => entry.anchor === selectedAnchor || includeMissing || !entry.missing);
}

export function countryUrl(currentUrl, anchor) {
  const url = new URL(currentUrl);
  url.hash = anchor ? `#${anchor}` : '';
  return url.href;
}
