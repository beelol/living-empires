# The default game is the single-player, prompt-driven robot colony.
default: dev

dev:
    node scripts/dev.mjs

# Asset Foundry remains an independent service, with its existing API and data.
studio:
    npm run studio

test:
    node --test solo/test/*.test.mjs
