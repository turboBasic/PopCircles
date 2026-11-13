import assert from 'node:assert/strict';
import test from 'node:test';
import {
  countryFromHash,
  countryUrl,
  defaultCountry,
  filterCountries,
  visibleCountries
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

test('filters countries by trimmed, case-insensitive names', () => {
  assert.deepEqual(filterCountries(countries, '  ALP '), [countries[0]]);
  assert.deepEqual(filterCountries(countries, ''), countries.slice(0, 2));
  assert.deepEqual(filterCountries(countries, 'population'), []);
  assert.deepEqual(filterCountries(countries, 'population', true), [countries[2]]);
  assert.deepEqual(filterCountries(countries, '', true), countries);
});

test('restores countries from stable URL anchors and falls back when unmatched', () => {
  assert.equal(countryFromHash(countries, '#country-result-AA', null), countries[0]);
  assert.equal(
    countryFromHash(countries, '#country-missing-No%20population%20%3C' + 'yet%3E', null),
    countries[2]
  );
  assert.equal(countryFromHash(countries, '#unknown', countries[1]), countries[1]);
  assert.equal(countryFromHash(countries, '', countries[1]), countries[1]);
});

test('defaults to Germany or the first available country', () => {
  assert.equal(defaultCountry(countries), countries[1]);
  assert.equal(defaultCountry([countries[0]]), countries[0]);
  assert.equal(defaultCountry([]), null);
});

test('shows maps for the selected country or every country when expanded', () => {
  assert.deepEqual(visibleCountries(countries, 'country-result-AA', false), [countries[0]]);
  assert.deepEqual(visibleCountries(countries, null, false), []);
  assert.deepEqual(visibleCountries(countries, 'country-result-AA', true), countries.slice(0, 2));
  assert.deepEqual(visibleCountries(countries, 'country-result-AA', true, true), countries);
});

test('copies the selected country fragment without keeping an old fragment', () => {
  assert.equal(
    countryUrl('https://example.test/gallery/?view=maps#old', 'country-result-AA'),
    'https://example.test/gallery/?view=maps#country-result-AA'
  );
});
