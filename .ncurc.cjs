// npm-check-updates config (used by `just bump-npm`).
module.exports = {
  // vue-tsc can't load TypeScript 7 yet — keep TS on 6.x.
  target: (name) => (name === "typescript" ? "minor" : "latest"),
};
