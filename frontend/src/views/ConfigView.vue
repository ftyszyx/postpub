<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { useConfigStore } from "../stores/config";
import { useNavigationStore, type ConfigPanel } from "../stores/navigation";
import { useTemplateStore } from "../stores/templates";
import type {
  CustomLlmProvider,
  ImageModelProvider,
  PublishTargetConfig,
  TemplateSummary
} from "../types/postpub";

const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const configStore = useConfigStore();
const navigationStore = useNavigationStore();
const templateStore = useTemplateStore();

const activeSection = computed<ConfigPanel>(() => navigationStore.activeConfigPanel);
const imageModels = computed(() => configStore.bundle.config.img_api.providers);
const customProviders = computed(() => configStore.bundle.ui_config.custom_llm_providers);
const publishTargets = computed(() => configStore.bundle.config.publish_targets);

const imageModelModalOpen = ref(false);
const imageModelEditId = ref<string | null>(null);
const imageModelDraft = ref<ImageModelProvider>(createImageModelDraft());

const llmModalOpen = ref(false);
const llmEditId = ref<string | null>(null);
const llmDraft = ref<CustomLlmProvider>(createLlmDraft());

const publishTargetModalOpen = ref(false);
const publishTargetEditId = ref<string | null>(null);
const publishTargetDraft = ref<PublishTargetConfig>(createPublishTargetDraft());

const publishTargetTemplateOptions = computed(() =>
  templateStore.templates.filter(
    (template: TemplateSummary) => template.category === publishTargetDraft.value.template_category
  )
);

const imageModelTypeOptions = computed(() => [
  { value: "picsum", label: t("config.sections.imageProviderPicsum") },
  { value: "ali", label: t("config.sections.imageProviderAli") }
]);

const publishPlatformTypeOptions = computed(() => [
  { value: "wechat", label: t("config.sections.publishPlatformTypeWechat") }
]);

function createImageModelDraft(index = imageModels.value.length + 1): ImageModelProvider {
  return {
    id: `image-model-${Date.now()}-${index}`,
    name: t("config.sections.imageModelDraftName", { index }),
    provider_type: "picsum",
    api_key: "",
    model: "",
    enabled: true
  };
}

function createLlmDraft(index = customProviders.value.length + 1): CustomLlmProvider {
  return {
    id: `custom-${Date.now()}-${index}`,
    name: t("config.sections.customProviderName", { index }),
    key_name: `CUSTOM_API_KEY_${index}`,
    api_key: "",
    api_base: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
    protocol_type: "openai",
    max_tokens: 8192,
    enabled: index === 1
  };
}

function createPublishTargetDraft(index = publishTargets.value.length + 1): PublishTargetConfig {
  return {
    id: `publish-wechat-${Date.now()}-${index}`,
    name: t("config.sections.publishPlatformDraftName", { index }),
    platform_type: "wechat",
    account_name: "",
    cookies: "",
    publish_url: "https://mp.weixin.qq.com",
    enabled: publishTargets.value.length === 0,
    article_format: "html",
    template_category: templateStore.categories[0]?.name || "general",
    template_name: "",
    min_article_len: 900,
    max_article_len: 2000,
    use_template: true,
    use_compress: false,
    auto_publish: false,
    format_publish: true
  };
}

function ensurePublishTargetTemplateSelection() {
  const draft = publishTargetDraft.value;
  const matched = publishTargetTemplateOptions.value.find(
    (template: TemplateSummary) => template.name === draft.template_name
  );
  if (!matched) {
    draft.template_name = publishTargetTemplateOptions.value[0]?.name || "";
  }
}

function setActiveImageModel(id: string) {
  configStore.bundle.config.img_api.active_provider_id = id;
  configStore.bundle.config.img_api.providers = imageModels.value.map((provider) => ({
    ...provider,
    enabled: provider.id === id ? true : provider.enabled
  }));
}

function removeImageModel(id: string) {
  const remaining = imageModels.value.filter((provider) => provider.id !== id);
  configStore.bundle.config.img_api.providers = remaining;
  if (configStore.bundle.config.img_api.active_provider_id === id) {
    configStore.bundle.config.img_api.active_provider_id = remaining[0]?.id || "";
  }
}

function openCreateImageModelModal() {
  imageModelEditId.value = null;
  imageModelDraft.value = createImageModelDraft();
  imageModelModalOpen.value = true;
}

function openEditImageModelModal(provider: ImageModelProvider) {
  imageModelEditId.value = provider.id;
  imageModelDraft.value = { ...provider };
  imageModelModalOpen.value = true;
}

function closeImageModelModal() {
  imageModelModalOpen.value = false;
  imageModelEditId.value = null;
}

function saveImageModelDraft() {
  const draft = { ...imageModelDraft.value };
  if (!draft.name.trim()) {
    draft.name = t("config.sections.imageModelFallbackName");
  }

  if (imageModelEditId.value) {
    configStore.bundle.config.img_api.providers = imageModels.value.map((provider) =>
      provider.id === imageModelEditId.value ? draft : provider
    );
  } else {
    configStore.bundle.config.img_api.providers.push(draft);
  }

  if (!configStore.bundle.config.img_api.active_provider_id) {
    configStore.bundle.config.img_api.active_provider_id = draft.id;
  }

  closeImageModelModal();
}

function openCreateLlmModal() {
  llmEditId.value = null;
  llmDraft.value = createLlmDraft();
  llmModalOpen.value = true;
}

function openEditLlmModal(provider: CustomLlmProvider) {
  llmEditId.value = provider.id;
  llmDraft.value = { ...provider };
  llmModalOpen.value = true;
}

function closeLlmModal() {
  llmModalOpen.value = false;
  llmEditId.value = null;
}

function saveLlmDraft() {
  const draft = { ...llmDraft.value };
  if (!draft.name.trim()) {
    draft.name = t("config.sections.customProviderFallbackName");
  }

  if (llmEditId.value) {
    configStore.bundle.ui_config.custom_llm_providers = customProviders.value.map((provider) =>
      provider.id === llmEditId.value ? draft : provider
    );
  } else {
    configStore.bundle.ui_config.custom_llm_providers.push(draft);
  }

  closeLlmModal();
}

function removeCustomProvider(id: string) {
  const remaining = customProviders.value.filter((provider) => provider.id !== id);
  configStore.bundle.ui_config.custom_llm_providers = remaining.length ? remaining : [createLlmDraft(1)];
}

function openCreatePublishTargetModal() {
  publishTargetEditId.value = null;
  publishTargetDraft.value = createPublishTargetDraft();
  ensurePublishTargetTemplateSelection();
  publishTargetModalOpen.value = true;
}

function openEditPublishTargetModal(target: PublishTargetConfig) {
  publishTargetEditId.value = target.id;
  publishTargetDraft.value = { ...target };
  ensurePublishTargetTemplateSelection();
  publishTargetModalOpen.value = true;
}

function closePublishTargetModal() {
  publishTargetModalOpen.value = false;
  publishTargetEditId.value = null;
}

function savePublishTargetDraft() {
  const draft = { ...publishTargetDraft.value };
  if (!draft.name.trim()) {
    draft.name = t("config.sections.publishPlatformFallbackName");
  }

  if (publishTargetEditId.value) {
    configStore.bundle.config.publish_targets = publishTargets.value.map((target) =>
      target.id === publishTargetEditId.value ? draft : target
    );
  } else {
    configStore.bundle.config.publish_targets.push(draft);
  }

  closePublishTargetModal();
}

function removePublishTarget(id: string) {
  configStore.bundle.config.publish_targets = publishTargets.value.filter((target) => target.id !== id);
}

async function restoreDefaults() {
  await configStore.loadDefaults();
}

onMounted(async () => {
  const legacyPanel = Array.isArray(route.query.panel) ? route.query.panel[0] : route.query.panel;
  if (legacyPanel) {
    navigationStore.setActiveConfigPanel(String(legacyPanel));
    await router.replace({ path: "/config" });
  }

  if (!templateStore.categories.length && !templateStore.loading) {
    void templateStore.loadAll();
  }
});

watch(
  () => route.query.panel,
  async (panel) => {
    const legacyPanel = Array.isArray(panel) ? panel[0] : panel;
    if (!legacyPanel) {
      return;
    }

    navigationStore.setActiveConfigPanel(String(legacyPanel));
    await router.replace({ path: "/config" });
  }
);

watch(
  () => publishTargetDraft.value.template_category,
  () => {
    ensurePublishTargetTemplateSelection();
  }
);
</script>

<template>
  <section class="config-content-stage">
    <section v-if="activeSection === 'ui'" class="panel panel--config">
      <div class="config-panel-header">
        <div>
          <p class="eyebrow">{{ t("config.sections.uiEyebrow") }}</p>
          <h3 class="panel-title">{{ t("config.sections.uiTitle") }}</h3>
          <p class="workspace-copy">{{ t("config.sections.uiDescription") }}</p>
        </div>
      </div>

      <div class="form-grid two-columns">
        <label class="field">
          <span>{{ t("config.sections.themeMode") }}</span>
          <select v-model="configStore.bundle.ui_config.theme" class="text-input">
            <option value="light">{{ t("config.sections.themeLight") }}</option>
            <option value="dark">{{ t("config.sections.themeDark") }}</option>
          </select>
          <small class="field-help">{{ t("config.sections.themeHelp") }}</small>
        </label>

        <label class="field">
          <span>{{ t("config.sections.windowMode") }}</span>
          <select v-model="configStore.bundle.ui_config.window_mode" class="text-input">
            <option value="STANDARD">{{ t("config.sections.windowModeStandard") }}</option>
            <option value="MAXIMIZED">{{ t("config.sections.windowModeMaximized") }}</option>
          </select>
          <small class="field-help">{{ t("config.sections.windowModeHelp") }}</small>
        </label>

        <label class="field">
          <span>{{ t("config.sections.designTheme") }}</span>
          <select v-model="configStore.bundle.ui_config.design_theme" class="text-input">
            <option value="follow-system">{{ t("config.sections.designThemeFollowSystem") }}</option>
            <option value="default">{{ t("config.sections.designThemeDefault") }}</option>
          </select>
          <small class="field-help">{{ t("config.sections.designThemeHelp") }}</small>
        </label>
      </div>

      <div class="config-actions-bar">
        <button class="btn btn-primary" type="button" @click="configStore.save">
          {{ configStore.saving ? t("common.saving") : t("config.saveConfig") }}
        </button>
        <button class="btn btn-secondary" type="button" @click="restoreDefaults">
          {{ t("common.restoreDefaults") }}
        </button>
      </div>
    </section>

    <section v-else-if="activeSection === 'image-models'" class="panel panel--config">
      <div class="config-panel-header">
        <div>
          <p class="eyebrow">{{ t("config.sections.imageModelsEyebrow") }}</p>
          <h3 class="panel-title">{{ t("config.sections.imageModelsTitle") }}</h3>
          <p class="workspace-copy">{{ t("config.sections.imageModelsDescription") }}</p>
        </div>
        <button class="btn btn-secondary" type="button" @click="openCreateImageModelModal">
          {{ t("config.sections.addImageModel") }}
        </button>
      </div>

      <div class="stack-list" v-if="imageModels.length">
        <article v-for="provider in imageModels" :key="provider.id" class="provider-card">
          <div class="provider-card__header">
            <div class="provider-card__title-row">
              <h4 class="provider-card__title">{{ provider.name }}</h4>
              <span class="provider-card__badge">
                {{
                  configStore.bundle.config.img_api.active_provider_id === provider.id
                    ? t("config.sections.currentInUse")
                    : t("config.sections.providerAvailable")
                }}
              </span>
            </div>

            <div class="btn-row">
              <button
                class="btn btn-secondary"
                type="button"
                @click="setActiveImageModel(provider.id)"
                :disabled="configStore.bundle.config.img_api.active_provider_id === provider.id"
              >
                {{ t("config.sections.setAsCurrent") }}
              </button>
              <button class="btn btn-secondary" type="button" @click="openEditImageModelModal(provider)">
                {{ t("config.sections.editItem") }}
              </button>
              <button class="btn btn-danger" type="button" @click="removeImageModel(provider.id)">
                {{ t("common.delete") }}
              </button>
            </div>
          </div>

          <div class="info-grid">
            <div class="info-row">
              <span>{{ t("config.sections.providerType") }}</span>
              <strong>{{ provider.provider_type }}</strong>
            </div>
            <div class="info-row">
              <span>{{ t("config.sections.modelName") }}</span>
              <strong>{{ provider.model || "-" }}</strong>
            </div>
          </div>
        </article>
      </div>

      <div v-else class="empty-state">
        {{ t("config.sections.imageModelsEmpty") }}
      </div>

      <div class="config-actions-bar">
        <button class="btn btn-primary" type="button" @click="configStore.save">
          {{ configStore.saving ? t("common.saving") : t("config.saveConfig") }}
        </button>
        <button class="btn btn-secondary" type="button" @click="restoreDefaults">
          {{ t("common.restoreDefaults") }}
        </button>
      </div>
    </section>

    <section v-else-if="activeSection === 'llm-models'" class="panel panel--config">
      <div class="config-panel-header">
        <div>
          <p class="eyebrow">{{ t("config.sections.customLlmEyebrow") }}</p>
          <h3 class="panel-title">{{ t("config.sections.customLlmTitle") }}</h3>
          <p class="workspace-copy">{{ t("config.sections.customLlmDescription") }}</p>
        </div>
        <button class="btn btn-secondary" type="button" @click="openCreateLlmModal">
          {{ t("config.sections.addCustomProvider") }}
        </button>
      </div>

      <div class="stack-list" v-if="customProviders.length">
        <article v-for="provider in customProviders" :key="provider.id" class="provider-card">
          <div class="provider-card__header">
            <div class="provider-card__title-row">
              <h4 class="provider-card__title">{{ provider.name }}</h4>
              <span class="provider-card__badge">
                {{ provider.enabled ? t("config.sections.providerActive") : t("config.sections.providerInactive") }}
              </span>
            </div>

            <div class="btn-row">
              <button class="btn btn-secondary" type="button" @click="openEditLlmModal(provider)">
                {{ t("config.sections.editItem") }}
              </button>
              <button class="btn btn-danger" type="button" @click="removeCustomProvider(provider.id)">
                {{ t("common.delete") }}
              </button>
            </div>
          </div>

          <div class="info-grid">
            <div class="info-row">
              <span>{{ t("config.sections.protocolType") }}</span>
              <strong>{{ provider.protocol_type }}</strong>
            </div>
            <div class="info-row">
              <span>{{ t("config.sections.modelName") }}</span>
              <strong>{{ provider.model }}</strong>
            </div>
            <div class="info-row">
              <span>{{ t("config.sections.apiBase") }}</span>
              <strong>{{ provider.api_base }}</strong>
            </div>
          </div>
        </article>
      </div>

      <div v-else class="empty-state">
        {{ t("config.sections.llmModelsEmpty") }}
      </div>

      <div class="config-actions-bar">
        <button class="btn btn-primary" type="button" @click="configStore.save">
          {{ configStore.saving ? t("common.saving") : t("config.saveConfig") }}
        </button>
        <button class="btn btn-secondary" type="button" @click="restoreDefaults">
          {{ t("common.restoreDefaults") }}
        </button>
      </div>
    </section>

    <section v-else class="panel panel--config">
      <div class="config-panel-header">
        <div>
          <p class="eyebrow">{{ t("config.sections.publishPlatformsEyebrow") }}</p>
          <h3 class="panel-title">{{ t("config.sections.publishPlatformsTitle") }}</h3>
          <p class="workspace-copy">{{ t("config.sections.publishPlatformsDescription") }}</p>
        </div>
        <button class="btn btn-secondary" type="button" @click="openCreatePublishTargetModal">
          {{ t("config.sections.addPublishPlatform") }}
        </button>
      </div>

      <div class="stack-list" v-if="publishTargets.length">
        <article v-for="target in publishTargets" :key="target.id" class="provider-card">
          <div class="provider-card__header">
            <div class="provider-card__title-row">
              <h4 class="provider-card__title">{{ target.name }}</h4>
              <span class="provider-card__badge">
                {{ target.enabled ? t("config.sections.providerActive") : t("config.sections.providerInactive") }}
              </span>
            </div>

            <div class="btn-row">
              <button class="btn btn-secondary" type="button" @click="openEditPublishTargetModal(target)">
                {{ t("config.sections.editItem") }}
              </button>
              <button class="btn btn-danger" type="button" @click="removePublishTarget(target.id)">
                {{ t("common.delete") }}
              </button>
            </div>
          </div>

          <div class="info-grid">
            <div class="info-row">
              <span>{{ t("config.sections.platformType") }}</span>
              <strong>{{ target.platform_type }}</strong>
            </div>
            <div class="info-row">
              <span>{{ t("config.sections.accountName") }}</span>
              <strong>{{ target.account_name || "-" }}</strong>
            </div>
            <div class="info-row">
              <span>{{ t("config.sections.publishUrl") }}</span>
              <strong>{{ target.publish_url || "-" }}</strong>
            </div>
            <div class="info-row">
              <span>{{ t("config.sections.outputFormat") }}</span>
              <strong>{{ target.article_format }}</strong>
            </div>
            <div class="info-row">
              <span>{{ t("config.sections.templateRule") }}</span>
              <strong>{{ `${target.template_category} / ${target.template_name || "-"}` }}</strong>
            </div>
          </div>
        </article>
      </div>

      <div v-else class="empty-state">
        {{ t("config.sections.publishPlatformsEmpty") }}
      </div>

      <div class="config-actions-bar">
        <button class="btn btn-primary" type="button" @click="configStore.save">
          {{ configStore.saving ? t("common.saving") : t("config.saveConfig") }}
        </button>
        <button class="btn btn-secondary" type="button" @click="restoreDefaults">
          {{ t("common.restoreDefaults") }}
        </button>
      </div>
    </section>

    <div v-if="configStore.lastMessage" class="banner banner-success">{{ configStore.lastMessage }}</div>
    <div v-if="configStore.error" class="banner banner-error">{{ configStore.error }}</div>
  </section>

  <div v-if="imageModelModalOpen" class="config-modal-backdrop" @click.self="closeImageModelModal">
    <section class="config-modal">
      <div class="config-modal__header">
        <div>
          <p class="eyebrow">{{ t("config.sections.imageModelsEyebrow") }}</p>
          <h3 class="panel-title">
            {{ imageModelEditId ? t("config.sections.editImageModel") : t("config.sections.addImageModel") }}
          </h3>
        </div>
        <button class="btn btn-secondary" type="button" @click="closeImageModelModal">
          {{ t("config.sections.closeModal") }}
        </button>
      </div>

      <div class="form-grid two-columns">
        <label class="field">
          <span>{{ t("common.name") }}</span>
          <input v-model="imageModelDraft.name" class="text-input" type="text" />
        </label>

        <label class="field">
          <span>{{ t("config.sections.providerType") }}</span>
          <select v-model="imageModelDraft.provider_type" class="text-input">
            <option v-for="option in imageModelTypeOptions" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
        </label>

        <label class="field">
          <span>{{ t("config.sections.apiKey") }}</span>
          <input v-model="imageModelDraft.api_key" class="text-input" type="password" />
        </label>

        <label class="field">
          <span>{{ t("config.sections.modelName") }}</span>
          <input v-model="imageModelDraft.model" class="text-input" type="text" />
        </label>

        <label class="checkbox-card field--wide">
          <input v-model="imageModelDraft.enabled" type="checkbox" />
          <span>{{ t("common.enabled") }}</span>
        </label>
      </div>

      <div class="config-actions-bar">
        <button class="btn btn-primary" type="button" @click="saveImageModelDraft">
          {{ t("common.save") }}
        </button>
        <button class="btn btn-secondary" type="button" @click="closeImageModelModal">
          {{ t("config.sections.cancelEdit") }}
        </button>
      </div>
    </section>
  </div>

  <div v-if="llmModalOpen" class="config-modal-backdrop" @click.self="closeLlmModal">
    <section class="config-modal">
      <div class="config-modal__header">
        <div>
          <p class="eyebrow">{{ t("config.sections.customLlmEyebrow") }}</p>
          <h3 class="panel-title">
            {{ llmEditId ? t("config.sections.editLlmModel") : t("config.sections.addCustomProvider") }}
          </h3>
        </div>
        <button class="btn btn-secondary" type="button" @click="closeLlmModal">
          {{ t("config.sections.closeModal") }}
        </button>
      </div>

      <div class="form-grid two-columns">
        <label class="field">
          <span>{{ t("config.sections.displayName") }}</span>
          <input v-model="llmDraft.name" class="text-input" type="text" />
        </label>

        <label class="field">
          <span>{{ t("config.sections.keyName") }}</span>
          <input v-model="llmDraft.key_name" class="text-input" type="text" />
        </label>

        <label class="field field--wide">
          <span>{{ t("config.sections.apiKey") }}</span>
          <input v-model="llmDraft.api_key" class="text-input" type="password" />
        </label>

        <label class="field">
          <span>{{ t("config.sections.apiBase") }}</span>
          <input v-model="llmDraft.api_base" class="text-input" type="text" />
        </label>

        <label class="field">
          <span>{{ t("config.sections.modelName") }}</span>
          <input v-model="llmDraft.model" class="text-input" type="text" />
        </label>

        <label class="field">
          <span>{{ t("config.sections.protocolType") }}</span>
          <select v-model="llmDraft.protocol_type" class="text-input">
            <option value="openai">openai</option>
            <option value="anthropic">anthropic</option>
            <option value="custom">custom</option>
          </select>
        </label>

        <label class="field">
          <span>{{ t("config.sections.maxTokens") }}</span>
          <input v-model.number="llmDraft.max_tokens" class="text-input" type="number" min="1" />
        </label>

        <label class="checkbox-card">
          <input v-model="llmDraft.enabled" type="checkbox" />
          <span>{{ t("config.sections.enableProvider") }}</span>
        </label>
      </div>

      <div class="config-actions-bar">
        <button class="btn btn-primary" type="button" @click="saveLlmDraft">
          {{ t("common.save") }}
        </button>
        <button class="btn btn-secondary" type="button" @click="closeLlmModal">
          {{ t("config.sections.cancelEdit") }}
        </button>
      </div>
    </section>
  </div>

  <div v-if="publishTargetModalOpen" class="config-modal-backdrop" @click.self="closePublishTargetModal">
    <section class="config-modal config-modal--wide">
      <div class="config-modal__header">
        <div>
          <p class="eyebrow">{{ t("config.sections.publishPlatformsEyebrow") }}</p>
          <h3 class="panel-title">
            {{
              publishTargetEditId
                ? t("config.sections.editPublishPlatform")
                : t("config.sections.addPublishPlatform")
            }}
          </h3>
        </div>
        <button class="btn btn-secondary" type="button" @click="closePublishTargetModal">
          {{ t("config.sections.closeModal") }}
        </button>
      </div>

      <div class="form-grid two-columns">
        <label class="field">
          <span>{{ t("common.name") }}</span>
          <input v-model="publishTargetDraft.name" class="text-input" type="text" />
        </label>

        <label class="field">
          <span>{{ t("config.sections.platformType") }}</span>
          <select v-model="publishTargetDraft.platform_type" class="text-input">
            <option v-for="option in publishPlatformTypeOptions" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
        </label>

        <label class="field">
          <span>{{ t("config.sections.accountName") }}</span>
          <input v-model="publishTargetDraft.account_name" class="text-input" type="text" />
        </label>

        <label class="field">
          <span>{{ t("config.sections.publishUrl") }}</span>
          <input v-model="publishTargetDraft.publish_url" class="text-input" type="text" />
        </label>

        <label class="field field--wide">
          <span>{{ t("config.sections.cookies") }}</span>
          <textarea v-model="publishTargetDraft.cookies" class="text-area" />
        </label>

        <label class="field">
          <span>{{ t("config.sections.outputFormat") }}</span>
          <select v-model="publishTargetDraft.article_format" class="text-input">
            <option value="html">{{ t("config.formats.html") }}</option>
            <option value="md">{{ t("config.formats.markdown") }}</option>
            <option value="txt">{{ t("config.formats.text") }}</option>
          </select>
        </label>

        <label class="field">
          <span>{{ t("config.fields.templateCategory") }}</span>
          <select v-model="publishTargetDraft.template_category" class="text-input">
            <option v-for="category in templateStore.categories" :key="category.name" :value="category.name">
              {{ category.name }}
            </option>
          </select>
        </label>

        <label class="field">
          <span>{{ t("config.fields.templateName") }}</span>
          <select v-model="publishTargetDraft.template_name" class="text-input">
            <option
              v-for="template in publishTargetTemplateOptions"
              :key="template.relative_path"
              :value="template.name"
            >
              {{ template.name }}
            </option>
          </select>
        </label>

        <label class="field">
          <span>{{ t("config.fields.minArticleLength") }}</span>
          <input v-model.number="publishTargetDraft.min_article_len" class="text-input" type="number" />
        </label>

        <label class="field">
          <span>{{ t("config.fields.maxArticleLength") }}</span>
          <input v-model.number="publishTargetDraft.max_article_len" class="text-input" type="number" />
        </label>
      </div>

      <div class="toggle-grid">
        <label class="checkbox-card">
          <input v-model="publishTargetDraft.enabled" type="checkbox" />
          <span>{{ t("common.enabled") }}</span>
        </label>
        <label class="checkbox-card">
          <input v-model="publishTargetDraft.use_template" type="checkbox" />
          <span>{{ t("config.toggles.useTemplate") }}</span>
        </label>
        <label class="checkbox-card">
          <input v-model="publishTargetDraft.use_compress" type="checkbox" />
          <span>{{ t("config.toggles.useCompress") }}</span>
        </label>
        <label class="checkbox-card">
          <input v-model="publishTargetDraft.auto_publish" type="checkbox" />
          <span>{{ t("config.toggles.autoPublish") }}</span>
        </label>
        <label class="checkbox-card">
          <input v-model="publishTargetDraft.format_publish" type="checkbox" />
          <span>{{ t("config.toggles.formatPublish") }}</span>
        </label>
      </div>

      <div class="config-actions-bar">
        <button class="btn btn-primary" type="button" @click="savePublishTargetDraft">
          {{ t("common.save") }}
        </button>
        <button class="btn btn-secondary" type="button" @click="closePublishTargetModal">
          {{ t("config.sections.cancelEdit") }}
        </button>
      </div>
    </section>
  </div>
</template>
