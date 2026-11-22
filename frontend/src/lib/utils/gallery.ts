import type { GalleryEntry } from '#lib/types/gallery';

export function filterCountries(countries: GalleryEntry[], query: string) {
  const search = normalizeCountryName(query.trim());
  return countries.filter((country) => normalizeCountryName(country.name).includes(search));
}

function normalizeCountryName(name: string) {
  return name
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase();
}

export function sortCountries(countries: GalleryEntry[]) {
  return [...countries].sort((left, right) =>
    left.name.localeCompare(right.name, 'en', { sensitivity: 'base' }),
  );
}

export function groupCountries(countries: GalleryEntry[]) {
  return Object.groupBy(sortCountries(countries), (country) =>
    normalizeCountryName(country.name).charAt(0).toUpperCase(),
  );
}

export function entryFromHash(entries: GalleryEntry[], hash: string) {
  const anchor = hash.startsWith('#') ? hash.slice(1) : hash;
  return entries.find((entry) => entry.anchor === anchor) ?? null;
}

export function territorySharePercent(share: number | null | undefined) {
  return share === undefined || share === null ? null : Math.floor(share * 100 + 0.5);
}

export function countryUrl(currentUrl: string, anchor: string | null) {
  const url = new URL(currentUrl);
  url.hash = anchor ? `#${anchor}` : '';
  return url.href;
}
