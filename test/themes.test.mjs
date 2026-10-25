import test from 'node:test';
import assert from 'node:assert/strict';
import { THEMES, draftThemeFromPrompt, fixtureRecipe, injectDefect, accessScenario, deterministicChecks, validateRecipe, validateTheme, themeSchema } from '../public/core.mjs';

test('presets have distinct silhouettes and each navigable fixture satisfies shared gates', () => {
  const recipes = Object.fromEntries(Object.keys(THEMES).map(id => [id, fixtureRecipe(id)]));
  const shape = recipe => JSON.stringify(recipe.parts.map(({ id, shape, role, position, size }) => ({ id, shape, role, position, size })));
  assert.equal(new Set(Object.values(recipes).map(shape)).size, 3);
  assert.match(shape(recipes.argonaut), /relay-fin-west/);
  assert.match(shape(recipes.sylvan), /pavilion-canopy/);
  assert.match(shape(recipes.ember), /rampart-west/);
  for (const [id, recipe] of Object.entries(recipes)) {
    assert.ok(recipe.parts.length >= 4 && recipe.parts.length <= 48);
    assert.equal(accessScenario(recipe).pass, true, `${id} entrance should remain clear`);
    assert.ok(deterministicChecks(recipe, id).every(check => check.status === 'pass'), id);
    assert.ok(recipe.parts.filter(part => ['wall', 'support'].includes(part.role)).every(part => Math.abs(part.position[1] - part.size[1] / 2) < 0.08), id);
  }
});

test('obstruction defect blocks traversal for every silhouette', () => {
  for (const id of Object.keys(THEMES)) {
    assert.equal(accessScenario(injectDefect(fixtureRecipe(id))).pass, false, id);
  }
});

test('validated custom contract drives recipe identity and reference geometry', () => {
  const custom = structuredClone(THEMES.sylvan);
  custom.id = 'moss-garden';
  custom.name = 'Moss Garden';
  custom.palette.accent = '#DDAA44';
  assert.equal(validateTheme(custom), custom);
  const recipe = fixtureRecipe(custom);
  assert.equal(recipe.themeId, custom.id);
  assert.match(JSON.stringify(recipe.parts), /pavilion-canopy/);
  assert.equal(validateRecipe(recipe, custom), recipe);
  assert.throws(() => validateRecipe(recipe, THEMES.argonaut), /theme/);
  assert.throws(() => fixtureRecipe('not-a-preset'), /Unknown theme/);
});

test('strict theme validation rejects malformed metadata and unsupported fields', () => {
  const badColor = structuredClone(THEMES.argonaut);
  badColor.palette.shell = 'cyan';
  assert.throws(() => validateTheme(badColor), /palette/);
  const empty = structuredClone(THEMES.argonaut);
  empty.weathering = ' ';
  assert.throws(() => validateTheme(empty), /weathering/);
  const unknown = { ...structuredClone(THEMES.argonaut), source: 'model' };
  assert.throws(() => validateTheme(unknown), /unsupported fields/);
  const badSlug = { ...structuredClone(THEMES.argonaut), id: 'Bad--slug' };
  assert.throws(() => validateTheme(badSlug), /slug/);
  assert.deepEqual(themeSchema.required, ['id', 'name', 'genre', 'description', 'biome', 'shapeLanguage', 'materialFinish', 'weathering', 'technology', 'motifs', 'lighting', 'scale', 'gameplayRole', 'exclusions', 'visualChecks', 'assumptions', 'renderingNotes', 'referencePreset', 'palette', 'sky', 'flora', 'architecture']);
});

test('prompt draft is explicitly local keyword/default selection and can use chosen preset fallback', () => {
  const forest = draftThemeFromPrompt('A mossy forest listening post', 'ember');
  assert.equal(forest.id, 'custom-draft');
  assert.equal(forest.referencePreset, 'sylvan');
  assert.match(forest.assumptions[0], /Local keyword-based preset draft/);
  assert.match(forest.assumptions[1], /does not claim to interpret/);
  const volcano = draftThemeFromPrompt('A basalt vent station');
  assert.equal(volcano.referencePreset, 'ember');
  const fallback = draftThemeFromPrompt('A quiet frontier station', 'sylvan');
  assert.equal(fallback.referencePreset, 'sylvan');
  assert.match(fallback.assumptions[0], /selected\/default preset \(sylvan\)/);
  assert.doesNotThrow(() => validateRecipe(fixtureRecipe(fallback), fallback));
  assert.equal(draftThemeFromPrompt('No theme keyword').referencePreset, 'argonaut');
});
