import { expect, test } from 'vitest';
import {
  entryFromHash,
  countryUrl,
  filterCountries,
  sortCountries,
  groupCountries,
  territorySharePercent,
} from '#lib/utils/gallery.js';

/** @param {{name: string, anchor: string, missing?: boolean}[]} values */
function makeCountries(values) {
  return values.map(({ name, anchor, missing = false }) => ({
    name,
    anchor,
    figures: [],
    missing,
  }));
}

const countries = makeCountries([
  { name: 'Alpha', anchor: 'country-result-AA' },
  { name: 'Germany', anchor: 'country-result-DE' },
  {
    name: 'No population <yet>',
    anchor: 'country-missing-No%20population%20%3C' + 'yet%3E',
    missing: true,
  },
]);
const entries = [
  { name: 'World', anchor: 'world-result', figures: [], missing: false },
  ...countries,
];

// --- Country list ---

test('filters countries by trimmed, case-insensitive names, including missing entries', () => {
  expect(filterCountries(countries, '  ALP ')).toEqual([countries[0]]);
  expect(filterCountries(countries, '')).toEqual(countries);
  expect(filterCountries(countries, 'population')).toEqual([countries[2]]);
});

test('sorts and groups countries alphabetically and counts normalized substring matches', () => {
  const unsorted = makeCountries([
    { name: '\u00c9ire', anchor: 'ie' },
    { name: 'belgium', anchor: 'be' },
    { name: '\u00c5land', anchor: 'ax' },
    { name: 'Canada', anchor: 'ca' },
  ]);
  expect(sortCountries(unsorted)).toEqual([unsorted[2], unsorted[1], unsorted[3], unsorted[0]]);
  expect(Object.keys(groupCountries(unsorted))).toEqual(['A', 'B', 'C', 'E']);
  expect(groupCountries(unsorted).A).toEqual([unsorted[2]]);
  expect(filterCountries(unsorted, '  IRE ')).toEqual([unsorted[0]]);
  expect(filterCountries(unsorted, 'land')).toEqual([unsorted[2]]);
  expect(filterCountries(unsorted, 'a')).toHaveLength(2);
  expect(filterCountries(unsorted, '')).toEqual(unsorted);
  expect(filterCountries(unsorted, 'zz')).toEqual([]);
});

// --- URL and display text ---

test('[navigation-and-history] resolves World and country entries from URL fragments, with the base URL unselected', () => {
  expect(entryFromHash(entries, '#world-result')).toBe(entries[0]);
  expect(entryFromHash(entries, '#country-result-AA')).toBe(countries[0]);
  expect(entryFromHash(entries, '#country-missing-No%20population%20%3C' + 'yet%3E')).toBe(
    countries[2],
  );
  expect(entryFromHash(entries, '#unknown')).toBeNull();
  expect(entryFromHash(entries, '')).toBeNull();
});

test('copies the selected country fragment without keeping an old fragment', () => {
  expect(countryUrl('https://example.test/gallery/?view=maps#old', 'country-result-AA')).toBe(
    'https://example.test/gallery/?view=maps#country-result-AA',
  );
});

test('rounds territory coverage to an integer percent and omits missing legacy values', () => {
  expect(territorySharePercent(0.375)).toBe(38);
  expect(territorySharePercent(0.625)).toBe(63);
  expect(territorySharePercent(undefined)).toBeNull();
  expect(territorySharePercent(null)).toBeNull();
});
