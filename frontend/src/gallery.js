export const MAP_ASPECT_RATIO = 11 / 6;

/**
 * @typedef {object} SettlementCaption
 * @property {string} name
 * @property {string} country
 * @property {number} distance_km
 */
/**
 * @typedef {object} FigureCaption
 * @property {number} radius_km
 * @property {number} share
 * @property {number} population
 * @property {number} centre_lat
 * @property {number} centre_lon
 * @property {SettlementCaption | null} settlement
 * @property {number} [territory_share]
 * @property {string} [territory_name]
 */
/**
 * @typedef {object} GalleryFigure
 * @property {string} map
 * @property {string} document
 * @property {string | null} country_id
 * @property {string} alt
 * @property {FigureCaption} [caption]
 */
/**
 * @typedef {object} GalleryEntry
 * @property {string} name
 * @property {string} anchor
 * @property {GalleryFigure[]} figures
 * @property {boolean} [missing]
 */
/**
 * @typedef {object} GalleryData
 * @property {GalleryFigure[]} world
 * @property {GalleryEntry[]} countries
 */

/** @param {GalleryEntry[]} countries @param {string} query */
export function filterCountries(countries, query) {
  const search = query.trim().toLowerCase();
  return countries.filter((country) => country.name.toLowerCase().includes(search));
}

/** @param {GalleryEntry[]} entries @param {string} hash */
export function entryFromHash(entries, hash) {
  const anchor = hash.startsWith('#') ? hash.slice(1) : hash;
  return entries.find((entry) => entry.anchor === anchor) ?? null;
}

/** @param {GalleryEntry[]} entries @param {string | null} selectedAnchor */
export function visibleEntries(entries, selectedAnchor) {
  return entries.filter((entry) => selectedAnchor === null || entry.anchor === selectedAnchor);
}

/** @param {number | null | undefined} share @param {string | undefined} territoryName */
export function territoryCaption(share, territoryName) {
  return share === undefined || share === null
    ? null
    : `Circle covers ${Math.floor(share * 100 + 0.5)}% of ${territoryName}`;
}

/** @param {string} currentUrl @param {string | null} anchor */
export function countryUrl(currentUrl, anchor) {
  const url = new URL(currentUrl);
  url.hash = anchor ? `#${anchor}` : '';
  return url.href;
}
