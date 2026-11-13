export function filterCountries(countries, query, includeZeroPopulation = false) {
  const search = query.trim().toLowerCase();
  return countries.filter(
    (country) =>
      (includeZeroPopulation || !country.missing) &&
      country.name.toLowerCase().includes(search)
  );
}

export function defaultCountry(countries) {
  return countries.find((country) => country.name === 'Germany') ?? countries[0] ?? null;
}

export function countryFromHash(countries, hash, fallback) {
  const anchor = hash.startsWith('#') ? hash.slice(1) : hash;
  return countries.find((country) => country.anchor === anchor) ?? fallback;
}

export function visibleCountries(countries, selectedAnchor, expanded, includeZeroPopulation = false) {
  return (expanded ? countries : countries.filter((country) => country.anchor === selectedAnchor))
    .filter((country) => includeZeroPopulation || !country.missing);
}

export function countryUrl(currentUrl, anchor) {
  const url = new URL(currentUrl);
  url.hash = anchor ? `#${anchor}` : '';
  return url.href;
}
