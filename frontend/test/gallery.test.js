import assert from 'node:assert/strict';
import test from 'node:test';
import { countryFromHash, defaultCountry, filterCountries } from '../src/gallery.js';

const countries = [
  { name: 'Alpha', anchor: 'country-result-AA' },
  { name: 'Germany', anchor: 'country-result-DE' },
  {
    name: 'No population <yet>',
    anchor: 'country-missing-No%20population%20%3C' + 'yet%3E'
  }
];

test('filters countries by trimmed, case-insensitive names', () => {
  assert.deepEqual(filterCountries(countries, '  ALP '), [countries[0]]);
  assert.deepEqual(filterCountries(countries, ''), countries);
  assert.deepEqual(filterCountries(countries, 'population'), [countries[2]]);
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
