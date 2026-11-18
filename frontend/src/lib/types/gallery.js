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
 * @property {boolean} missing
 */
/**
 * @typedef {object} GalleryData
 * @property {GalleryFigure[]} world
 * @property {GalleryEntry[]} countries
 */

export {};
