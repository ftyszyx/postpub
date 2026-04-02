const fs = require("fs");
const path = require("path");
const { glob } = require("glob");
const babelParser = require("@babel/parser");
const traverse = require("@babel/traverse").default;
const { parse: parseSfc } = require("@vue/compiler-sfc");
const { compile: compileTemplate } = require("@vue/compiler-dom");

const rootDir = process.cwd();

function loadConfig() {
  const candidates = [
    path.resolve(rootDir, "i18n.config.cjs"),
    path.resolve(rootDir, "i18n.config.js")
  ];

  for (const configPath of candidates) {
    if (fs.existsSync(configPath)) {
      return require(configPath);
    }
  }

  return {};
}

const userConfig = loadConfig();
const config = {
  srcDir: path.resolve(rootDir, userConfig.srcDir || "./src"),
  outputDir: path.resolve(rootDir, userConfig.outputDir || "./src/locales"),
  outputFormat: userConfig.outputFormat === "json" ? "json" : "ts",
  locales: userConfig.locales || ["zh-CN", "en-US"],
  filePatterns: userConfig.filePatterns || ["**/*.vue", "**/*.ts", "**/*.js"],
  ignoreDirs: userConfig.ignoreDirs || ["node_modules", "dist", "public"],
  ignoreFiles: userConfig.ignoreFiles || [],
  preserveExistingTranslations:
    userConfig.preserveExistingTranslations !== undefined
      ? userConfig.preserveExistingTranslations
      : true,
  organizeByNamespace:
    userConfig.organizeByNamespace !== undefined ? userConfig.organizeByNamespace : true,
  functionNames: userConfig.functionNames || ["t", "$t"],
  defaultValue: userConfig.defaultValue !== undefined ? userConfig.defaultValue : ""
};

function extractKeysFromCode(code) {
  const keys = new Set();
  let ast;

  try {
    ast = babelParser.parse(code, {
      sourceType: "unambiguous",
      plugins: ["jsx", "typescript"]
    });
  } catch {
    return [];
  }

  traverse(ast, {
    CallExpression(callPath) {
      const { callee, arguments: args } = callPath.node;
      const firstArg = args[0];
      if (!firstArg || firstArg.type !== "StringLiteral") {
        return;
      }

      if (callee.type === "Identifier" && config.functionNames.includes(callee.name)) {
        keys.add(firstArg.value);
        return;
      }

      if (
        callee.type === "MemberExpression" &&
        !callee.computed &&
        callee.property.type === "Identifier" &&
        config.functionNames.includes(callee.property.name)
      ) {
        keys.add(firstArg.value);
      }
    }
  });

  return Array.from(keys);
}

function extractKeysFromComments(code) {
  const keys = [];
  const matcher =
    /(?:\/\/|\/\*)\s*i18n(?:-key)?:\s*([a-zA-Z_][\w]*(?:\.[a-zA-Z_][\w]*)+)/g;

  for (const match of code.matchAll(matcher)) {
    keys.push(match[1]);
  }

  return keys;
}

function extractKeysFromVue(fileContent) {
  const keys = new Set();

  try {
    const { descriptor } = parseSfc(fileContent);
    const scriptContent =
      (descriptor.script?.content || "") + "\n" + (descriptor.scriptSetup?.content || "");

    for (const key of extractKeysFromCode(scriptContent)) {
      keys.add(key);
    }

    if (descriptor.template?.content) {
      const renderCode = compileTemplate(descriptor.template.content, { mode: "module" }).code;
      for (const key of extractKeysFromCode(renderCode)) {
        keys.add(key);
      }
    }
  } catch {
    return [];
  }

  return Array.from(keys);
}

function extractKeysFromFile(filePath) {
  const content = fs.readFileSync(filePath, "utf8");
  const ext = path.extname(filePath).toLowerCase();
  const keys = new Set(extractKeysFromComments(content));

  const extracted =
    ext === ".vue" ? extractKeysFromVue(content) : extractKeysFromCode(content);

  for (const key of extracted) {
    keys.add(key);
  }

  return Array.from(keys);
}

function toNestedObject(keys) {
  const output = {};

  for (const key of keys) {
    const parts = key.split(".");
    let current = output;

    for (let index = 0; index < parts.length; index += 1) {
      const part = parts[index];
      const isLeaf = index === parts.length - 1;

      if (isLeaf) {
        if (current[part] === undefined) {
          current[part] = config.defaultValue;
        }
      } else {
        current[part] = current[part] || {};
        current = current[part];
      }
    }
  }

  return output;
}

function deepMerge(existingValue, nextValue) {
  const result = { ...existingValue };

  for (const key of Object.keys(nextValue)) {
    const existing = existingValue?.[key];
    const next = nextValue[key];

    if (
      existing &&
      typeof existing === "object" &&
      !Array.isArray(existing) &&
      next &&
      typeof next === "object" &&
      !Array.isArray(next)
    ) {
      result[key] = deepMerge(existing, next);
      continue;
    }

    if (config.preserveExistingTranslations && existing !== undefined) {
      result[key] = existing;
    } else {
      result[key] = next;
    }
  }

  return result;
}

async function collectFiles() {
  const files = new Set();
  const ignorePatterns = [
    ...config.ignoreDirs.map((entry) => `**/${entry}/**`),
    ...config.ignoreFiles
  ];

  for (const pattern of config.filePatterns) {
    const matches = await glob(pattern, {
      cwd: config.srcDir,
      absolute: true,
      ignore: ignorePatterns
    });
    for (const match of matches) {
      files.add(match);
    }
  }

  return Array.from(files);
}

async function main() {
  const files = await collectFiles();
  const keySet = new Set();

  for (const file of files) {
    for (const key of extractKeysFromFile(file)) {
      keySet.add(key);
    }
  }

  const extractedTree = toNestedObject(Array.from(keySet).sort());
  fs.mkdirSync(config.outputDir, { recursive: true });

  for (const locale of config.locales) {
    const filePath = path.join(config.outputDir, `${locale}.${config.outputFormat}`);
    const existing = fs.existsSync(filePath)
      ? JSON.parse(fs.readFileSync(filePath, "utf8"))
      : {};
    const merged = deepMerge(existing, extractedTree);
    fs.writeFileSync(filePath, `${JSON.stringify(merged, null, 2)}\n`, "utf8");
    console.log(`updated ${path.relative(rootDir, filePath)}`);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
