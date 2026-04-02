<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import "monaco-editor/min/vs/editor/editor.main.css";

type MonacoLanguage = "html" | "markdown" | "plaintext";

interface CursorPosition {
  line: number;
  column: number;
}

const props = withDefaults(
  defineProps<{
    modelValue: string;
    language: MonacoLanguage;
    ariaLabel?: string;
    autofocus?: boolean;
  }>(),
  {
    ariaLabel: "Code editor",
    autofocus: false
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  "cursor-change": [position: CursorPosition];
  "save-shortcut": [];
}>();

const root = ref<HTMLElement | null>(null);

let monaco: typeof import("monaco-editor") | null = null;
let editor: import("monaco-editor").editor.IStandaloneCodeEditor | null = null;
let syncingExternalValue = false;
let themeObserver: MutationObserver | null = null;

const isDarkTheme = ref(false);
const themeName = computed(() => (isDarkTheme.value ? "postpub-dark" : "postpub-light"));

async function ensureMonaco() {
  if (monaco) return monaco;

  const [monacoModule, editorWorkerModule, cssWorkerModule, htmlWorkerModule, jsonWorkerModule, tsWorkerModule] =
    await Promise.all([
      import("monaco-editor"),
      import("monaco-editor/esm/vs/editor/editor.worker?worker"),
      import("monaco-editor/esm/vs/language/css/css.worker?worker"),
      import("monaco-editor/esm/vs/language/html/html.worker?worker"),
      import("monaco-editor/esm/vs/language/json/json.worker?worker"),
      import("monaco-editor/esm/vs/language/typescript/ts.worker?worker")
    ]);

  monaco = monacoModule;
  const globalScope = globalThis as typeof globalThis & {
    MonacoEnvironment?: {
      getWorker: (_workerId: string, label: string) => Worker;
    };
  };

  if (!globalScope.MonacoEnvironment) {
    globalScope.MonacoEnvironment = {
      getWorker(_workerId, label) {
        if (label === "json") return new jsonWorkerModule.default();
        if (label === "css" || label === "scss" || label === "less") return new cssWorkerModule.default();
        if (label === "html" || label === "handlebars" || label === "razor") return new htmlWorkerModule.default();
        if (label === "typescript" || label === "javascript") return new tsWorkerModule.default();
        return new editorWorkerModule.default();
      }
    };
  }

  return monaco;
}

function ensureThemes() {
  if (!monaco) return;
  monaco.editor.defineTheme("postpub-light", {
    base: "vs",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#ffffff",
      "editor.foreground": "#0f172a",
      "editorLineNumber.foreground": "#94a3b8",
      "editorLineNumber.activeForeground": "#0f172a",
      "editorGutter.background": "#ffffff",
      "editorIndentGuide.background1": "#e2e8f0",
      "editorIndentGuide.activeBackground1": "#94a3b8",
      "editor.selectionBackground": "#bfdbfe",
      "editor.inactiveSelectionBackground": "#dbeafe"
    }
  });

  monaco.editor.defineTheme("postpub-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#0b1120",
      "editor.foreground": "#e2e8f0",
      "editorLineNumber.foreground": "#64748b",
      "editorLineNumber.activeForeground": "#f8fafc",
      "editorGutter.background": "#0b1120",
      "editorIndentGuide.background1": "#334155",
      "editorIndentGuide.activeBackground1": "#64748b",
      "editor.selectionBackground": "#1d4ed8",
      "editor.inactiveSelectionBackground": "#1e3a8a"
    }
  });
}

function emitCursorPosition() {
  if (!editor) return;
  const position = editor.getPosition();
  if (!position) return;
  emit("cursor-change", { line: position.lineNumber, column: position.column });
}

function focusEditor() {
  editor?.focus();
}

function readThemeState() {
  const rootElement = document.documentElement;
  isDarkTheme.value =
    rootElement.getAttribute("data-design-surface") === "dark" || rootElement.getAttribute("data-theme") === "dark";
}

async function createEditor() {
  if (!root.value) return;

  const monacoModule = await ensureMonaco();
  ensureThemes();

  editor = monacoModule.editor.create(root.value, {
    value: props.modelValue,
    language: props.language,
    theme: themeName.value,
    automaticLayout: true,
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    wordWrap: "on",
    tabSize: 2,
    insertSpaces: true,
    formatOnPaste: true,
    formatOnType: true,
    fontSize: 14,
    lineHeight: 24,
    fontFamily: "Consolas, 'SFMono-Regular', Menlo, Monaco, monospace",
    padding: { top: 18, bottom: 18 },
    scrollbar: {
      verticalScrollbarSize: 10,
      horizontalScrollbarSize: 10
    }
  });

  editor.onDidChangeModelContent(() => {
    if (!editor || syncingExternalValue) return;
    const value = editor.getValue();
    if (value !== props.modelValue) emit("update:modelValue", value);
  });

  editor.onDidChangeCursorPosition(() => {
    emitCursorPosition();
  });

  editor.addCommand(monacoModule.KeyMod.CtrlCmd | monacoModule.KeyCode.KeyS, () => {
    emit("save-shortcut");
  });

  emitCursorPosition();

  if (props.autofocus) {
    requestAnimationFrame(() => {
      focusEditor();
    });
  }
}

watch(
  () => props.modelValue,
  (value) => {
    if (!editor) return;
    if (value === editor.getValue()) return;

    syncingExternalValue = true;
    editor.setValue(value);
    syncingExternalValue = false;
    emitCursorPosition();
  }
);

watch(
  () => props.language,
  (language) => {
    if (!monaco) return;
    const model = editor?.getModel();
    if (!model) return;
    monaco.editor.setModelLanguage(model, language);
  }
);

watch(themeName, (value) => {
  if (!monaco) return;
  monaco.editor.setTheme(value);
});

onMounted(() => {
  readThemeState();
  themeObserver = new MutationObserver(() => {
    readThemeState();
  });
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-design-surface", "data-theme"]
  });
  void createEditor();
});

onBeforeUnmount(() => {
  themeObserver?.disconnect();
  themeObserver = null;
  editor?.dispose();
  editor = null;
});

defineExpose({
  focus: focusEditor
});
</script>

<template>
  <div ref="root" class="template-monaco-editor" :aria-label="ariaLabel"></div>
</template>
