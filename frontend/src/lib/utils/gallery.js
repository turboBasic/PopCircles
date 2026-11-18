export const MAP_ASPECT_RATIO = 11 / 6;

/** @typedef {import('#lib/types/gallery.js').GalleryEntry} GalleryEntry */

/** @param {GalleryEntry[]} countries @param {string} query */
export function filterCountries(countries, query) {
  const search = normalizeCountryName(query.trim());
  return countries.filter((country) => normalizeCountryName(country.name).includes(search));
}

/** @param {string} name */
function normalizeCountryName(name) {
  return name
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase();
}

/** @param {GalleryEntry[]} countries */
export function sortCountries(countries) {
  return [...countries].sort((left, right) =>
    left.name.localeCompare(right.name, 'en', { sensitivity: 'base' }),
  );
}

/** @param {GalleryEntry[]} countries */
export function groupCountries(countries) {
  return Object.groupBy(sortCountries(countries), (country) =>
    normalizeCountryName(country.name).charAt(0).toUpperCase(),
  );
}

/** @param {GalleryEntry[]} entries @param {string} hash */
export function entryFromHash(entries, hash) {
  const anchor = hash.startsWith('#') ? hash.slice(1) : hash;
  return entries.find((entry) => entry.anchor === anchor) ?? null;
}

/** @param {number | null | undefined} share */
export function territorySharePercent(share) {
  return share === undefined || share === null ? null : Math.floor(share * 100 + 0.5);
}

/** @param {string} currentUrl @param {string | null} anchor */
export function countryUrl(currentUrl, anchor) {
  const url = new URL(currentUrl);
  url.hash = anchor ? `#${anchor}` : '';
  return url.href;
}
