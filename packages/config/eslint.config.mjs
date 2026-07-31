// Shared flat ESLint config for the viewer apps and packages. Opt-in: an app/package adds
// its own `eslint.config.mjs` that re-exports this.
import js from "@eslint/js";

export default [
  js.configs.recommended,
  {
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
    },
    rules: {
      "no-unused-vars": ["warn", { argsIgnorePattern: "^_" }],
    },
  },
  {
    ignores: ["**/dist/**", "**/node_modules/**", "**/src-tauri/**"],
  },
];
