export function filterCountries(countries, query) {
  const search = query.trim().toLowerCase();
  return countries.filter((country) => country.name.toLowerCase().includes(search));
}

export function defaultCountry(countries) {
  return countries.find((country) => country.name === 'Germany') ?? countries[0] ?? null;
}

export function countryFromHash(countries, hash, fallback) {
  const anchor = hash.startsWith('#') ? hash.slice(1) : hash;
  return countries.find((country) => country.anchor === anchor) ?? fallback;
}

export function countryMapSource(map, selectedAnchor, countryAnchor, expanded) {
  return expanded || selectedAnchor === countryAnchor ? map : null;
}

export function countryUrl(currentUrl, anchor) {
  const url = new URL(currentUrl);
  url.hash = anchor ? `#${anchor}` : '';
  return url.href;
}
