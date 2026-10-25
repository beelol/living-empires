# Paid procedural city expansion

Primary SET-01, SET-04; affected SET-02, WORLD-05. G owns game/src/server.rs. City owner owns Lua layouts and native procedural forms. Baseline fd982be. No save, protocol, recipe-cost, collision or approval format changes. The independent solo-colony lane is excluded.

The previous server stopped after one core, generator and storehouse per empire. Continue serial construction through existing World transactions, with stable procedural-growth-N receipts and a limit of twelve buildings per empire. Every accepted site still passes terrain, rights, collision, access and cost checks. Existing buildings and starter receipts stay intact. This is a bounded development scenario, not evidence of useful economic production, autonomous need selection or reviewed Foundry activation.

Regression first failed because helios stayed at three buildings. The test exercises real construction ticks and disk saves, reconciles costs against hand-specified 100/60/40 prices, releases the world lock, reloads, continues and verifies original entity IDs/positions remain. Default layout may exhaust legal sites below twelve; that outcome remains blocked, without relaxing authority checks. Native variety is the city renderer owner's separate checkpoint.

Validation: headless game library 13 tests pass with localhost permission; initial restricted run had only the known socket permission failure. Strict headless library clippy passes. The growth regression failed on the original three-building stop before implementation. Full native/visual acceptance depends on the city renderer checkpoint and actual captures; no AC box is marked passed.
