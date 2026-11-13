import assert from 'node:assert/strict';
import test from 'node:test';
import {
  entryFromHash,
  countryUrl,
  filterCountries,
  visibleEntries
} from '../src/gallery.js';

const countries = [
  { name: 'Alpha', anchor: 'country-result-AA' },
  { name: 'Germany', anchor: 'country-result-DE' },
  {
    name: 'No population <yet>',
    anchor: 'country-missing-No%20population%20%3C' + 'yet%3E',
    missing: true
  }
];
const entries = [{ name: 'World', anchor: 'world-result' }, ...countries];

test('filters countries by trimmed, case-insensitive names', () => {
  assert.deepEqual(filterCountries(countries, '  ALP '), [countries[0]]);
  assert.deepEqual(filterCountries(countries, ''), countries.slice(0, 2));
  assert.deepEqual(filterCountries(countries, 'population'), []);
  assert.deepEqual(filterCountries(countries, 'population', true), [countries[2]]);
  assert.deepEqual(filterCountries(countries, '', true), countries);
});

test('resolves World and country entries from URL fragments, with the base URL unselected', () => {
  assert.equal(entryFromHash(entries, '#world-result'), entries[0]);
  assert.equal(entryFromHash(entries, '#country-result-AA'), countries[0]);
  assert.equal(
    entryFromHash(entries, '#country-missing-No%20population%20%3C' + 'yet%3E'),
    countries[2]
  );
  assert.equal(entryFromHash(entries, '#unknown'), null);
  assert.equal(entryFromHash(entries, ''), null);
});

test('shows all available entries by default or only a selected fragment entry', () => {
  assert.deepEqual(visibleEntries(entries, null), entries.slice(0, 3));
  assert.deepEqual(visibleEntries(entries, 'world-result'), [entries[0]]);
  assert.deepEqual(visibleEntries(entries, 'country-result-AA'), [countries[0]]);
  assert.deepEqual(visibleEntries(entries, null, true), entries);
  assert.deepEqual(visibleEntries(entries, countries[2].anchor, true), [countries[2]]);
});

test('copies the selected country fragment without keeping an old fragment', () => {
  assert.equal(
    countryUrl('https://example.test/gallery/?view=maps#old', 'country-result-AA'),
    'https://example.test/gallery/?view=maps#country-result-AA'
  );
});
