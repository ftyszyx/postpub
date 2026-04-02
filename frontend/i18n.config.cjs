module.exports = {
  srcDir: "./src",
  outputDir: "./src/locales",
  outputFormat: "json",
  locales: ["zh-CN", "en-US"],
  filePatterns: ["**/*.vue", "**/*.ts"],
  ignoreDirs: ["vendors"],
  ignoreFiles: [],
  preserveExistingTranslations: true,
  organizeByNamespace: true,
  functionNames: ["t", "$t"],
  defaultValue: ""
};
