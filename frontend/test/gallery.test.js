import assert from 'node:assert/strict';
import test from 'node:test';
import {
  entryFromHash,
  countryUrl,
  filterCountries,
  sortCountries,
  groupCountries,
  territoryCaption,
} from '../src/gallery.js';

const countries = [
  { name: 'Alpha', anchor: 'country-result-AA' },
  { name: 'Germany', anchor: 'country-result-DE' },
  {
    name: 'No population <yet>',
    anchor: 'country-missing-No%20population%20%3C' + 'yet%3E',
    missing: true,
  },
];
const entries = [{ name: 'World', anchor: 'world-result' }, ...countries];

// --- Country list ---

test('filters countries by trimmed, case-insensitive names, including missing entries', () => {
  assert.deepEqual(filterCountries(countries, '  ALP '), [countries[0]]);
  assert.deepEqual(filterCountries(countries, ''), countries);
  assert.deepEqual(filterCountries(countries, 'population'), [countries[2]]);
});

test('sorts and groups countries alphabetically and counts normalized substring matches', () => {
  const unsorted = [
    { name: '\u00c9ire', anchor: 'ie' },
    { name: 'belgium', anchor: 'be' },
    { name: '\u00c5land', anchor: 'ax' },
    { name: 'Canada', anchor: 'ca' },
  ];
  assert.deepEqual(sortCountries(unsorted), [unsorted[2], unsorted[1], unsorted[3], unsorted[0]]);
  assert.deepEqual(Object.keys(groupCountries(unsorted)), ['A', 'B', 'C', 'E']);
  assert.deepEqual(groupCountries(unsorted).A, [unsorted[2]]);
  assert.deepEqual(filterCountries(unsorted, '  IRE '), [unsorted[0]]);
  assert.deepEqual(filterCountries(unsorted, 'land'), [unsorted[2]]);
  assert.equal(filterCountries(unsorted, 'a').length, 2);
  assert.deepEqual(filterCountries(unsorted, ''), unsorted);
  assert.deepEqual(filterCountries(unsorted, 'zz'), []);
});

// --- URL and display text ---

test('[navigation-and-history] resolves World and country entries from URL fragments, with the base URL unselected', () => {
  assert.equal(entryFromHash(entries, '#world-result'), entries[0]);
  assert.equal(entryFromHash(entries, '#country-result-AA'), countries[0]);
  assert.equal(
    entryFromHash(entries, '#country-missing-No%20population%20%3C' + 'yet%3E'),
    countries[2],
  );
  assert.equal(entryFromHash(entries, '#unknown'), null);
  assert.equal(entryFromHash(entries, ''), null);
});

test('copies the selected country fragment without keeping an old fragment', () => {
  assert.equal(
    countryUrl('https://example.test/gallery/?view=maps#old', 'country-result-AA'),
    'https://example.test/gallery/?view=maps#country-result-AA',
  );
});

test('formats circle coverage with an integer percent and omits missing legacy values', () => {
  assert.equal(
    territoryCaption(0.375, 'Earth’s land area'),
    'Circle covers 38% of Earth’s land area',
  );
  assert.equal(territoryCaption(0.625, 'Germany'), 'Circle covers 63% of Germany');
  assert.equal(territoryCaption(undefined), null);
  assert.equal(territoryCaption(null), null);
});
