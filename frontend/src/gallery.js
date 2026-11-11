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
