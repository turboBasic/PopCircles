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

export function countryFromHash(countries, hash, fallback) {
  return entryFromHash(countries, hash) ?? fallback;
}

export function defaultCountry(countries) {
  return countries.find((country) => country.name === 'Germany') ?? countries[0] ?? null;
}

export function visibleEntries(entries, selectedAnchor, includeMissing = false) {
  return entries
    .filter((entry) => selectedAnchor === null || entry.anchor === selectedAnchor)
    .filter((entry) => includeMissing || !entry.missing);
}

export function visibleCountries(countries, selectedAnchor, expanded, includeZeroPopulation = false) {
  return visibleEntries(countries, expanded ? null : selectedAnchor, includeZeroPopulation);
}

export function countryUrl(currentUrl, anchor) {
  const url = new URL(currentUrl);
  url.hash = anchor ? `#${anchor}` : '';
  return url.href;
}
