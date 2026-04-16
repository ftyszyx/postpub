<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  apiDelete,
  apiGet,
  apiPost,
  encodePathSegments,
  type ApiResponse,
} from "../api/client";
import {
  Button as AButton,
  Empty as AEmpty,
  Form as AForm,
  FormItem as AFormItem,
  Input as AInput,
  InputNumber as AInputNumber,
  Modal as AModal,
  Select as ASelect,
  Space as ASpace,
  Switch as ASwitch,
  Table as ATable,
  Tag as ATag,
  message,
} from "ant-design-vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { LLM_MAX_TOKENS_LIMIT, useConfigStore } from "../stores/config";
import { useNavigationStore, type ConfigPanel } from "../stores/navigation";
import { useTemplateStore } from "../stores/templates";
import type {
  BrowserEnvironmentStatus,
  CustomLlmProvider,
  ImageModelProvider,
  PublishTargetConfig,
  PublishTargetLoginStatus,
  TemplateSummary,
} from "../types/postpub";

const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const configStore = useConfigStore();
const navigationStore = useNavigationStore();
const templateStore = useTemplateStore();

const activeSection = computed<ConfigPanel>(
  () => navigationStore.activeConfigPanel,
);
const imageModels = computed(() => configStore.bundle.config.img_api.providers);
const customProviders = computed(
  () => configStore.bundle.ui_config.custom_llm_providers,
);
const publishTargets = computed(
  () => configStore.bundle.config.publish_targets,
);

const imageModelModalOpen = ref(false);
const imageModelEditId = ref<string | null>(null);
const imageModelDraft = ref<ImageModelProvider>(createImageModelDraft());

const llmModalOpen = ref(false);
const llmEditId = ref<string | null>(null);
const llmDraft = ref<CustomLlmProvider>(createLlmDraft());

const publishTargetModalOpen = ref(false);
const publishTargetEditId = ref<string | null>(null);
const publishTargetDraft = ref<PublishTargetConfig>(createPublishTargetDraft());
const publishTargetBrowserStatus = ref<BrowserEnvironmentStatus | null>(null);
const publishTargetBrowserLoading = ref(false);
const publishTargetBrowserError = ref("");
const publishTargetBrowserMessage = ref("");
const publishTargetLoginStatus = ref<PublishTargetLoginStatus | null>(null);
const publishTargetLoginCheckingId = ref("");
const publishTargetOpeningId = ref("");

const publishTargetTemplateOptions = computed(() =>
  templateStore.templates.filter(
    (template: TemplateSummary) =>
      template.category === publishTargetDraft.value.template_category,
  ),
);

const imageModelTypeOptions = computed(() => [
  { value: "picsum", label: t("config.sections.imageProviderPicsum") },
  { value: "ali", label: t("config.sections.imageProviderAli") },
]);

const llmProtocolOptions = computed(() => [
  { value: "openai", label: t("config.sections.llmProtocolOpenAi") },
  {
    value: "openai_compatible",
    label: t("config.sections.llmProtocolOpenAiCompatible"),
  },
  { value: "ollama", label: t("config.sections.llmProtocolOllama") },
]);

const publishPlatformTypeOptions = computed(() => [
  { value: "wechat", label: t("config.sections.publishPlatformTypeWechat") },
]);

const wechatCoverStrategyOptions = computed(() => [
  {
    value: "article_cover",
    label: t("config.sections.wechatCoverStrategyArticleCover"),
  },
  {
    value: "first_image",
    label: t("config.sections.wechatCoverStrategyFirstImage"),
  },
  {
    value: "custom_path",
    label: t("config.sections.wechatCoverStrategyCustomPath"),
  },
  {
    value: "platform_ai",
    label: t("config.sections.wechatCoverStrategyPlatformAi"),
  },
  { value: "manual", label: t("config.sections.wechatCoverStrategyManual") },
]);

const wechatCommentModeOptions = computed(() => [
  {
    value: "auto_selected_open",
    label: t("config.sections.wechatCommentModeAutoSelectedOpen"),
  },
  { value: "open_all", label: t("config.sections.wechatCommentModeOpenAll") },
  { value: "closed", label: t("config.sections.wechatCommentModeClosed") },
]);

const publishTargetCategoryOptions = computed(() =>
  templateStore.categories.map((category) => ({
    value: category.name,
    label: category.name,
  })),
);

const publishTargetTemplateNameOptions = computed(() =>
  publishTargetTemplateOptions.value.map((template) => ({
    value: template.name,
    label: template.name,
  })),
);

const imageModelColumns = computed(() => [
  { title: t("common.name"), key: "name", dataIndex: "name", ellipsis: true },
  {
    title: t("config.sections.providerType"),
    key: "provider_type",
    dataIndex: "provider_type",
    width: 180,
  },
  {
    title: t("config.sections.modelName"),
    key: "model",
    dataIndex: "model",
    ellipsis: true,
  },
  { title: t("common.status"), key: "status", width: 140 },
  {
    title: t("common.actions"),
    key: "actions",
    width: 240,
    fixed: "right" as const,
  },
]);

const llmModelColumns = computed(() => [
  {
    title: t("config.sections.displayName"),
    key: "name",
    dataIndex: "name",
    ellipsis: true,
  },
  {
    title: t("config.sections.protocolType"),
    key: "protocol_type",
    dataIndex: "protocol_type",
    width: 140,
  },
  {
    title: t("config.sections.modelName"),
    key: "model",
    dataIndex: "model",
    ellipsis: true,
  },
  {
    title: t("config.sections.apiBase"),
    key: "api_base",
    dataIndex: "api_base",
    ellipsis: true,
  },
  { title: t("config.sections.apiKey"), key: "api_key_status", width: 180 },
  { title: t("common.status"), key: "status", width: 120 },
  {
    title: t("common.actions"),
    key: "actions",
    width: 360,
    fixed: "right" as const,
  },
]);

const publishPlatformColumns = computed(() => [
  { title: t("common.name"), key: "name", dataIndex: "name", ellipsis: true },
  {
    title: t("config.sections.platformType"),
    key: "platform_type",
    dataIndex: "platform_type",
    width: 160,
  },
  {
    title: t("config.sections.accountName"),
    key: "account_name",
    dataIndex: "account_name",
    ellipsis: true,
  },
  {
    title: t("config.sections.publishUrl"),
    key: "publish_url",
    dataIndex: "publish_url",
    ellipsis: true,
  },
  {
    title: t("config.sections.templateRule"),
    key: "template_rule",
    width: 220,
    ellipsis: true,
  },
  {
    title: t("config.sections.wechatSettingsSummary"),
    key: "wechat_settings",
    width: 280,
    ellipsis: true,
  },
  { title: t("common.status"), key: "status", width: 120 },
  {
    title: t("common.actions"),
    key: "actions",
    width: 260,
    fixed: "right" as const,
  },
]);

function createImageModelDraft(
  index = imageModels.value.length + 1,
): ImageModelProvider {
  return {
    id: `image-model-${Date.now()}-${index}`,
    name: t("config.sections.imageModelDraftName", { index }),
    provider_type: "picsum",
    api_key: "",
    model: "",
    enabled: true,
  };
}

function createLlmDraft(
  index = customProviders.value.length + 1,
): CustomLlmProvider {
  return {
    id: `custom-${Date.now()}-${index}`,
    name: t("config.sections.customProviderName", { index }),
    api_key: "",
    api_base: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
    protocol_type: "openai",
    max_tokens: 8192,
    enabled: index === 1,
  };
}

function createPublishTargetDraft(
  index = publishTargets.value.length + 1,
): PublishTargetConfig {
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
    format_publish: true,
    wechat: {
      cover_strategy: "article_cover",
      cover_path: "",
      cover_width: 900,
      cover_height: 383,
      declare_original: false,
      enable_reward: false,
      enable_paid: false,
      comment_mode: "auto_selected_open",
      collection_id: "",
      source_url: "",
      source_label: "",
      platform_recommendation_enabled: true,
    },
  };
}

function ensurePublishTargetTemplateSelection() {
  const draft = publishTargetDraft.value;
  const matched = publishTargetTemplateOptions.value.find(
    (template: TemplateSummary) => template.name === draft.template_name,
  );
  if (!matched) {
    draft.template_name = publishTargetTemplateOptions.value[0]?.name || "";
  }
}

function asImageModel(record: unknown) {
  return record as ImageModelProvider;
}

function asLlmProvider(record: unknown) {
  return record as CustomLlmProvider;
}

function asPublishTarget(record: unknown) {
  return record as PublishTargetConfig;
}

function imageProviderTypeLabel(providerType: string) {
  return (
    imageModelTypeOptions.value.find((option) => option.value === providerType)
      ?.label || providerType
  );
}

function publishPlatformTypeLabel(platformType: string) {
  return (
    publishPlatformTypeOptions.value.find(
      (option) => option.value === platformType,
    )?.label || platformType
  );
}

function enabledTag(enabled: boolean) {
  return enabled
    ? { color: "success", label: t("config.sections.providerActive") }
    : { color: "default", label: t("config.sections.providerInactive") };
}

function llmProviderTag(provider: CustomLlmProvider) {
  return provider.enabled
    ? { color: "success", label: t("config.sections.currentInUse") }
    : { color: "default", label: t("config.sections.providerInactive") };
}

function llmProtocolLabel(protocolType: string) {
  return (
    llmProtocolOptions.value.find((option) => option.value === protocolType)
      ?.label || protocolType
  );
}

function llmApiKeyTag(provider: CustomLlmProvider) {
  if (provider.api_key.trim()) {
    return { color: "success", label: t("config.sections.apiKeySaved") };
  }

  return { color: "error", label: t("config.sections.apiKeyMissing") };
}

function imageModelTag(provider: ImageModelProvider) {
  if (configStore.bundle.config.img_api.active_provider_id === provider.id) {
    return { color: "success", label: t("config.sections.currentInUse") };
  }

  return provider.enabled
    ? { color: "processing", label: t("config.sections.providerAvailable") }
    : { color: "default", label: t("config.sections.providerInactive") };
}

function publishTargetTemplateRule(target: PublishTargetConfig) {
  return `${target.template_category} / ${target.template_name || "-"}`;
}

function wechatCoverStrategyLabel(strategy: string) {
  return (
    wechatCoverStrategyOptions.value.find((option) => option.value === strategy)
      ?.label || strategy
  );
}

function wechatCommentModeLabel(mode: string) {
  return (
    wechatCommentModeOptions.value.find((option) => option.value === mode)
      ?.label || mode
  );
}

function publishTargetWechatSummary(target: PublishTargetConfig) {
  if (target.platform_type !== "wechat") {
    return "-";
  }

  const parts = [
    wechatCoverStrategyLabel(target.wechat.cover_strategy),
    wechatCommentModeLabel(target.wechat.comment_mode),
  ];

  if (target.wechat.declare_original) {
    parts.push(t("config.sections.wechatDeclareOriginalEnabled"));
  }

  if (!target.wechat.platform_recommendation_enabled) {
    parts.push(t("config.sections.wechatPlatformRecommendationDisabled"));
  }

  return parts.join(" / ");
}

function formatPublishTargetLoginCheckedAt(value?: string | null) {
  if (!value) {
    return "-";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleString();
}

function syncActiveImageProvider(providerId?: string) {
  const nextActiveProvider =
    imageModels.value.find((provider) => provider.id === providerId) ||
    imageModels.value.find(
      (provider) =>
        provider.id === configStore.bundle.config.img_api.active_provider_id,
    ) ||
    imageModels.value[0];

  configStore.bundle.config.img_api.active_provider_id =
    nextActiveProvider?.id || "";
  configStore.bundle.config.img_api.api_type =
    nextActiveProvider?.provider_type || "picsum";
}

async function setActiveImageModel(id: string) {
  configStore.bundle.config.img_api.providers = imageModels.value.map(
    (provider) => ({
      ...provider,
      enabled: provider.id === id ? true : provider.enabled,
    }),
  );
  syncActiveImageProvider(id);
  await persistConfigMutation();
}

function normalizeLlmProviders(
  providers: CustomLlmProvider[],
  preferredId?: string,
) {
  const nextProviders = providers.length ? providers : [createLlmDraft(1)];
  const activeProvider =
    nextProviders.find((provider) => provider.id === preferredId) ||
    nextProviders.find((provider) => provider.enabled) ||
    nextProviders[0];

  return nextProviders.map((provider) => ({
    ...provider,
    enabled: provider.id === activeProvider.id,
  }));
}

async function persistConfigMutation() {
  await configStore.save();
}

async function removeImageModel(id: string) {
  configStore.bundle.config.img_api.providers = imageModels.value.filter(
    (provider) => provider.id !== id,
  );
  syncActiveImageProvider();
  await persistConfigMutation();
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

async function saveImageModelDraft() {
  const draft = { ...imageModelDraft.value };
  if (!draft.name.trim()) {
    draft.name = t("config.sections.imageModelFallbackName");
  }

  if (imageModelEditId.value) {
    configStore.bundle.config.img_api.providers = imageModels.value.map(
      (provider) => (provider.id === imageModelEditId.value ? draft : provider),
    );
  } else {
    configStore.bundle.config.img_api.providers.push(draft);
  }

  syncActiveImageProvider();
  await persistConfigMutation();
  if (!configStore.error) {
    closeImageModelModal();
  }
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

async function saveLlmDraft() {
  const draft = {
    ...llmDraft.value,
    max_tokens: Math.min(
      Math.max(Math.trunc(llmDraft.value.max_tokens || 8192), 1),
      LLM_MAX_TOKENS_LIMIT,
    ),
  };
  if (!draft.name.trim()) {
    draft.name = t("config.sections.customProviderFallbackName");
  }

  let nextProviders: CustomLlmProvider[];
  if (llmEditId.value) {
    nextProviders = customProviders.value.map((provider) =>
      provider.id === llmEditId.value ? draft : provider,
    );
  } else {
    nextProviders = [...customProviders.value, draft];
  }

  configStore.bundle.ui_config.custom_llm_providers = normalizeLlmProviders(
    nextProviders,
    draft.enabled ? draft.id : undefined,
  );

  await persistConfigMutation();
  if (!configStore.error) {
    closeLlmModal();
  }
}

async function setActiveLlmProvider(id: string) {
  configStore.bundle.ui_config.custom_llm_providers = normalizeLlmProviders(
    customProviders.value,
    id,
  );
  await persistConfigMutation();
}

async function removeCustomProvider(id: string) {
  const remaining = customProviders.value.filter(
    (provider) => provider.id !== id,
  );
  configStore.bundle.ui_config.custom_llm_providers =
    normalizeLlmProviders(remaining);
  await persistConfigMutation();
}

function openCreatePublishTargetModal() {
  publishTargetEditId.value = null;
  publishTargetDraft.value = createPublishTargetDraft();
  publishTargetBrowserStatus.value = null;
  publishTargetBrowserError.value = "";
  publishTargetBrowserMessage.value = "";
  publishTargetLoginStatus.value = null;
  ensurePublishTargetTemplateSelection();
  publishTargetModalOpen.value = true;
}

function openEditPublishTargetModal(target: PublishTargetConfig) {
  publishTargetEditId.value = target.id;
  publishTargetDraft.value = {
    ...target,
    wechat: { ...target.wechat },
  };
  publishTargetBrowserError.value = "";
  publishTargetBrowserMessage.value = "";
  publishTargetLoginStatus.value = null;
  ensurePublishTargetTemplateSelection();
  publishTargetModalOpen.value = true;
  void loadPublishTargetBrowserStatus(target.id);
}

function closePublishTargetModal() {
  publishTargetModalOpen.value = false;
  publishTargetEditId.value = null;
  publishTargetBrowserStatus.value = null;
  publishTargetBrowserLoading.value = false;
  publishTargetBrowserError.value = "";
  publishTargetBrowserMessage.value = "";
  publishTargetLoginStatus.value = null;
}

async function savePublishTargetDraft() {
  const draft = {
    ...publishTargetDraft.value,
    wechat: { ...publishTargetDraft.value.wechat },
  };
  if (!draft.name.trim()) {
    draft.name = t("config.sections.publishPlatformFallbackName");
  }

  if (publishTargetEditId.value) {
    configStore.bundle.config.publish_targets = publishTargets.value.map(
      (target) => (target.id === publishTargetEditId.value ? draft : target),
    );
  } else {
    configStore.bundle.config.publish_targets.push(draft);
  }

  await persistConfigMutation();
  if (!configStore.error) {
    closePublishTargetModal();
  }
}

async function removePublishTarget(id: string) {
  configStore.bundle.config.publish_targets = publishTargets.value.filter(
    (target) => target.id !== id,
  );
  await persistConfigMutation();
}

async function openPublishTargetHomepage(target: PublishTargetConfig) {
  publishTargetOpeningId.value = target.id;

  try {
    await apiPost<
      ApiResponse<{ target_id: string; profile_dir: string; url: string }>
    >(`/api/system/browser/open/${encodePathSegments(target.id)}`);

    const successMessage = t("config.sections.openPublishPlatformSuccess", {
      name: target.name || target.id,
    });
    message.success(successMessage);

    if (publishTargetEditId.value === target.id) {
      publishTargetBrowserMessage.value = successMessage;
      publishTargetBrowserError.value = "";
      await loadPublishTargetBrowserStatus(target.id);
    }
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    message.error(
      t("config.sections.openPublishPlatformFailed", {
        error: detail,
      }),
    );

    if (publishTargetEditId.value === target.id) {
      publishTargetBrowserMessage.value = "";
      publishTargetBrowserError.value = detail;
    }
  } finally {
    publishTargetOpeningId.value = "";
  }
}

async function checkPublishTargetLogin(target: PublishTargetConfig) {
  publishTargetLoginCheckingId.value = target.id;

  try {
    const response = await apiPost<ApiResponse<PublishTargetLoginStatus>>(
      `/api/publish/targets/${encodePathSegments(target.id)}/login-status`,
    );
    const status = response.data;
    const targetName = status.target_name || target.name || target.id;

    if (publishTargetEditId.value === target.id) {
      publishTargetLoginStatus.value = status;
    }

    if (status.valid) {
      message.success(
        t("config.sections.loginCheckSuccess", {
          name: targetName,
        }),
      );
      return;
    }

    message.warning(
      t("config.sections.loginCheckInvalid", {
        name: targetName,
      }),
    );

    if (status.needs_login) {
      AModal.confirm({
        title: t("config.sections.loginCheckOpenLoginTitle"),
        content: t("config.sections.loginCheckOpenLoginBody", {
          target: targetName,
          detail:
            status.detail?.trim() ||
            t("config.sections.loginCheckUnknownDetail"),
        }),
        okText: t("config.sections.loginCheckOpenLoginOk"),
        cancelText: t("config.sections.loginCheckOpenLoginCancel"),
        onOk: async () => {
          await openPublishTargetHomepage(target);
        },
      });
    }
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    message.error(
      t("config.sections.loginCheckFailed", {
        error: detail,
      }),
    );
  } finally {
    publishTargetLoginCheckingId.value = "";
  }
}

async function checkCurrentPublishTargetLogin() {
  if (!publishTargetEditId.value) {
    return;
  }

  await checkPublishTargetLogin({
    ...publishTargetDraft.value,
    id: publishTargetEditId.value,
    wechat: { ...publishTargetDraft.value.wechat },
  });
}

async function loadPublishTargetBrowserStatus(targetId?: string | null) {
  if (!targetId) {
    publishTargetBrowserStatus.value = null;
    publishTargetBrowserLoading.value = false;
    publishTargetBrowserError.value = "";
    return;
  }

  publishTargetBrowserLoading.value = true;
  publishTargetBrowserError.value = "";

  try {
    const response = await apiGet<ApiResponse<BrowserEnvironmentStatus>>(
      "/api/system/browser",
      {
        target_id: targetId,
      },
    );
    publishTargetBrowserStatus.value = response.data;
  } catch (error) {
    publishTargetBrowserError.value =
      error instanceof Error ? error.message : String(error);
  } finally {
    publishTargetBrowserLoading.value = false;
  }
}

async function clearPublishTargetBrowserProfile() {
  if (!publishTargetEditId.value) {
    return;
  }

  publishTargetBrowserLoading.value = true;
  publishTargetBrowserError.value = "";
  publishTargetBrowserMessage.value = "";

  try {
    await apiDelete<ApiResponse<{ target_id: string; profile_dir: string }>>(
      `/api/system/browser/profiles/${encodePathSegments(publishTargetEditId.value)}`,
    );
    publishTargetBrowserMessage.value = t(
      "config.sections.browserProfileCleared",
    );
    await loadPublishTargetBrowserStatus(publishTargetEditId.value);
  } catch (error) {
    publishTargetBrowserError.value =
      error instanceof Error ? error.message : String(error);
  } finally {
    publishTargetBrowserLoading.value = false;
  }
}

async function restoreDefaults() {
  await configStore.loadDefaults();
}

onMounted(async () => {
  const legacyPanel = Array.isArray(route.query.panel)
    ? route.query.panel[0]
    : route.query.panel;
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
  },
);

watch(
  () => publishTargetDraft.value.template_category,
  () => {
    ensurePublishTargetTemplateSelection();
  },
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
          <select
            v-model="configStore.bundle.ui_config.theme"
            class="text-input"
          >
            <option value="light">{{ t("config.sections.themeLight") }}</option>
            <option value="dark">{{ t("config.sections.themeDark") }}</option>
          </select>
          <small class="field-help">{{ t("config.sections.themeHelp") }}</small>
        </label>

        <label class="field">
          <span>{{ t("config.sections.windowMode") }}</span>
          <select
            v-model="configStore.bundle.ui_config.window_mode"
            class="text-input"
          >
            <option value="STANDARD">
              {{ t("config.sections.windowModeStandard") }}
            </option>
            <option value="MAXIMIZED">
              {{ t("config.sections.windowModeMaximized") }}
            </option>
          </select>
          <small class="field-help">{{
            t("config.sections.windowModeHelp")
          }}</small>
        </label>

        <label class="field">
          <span>{{ t("config.sections.designTheme") }}</span>
          <select
            v-model="configStore.bundle.ui_config.design_theme"
            class="text-input"
          >
            <option value="follow-system">
              {{ t("config.sections.designThemeFollowSystem") }}
            </option>
            <option value="default">
              {{ t("config.sections.designThemeDefault") }}
            </option>
          </select>
          <small class="field-help">{{
            t("config.sections.designThemeHelp")
          }}</small>
        </label>
      </div>

      <div class="config-actions-bar">
        <AButton
          type="primary"
          :loading="configStore.saving"
          @click="configStore.save"
        >
          {{ configStore.saving ? t("common.saving") : t("config.saveConfig") }}
        </AButton>
        <AButton @click="restoreDefaults">
          {{ t("common.restoreDefaults") }}
        </AButton>
      </div>
    </section>

    <section
      v-else-if="activeSection === 'image-models'"
      class="panel panel--config"
    >
      <div class="config-panel-header">
        <div>
          <p class="eyebrow">{{ t("config.sections.imageModelsEyebrow") }}</p>
          <h3 class="panel-title">
            {{ t("config.sections.imageModelsTitle") }}
          </h3>
          <p class="workspace-copy">
            {{ t("config.sections.imageModelsDescription") }}
          </p>
        </div>
        <AButton type="primary" @click="openCreateImageModelModal">
          {{ t("config.sections.addImageModel") }}
        </AButton>
      </div>

      <ATable
        class="config-table"
        :columns="imageModelColumns"
        :data-source="imageModels"
        :pagination="false"
        :scroll="{ x: 980 }"
        row-key="id"
      >
        <template #emptyText>
          <AEmpty :description="t('config.sections.imageModelsEmpty')" />
        </template>

        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'provider_type'">
            {{ imageProviderTypeLabel(asImageModel(record).provider_type) }}
          </template>

          <template v-else-if="column.key === 'model'">
            {{ asImageModel(record).model || "-" }}
          </template>

          <template v-else-if="column.key === 'status'">
            <ATag :color="imageModelTag(asImageModel(record)).color">
              {{ imageModelTag(asImageModel(record)).label }}
            </ATag>
          </template>

          <template v-else-if="column.key === 'actions'">
            <ASpace wrap size="small">
              <AButton
                v-if="
                  configStore.bundle.config.img_api.active_provider_id !==
                  asImageModel(record).id
                "
                size="small"
                @click="setActiveImageModel(asImageModel(record).id)"
              >
                {{ t("config.sections.setAsCurrent") }}
              </AButton>
              <AButton
                size="small"
                @click="openEditImageModelModal(asImageModel(record))"
              >
                {{ t("config.sections.editItem") }}
              </AButton>
              <AButton
                danger
                size="small"
                @click="removeImageModel(asImageModel(record).id)"
              >
                {{ t("common.delete") }}
              </AButton>
            </ASpace>
          </template>
        </template>
      </ATable>
    </section>

    <section
      v-else-if="activeSection === 'llm-models'"
      class="panel panel--config"
    >
      <div class="config-panel-header">
        <div>
          <p class="eyebrow">{{ t("config.sections.customLlmEyebrow") }}</p>
          <h3 class="panel-title">{{ t("config.sections.customLlmTitle") }}</h3>
          <p class="workspace-copy">
            {{ t("config.sections.customLlmDescription") }}
          </p>
        </div>
        <AButton type="primary" @click="openCreateLlmModal">
          {{ t("config.sections.addCustomProvider") }}
        </AButton>
      </div>

      <ATable
        class="config-table"
        :columns="llmModelColumns"
        :data-source="customProviders"
        :pagination="false"
        :scroll="{ x: 1080 }"
        row-key="id"
      >
        <template #emptyText>
          <AEmpty :description="t('config.sections.llmModelsEmpty')" />
        </template>

        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'protocol_type'">
            {{ llmProtocolLabel(asLlmProvider(record).protocol_type) }}
          </template>

          <template v-else-if="column.key === 'api_key_status'">
            <ATag :color="llmApiKeyTag(asLlmProvider(record)).color">
              {{ llmApiKeyTag(asLlmProvider(record)).label }}
            </ATag>
          </template>

          <template v-else-if="column.key === 'status'">
            <ATag :color="llmProviderTag(asLlmProvider(record)).color">
              {{ llmProviderTag(asLlmProvider(record)).label }}
            </ATag>
          </template>

          <template v-else-if="column.key === 'actions'">
            <ASpace wrap size="small">
              <AButton
                v-if="!asLlmProvider(record).enabled"
                size="small"
                @click="setActiveLlmProvider(asLlmProvider(record).id)"
              >
                {{ t("config.sections.setAsCurrent") }}
              </AButton>
              <AButton
                size="small"
                @click="openEditLlmModal(asLlmProvider(record))"
              >
                {{ t("config.sections.editItem") }}
              </AButton>
              <AButton
                danger
                size="small"
                @click="removeCustomProvider(asLlmProvider(record).id)"
              >
                {{ t("common.delete") }}
              </AButton>
            </ASpace>
          </template>
        </template>
      </ATable>
    </section>

    <section v-else class="panel panel--config">
      <div class="config-panel-header">
        <div>
          <p class="eyebrow">
            {{ t("config.sections.publishPlatformsEyebrow") }}
          </p>
          <h3 class="panel-title">
            {{ t("config.sections.publishPlatformsTitle") }}
          </h3>
          <p class="workspace-copy">
            {{ t("config.sections.publishPlatformsDescription") }}
          </p>
        </div>
        <AButton type="primary" @click="openCreatePublishTargetModal">
          {{ t("config.sections.addPublishPlatform") }}
        </AButton>
      </div>

      <ATable
        class="config-table"
        :columns="publishPlatformColumns"
        :data-source="publishTargets"
        :pagination="false"
        :scroll="{ x: 1500 }"
        row-key="id"
      >
        <template #emptyText>
          <AEmpty :description="t('config.sections.publishPlatformsEmpty')" />
        </template>

        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'platform_type'">
            {{
              publishPlatformTypeLabel(asPublishTarget(record).platform_type)
            }}
          </template>

          <template v-else-if="column.key === 'account_name'">
            {{ asPublishTarget(record).account_name || "-" }}
          </template>

          <template v-else-if="column.key === 'template_rule'">
            {{ publishTargetTemplateRule(asPublishTarget(record)) }}
          </template>

          <template v-else-if="column.key === 'wechat_settings'">
            {{ publishTargetWechatSummary(asPublishTarget(record)) }}
          </template>

          <template v-else-if="column.key === 'status'">
            <ATag :color="enabledTag(asPublishTarget(record).enabled).color">
              {{ enabledTag(asPublishTarget(record).enabled).label }}
            </ATag>
          </template>

          <template v-else-if="column.key === 'actions'">
            <ASpace wrap size="small">
              <AButton
                size="small"
                :loading="
                  publishTargetLoginCheckingId === asPublishTarget(record).id
                "
                @click="checkPublishTargetLogin(asPublishTarget(record))"
              >
                {{
                  publishTargetLoginCheckingId === asPublishTarget(record).id
                    ? t("config.sections.testLoginChecking")
                    : t("config.sections.testLoginItem")
                }}
              </AButton>
              <AButton
                size="small"
                :loading="publishTargetOpeningId === asPublishTarget(record).id"
                @click="openPublishTargetHomepage(asPublishTarget(record))"
              >
                {{ t("config.sections.openItem") }}
              </AButton>
              <AButton
                size="small"
                @click="openEditPublishTargetModal(asPublishTarget(record))"
              >
                {{ t("config.sections.editItem") }}
              </AButton>
              <AButton
                danger
                size="small"
                @click="removePublishTarget(asPublishTarget(record).id)"
              >
                {{ t("common.delete") }}
              </AButton>
            </ASpace>
          </template>
        </template>
      </ATable>
    </section>

    <div v-if="configStore.lastMessage" class="banner banner-success">
      {{ configStore.lastMessage }}
    </div>
    <div v-if="configStore.error" class="banner banner-error">
      {{ configStore.error }}
    </div>
  </section>

  <AModal
    v-model:open="imageModelModalOpen"
    :title="
      imageModelEditId
        ? t('config.sections.editImageModel')
        : t('config.sections.addImageModel')
    "
    :ok-text="t('common.save')"
    :cancel-text="t('common.cancel')"
    width="720px"
    destroy-on-close
    @ok="saveImageModelDraft"
    @cancel="closeImageModelModal"
  >
    <AForm layout="vertical" class="config-ant-form">
      <div class="config-form-grid">
        <AFormItem :label="t('common.name')">
          <AInput v-model:value="imageModelDraft.name" />
        </AFormItem>

        <AFormItem :label="t('config.sections.providerType')">
          <ASelect
            v-model:value="imageModelDraft.provider_type"
            :options="imageModelTypeOptions"
          />
        </AFormItem>

        <AFormItem :label="t('config.sections.apiKey')">
          <AInput v-model:value="imageModelDraft.api_key" type="password" />
        </AFormItem>

        <AFormItem :label="t('config.sections.modelName')">
          <AInput v-model:value="imageModelDraft.model" />
        </AFormItem>

        <AFormItem class="config-form-grid__full" :label="t('common.enabled')">
          <ASwitch v-model:checked="imageModelDraft.enabled" />
        </AFormItem>
      </div>
    </AForm>
  </AModal>

  <AModal
    v-model:open="llmModalOpen"
    :title="
      llmEditId
        ? t('config.sections.editLlmModel')
        : t('config.sections.addCustomProvider')
    "
    :ok-text="t('common.save')"
    :cancel-text="t('common.cancel')"
    width="760px"
    destroy-on-close
    @ok="saveLlmDraft"
    @cancel="closeLlmModal"
  >
    <AForm layout="vertical" class="config-ant-form">
      <div class="config-form-grid">
        <AFormItem :label="t('config.sections.displayName')">
          <AInput v-model:value="llmDraft.name" />
        </AFormItem>

        <AFormItem
          class="config-form-grid__full"
          :label="t('config.sections.apiKey')"
        >
          <AInput v-model:value="llmDraft.api_key" type="password" />
        </AFormItem>

        <AFormItem :label="t('config.sections.apiBase')">
          <AInput v-model:value="llmDraft.api_base" />
        </AFormItem>

        <AFormItem :label="t('config.sections.modelName')">
          <AInput v-model:value="llmDraft.model" />
        </AFormItem>

        <AFormItem :label="t('config.sections.protocolType')">
          <ASelect
            v-model:value="llmDraft.protocol_type"
            :options="llmProtocolOptions"
          />
        </AFormItem>

        <AFormItem :label="t('config.sections.maxTokens')">
          <AInputNumber
            v-model:value="llmDraft.max_tokens"
            :min="1"
            :max="LLM_MAX_TOKENS_LIMIT"
            :precision="0"
            class="config-number-input"
          />
        </AFormItem>

        <AFormItem :label="t('config.sections.enableProvider')">
          <ASwitch v-model:checked="llmDraft.enabled" />
        </AFormItem>
      </div>
    </AForm>
  </AModal>

  <AModal
    v-model:open="publishTargetModalOpen"
    :title="
      publishTargetEditId
        ? t('config.sections.editPublishPlatform')
        : t('config.sections.addPublishPlatform')
    "
    :ok-text="t('common.save')"
    :cancel-text="t('common.cancel')"
    width="980px"
    destroy-on-close
    @ok="savePublishTargetDraft"
    @cancel="closePublishTargetModal"
  >
    <AForm layout="vertical" class="config-ant-form">
      <div class="config-form-grid">
        <AFormItem :label="t('common.name')">
          <AInput v-model:value="publishTargetDraft.name" />
        </AFormItem>

        <AFormItem :label="t('config.sections.platformType')">
          <ASelect
            v-model:value="publishTargetDraft.platform_type"
            :options="publishPlatformTypeOptions"
          />
        </AFormItem>

        <AFormItem :label="t('config.sections.accountName')">
          <AInput v-model:value="publishTargetDraft.account_name" />
        </AFormItem>

        <AFormItem :label="t('config.sections.publishUrl')">
          <AInput v-model:value="publishTargetDraft.publish_url" />
        </AFormItem>

        <AFormItem
          class="config-form-grid__full"
          :label="t('config.sections.cookies')"
        >
          <a-textarea v-model:value="publishTargetDraft.cookies" :rows="4" />
        </AFormItem>

        <AFormItem :label="t('config.sections.outputFormat')">
          <ASelect
            v-model:value="publishTargetDraft.article_format"
            :options="[
              { value: 'html', label: t('config.formats.html') },
              { value: 'md', label: t('config.formats.markdown') },
              { value: 'txt', label: t('config.formats.text') },
            ]"
          />
        </AFormItem>

        <AFormItem :label="t('config.fields.templateCategory')">
          <ASelect
            v-model:value="publishTargetDraft.template_category"
            :options="publishTargetCategoryOptions"
            :not-found-content="null"
          />
        </AFormItem>

        <AFormItem :label="t('config.fields.templateName')">
          <ASelect
            v-model:value="publishTargetDraft.template_name"
            :options="publishTargetTemplateNameOptions"
            :not-found-content="null"
          />
        </AFormItem>

        <AFormItem :label="t('config.fields.minArticleLength')">
          <AInputNumber
            v-model:value="publishTargetDraft.min_article_len"
            :min="1"
            :precision="0"
            class="config-number-input"
          />
        </AFormItem>

        <AFormItem :label="t('config.fields.maxArticleLength')">
          <AInputNumber
            v-model:value="publishTargetDraft.max_article_len"
            :min="1"
            :precision="0"
            class="config-number-input"
          />
        </AFormItem>
      </div>

      <div class="config-toggle-grid">
        <AFormItem :label="t('common.enabled')">
          <ASwitch v-model:checked="publishTargetDraft.enabled" />
        </AFormItem>
        <AFormItem :label="t('config.toggles.useTemplate')">
          <ASwitch v-model:checked="publishTargetDraft.use_template" />
        </AFormItem>
        <AFormItem :label="t('config.toggles.useCompress')">
          <ASwitch v-model:checked="publishTargetDraft.use_compress" />
        </AFormItem>
        <AFormItem :label="t('config.toggles.autoPublish')">
          <ASwitch v-model:checked="publishTargetDraft.auto_publish" />
        </AFormItem>
        <AFormItem :label="t('config.toggles.formatPublish')">
          <ASwitch v-model:checked="publishTargetDraft.format_publish" />
        </AFormItem>
      </div>

      <template v-if="publishTargetDraft.platform_type === 'wechat'">
        <div class="config-subsection">
          <p class="config-subsection__eyebrow">
            {{ t("config.sections.wechatSettingsEyebrow") }}
          </p>
          <h4 class="config-subsection__title">
            {{ t("config.sections.wechatSettingsTitle") }}
          </h4>
          <p class="config-subsection__description">
            {{ t("config.sections.wechatSettingsDescription") }}
          </p>
        </div>

        <div class="config-form-grid">
          <AFormItem :label="t('config.sections.wechatCoverStrategy')">
            <ASelect
              v-model:value="publishTargetDraft.wechat.cover_strategy"
              :options="wechatCoverStrategyOptions"
            />
          </AFormItem>

          <AFormItem :label="t('config.sections.wechatCommentMode')">
            <ASelect
              v-model:value="publishTargetDraft.wechat.comment_mode"
              :options="wechatCommentModeOptions"
            />
          </AFormItem>

          <AFormItem
            class="config-form-grid__full"
            :label="t('config.sections.wechatCoverPath')"
          >
            <AInput v-model:value="publishTargetDraft.wechat.cover_path" />
          </AFormItem>

          <AFormItem :label="t('config.sections.wechatCoverWidth')">
            <AInputNumber
              v-model:value="publishTargetDraft.wechat.cover_width"
              :min="1"
              :precision="0"
              class="config-number-input"
            />
          </AFormItem>

          <AFormItem :label="t('config.sections.wechatCoverHeight')">
            <AInputNumber
              v-model:value="publishTargetDraft.wechat.cover_height"
              :min="1"
              :precision="0"
              class="config-number-input"
            />
          </AFormItem>

          <AFormItem :label="t('config.sections.wechatCollectionId')">
            <AInput v-model:value="publishTargetDraft.wechat.collection_id" />
          </AFormItem>

          <AFormItem :label="t('config.sections.wechatSourceLabel')">
            <AInput v-model:value="publishTargetDraft.wechat.source_label" />
          </AFormItem>

          <AFormItem
            class="config-form-grid__full"
            :label="t('config.sections.wechatSourceUrl')"
          >
            <AInput v-model:value="publishTargetDraft.wechat.source_url" />
          </AFormItem>
        </div>

        <div class="config-toggle-grid">
          <AFormItem :label="t('config.sections.wechatDeclareOriginal')">
            <ASwitch
              v-model:checked="publishTargetDraft.wechat.declare_original"
            />
          </AFormItem>
          <AFormItem :label="t('config.sections.wechatEnableReward')">
            <ASwitch
              v-model:checked="publishTargetDraft.wechat.enable_reward"
            />
          </AFormItem>
          <AFormItem :label="t('config.sections.wechatEnablePaid')">
            <ASwitch v-model:checked="publishTargetDraft.wechat.enable_paid" />
          </AFormItem>
          <AFormItem
            :label="t('config.sections.wechatPlatformRecommendationEnabled')"
          >
            <ASwitch
              v-model:checked="
                publishTargetDraft.wechat.platform_recommendation_enabled
              "
            />
          </AFormItem>
        </div>

        <div class="config-subsection">
          <p class="config-subsection__eyebrow">
            {{ t("config.sections.browserEnvironmentEyebrow") }}
          </p>
          <h4 class="config-subsection__title">
            {{ t("config.sections.browserEnvironmentTitle") }}
          </h4>
          <p class="config-subsection__description">
            {{ t("config.sections.browserEnvironmentDescription") }}
          </p>
        </div>

        <template v-if="publishTargetEditId">
          <div
            v-if="publishTargetBrowserMessage"
            class="browser-status browser-status--success"
          >
            {{ publishTargetBrowserMessage }}
          </div>
          <div
            v-if="publishTargetBrowserError"
            class="browser-status browser-status--error"
          >
            {{ publishTargetBrowserError }}
          </div>

          <div v-if="publishTargetLoginStatus" class="browser-status-card">
            <div class="browser-status-grid">
              <div class="browser-status-item">
                <span class="browser-status-item__label">{{
                  t("config.sections.loginStatusTitle")
                }}</span>
                <strong>
                  {{
                    publishTargetLoginStatus.valid
                      ? t("config.sections.loginStatusValid")
                      : t("config.sections.loginStatusInvalid")
                  }}
                  <template v-if="publishTargetLoginStatus.needs_login">
                    · {{ t("config.sections.loginStatusNeedsLogin") }}
                  </template>
                </strong>
              </div>
              <div class="browser-status-item">
                <span class="browser-status-item__label">{{
                  t("config.sections.loginStatusLastChecked")
                }}</span>
                <strong>{{
                  formatPublishTargetLoginCheckedAt(
                    publishTargetLoginStatus.checked_at,
                  )
                }}</strong>
              </div>
            </div>

            <div class="browser-status-paths">
              <div
                v-if="publishTargetLoginStatus.current_url"
                class="browser-status-path"
              >
                <span class="browser-status-item__label">{{
                  t("config.sections.loginStatusCurrentUrl")
                }}</span>
                <code>{{ publishTargetLoginStatus.current_url }}</code>
              </div>
              <div
                v-if="publishTargetLoginStatus.detail"
                class="browser-status-path"
              >
                <span class="browser-status-item__label">{{
                  t("config.sections.loginStatusDetail")
                }}</span>
                <code>{{ publishTargetLoginStatus.detail }}</code>
              </div>
            </div>
          </div>

          <div class="browser-status-card">
            <div class="browser-status-grid">
              <div class="browser-status-item">
                <span class="browser-status-item__label">{{
                  t("config.sections.browserRemoteVersion")
                }}</span>
                <strong>{{
                  publishTargetBrowserStatus?.remote_version || "-"
                }}</strong>
              </div>
              <div class="browser-status-item">
                <span class="browser-status-item__label">{{
                  t("config.sections.browserLocalVersion")
                }}</span>
                <strong>{{
                  publishTargetBrowserStatus?.local_version || "-"
                }}</strong>
              </div>
              <div class="browser-status-item">
                <span class="browser-status-item__label">{{
                  t("config.sections.browserReady")
                }}</span>
                <strong>{{
                  publishTargetBrowserStatus?.browser_ready
                    ? t("config.sections.providerActive")
                    : t("config.sections.providerInactive")
                }}</strong>
              </div>
              <div class="browser-status-item">
                <span class="browser-status-item__label">{{
                  t("config.sections.browserProfileEntries")
                }}</span>
                <strong>{{
                  publishTargetBrowserStatus?.profile_entry_count ?? 0
                }}</strong>
              </div>
            </div>

            <div class="browser-status-paths">
              <div class="browser-status-path">
                <span class="browser-status-item__label">{{
                  t("config.sections.browserExecutablePath")
                }}</span>
                <code>{{
                  publishTargetBrowserStatus?.browser_executable || "-"
                }}</code>
              </div>
              <div class="browser-status-path">
                <span class="browser-status-item__label">{{
                  t("config.sections.browserProfilePath")
                }}</span>
                <code>{{
                  publishTargetBrowserStatus?.profile_dir || "-"
                }}</code>
              </div>
              <div class="browser-status-path">
                <span class="browser-status-item__label">{{
                  t("config.sections.browserConfigUrl")
                }}</span>
                <code>{{ publishTargetBrowserStatus?.config_url || "-" }}</code>
              </div>
              <div
                class="browser-status-path"
                v-if="publishTargetBrowserStatus?.remote_error"
              >
                <span class="browser-status-item__label">{{
                  t("config.sections.browserRemoteError")
                }}</span>
                <code>{{ publishTargetBrowserStatus.remote_error }}</code>
              </div>
            </div>

            <div class="config-actions-bar">
              <AButton
                :loading="publishTargetLoginCheckingId === publishTargetEditId"
                @click="checkCurrentPublishTargetLogin"
              >
                {{
                  publishTargetLoginCheckingId === publishTargetEditId
                    ? t("config.sections.testLoginChecking")
                    : t("config.sections.testLoginItem")
                }}
              </AButton>
              <AButton
                :loading="publishTargetBrowserLoading"
                @click="loadPublishTargetBrowserStatus(publishTargetEditId)"
              >
                {{ t("config.sections.refreshBrowserEnvironment") }}
              </AButton>
              <AButton
                danger
                :loading="publishTargetBrowserLoading"
                @click="clearPublishTargetBrowserProfile"
              >
                {{ t("config.sections.clearBrowserProfile") }}
              </AButton>
            </div>
          </div>
        </template>

        <div v-else class="browser-status browser-status--muted">
          {{ t("config.sections.browserEnvironmentSaveFirst") }}
        </div>
      </template>
    </AForm>
  </AModal>
</template>

<style scoped>
.config-table {
  margin-top: 12px;
}

.config-ant-form {
  margin-top: 12px;
}

.config-form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0 16px;
}

.config-form-grid__full {
  grid-column: 1 / -1;
}

.config-toggle-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0 16px;
}

.config-number-input {
  width: 100%;
}

.config-subsection {
  margin: 24px 0 12px;
}

.config-subsection__eyebrow {
  margin: 0 0 6px;
  color: var(--ink-muted);
  font-size: 12px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.config-subsection__title {
  margin: 0;
  color: var(--ink-strong);
  font-size: 16px;
}

.config-subsection__description {
  margin: 6px 0 0;
  color: var(--ink-muted);
  font-size: 13px;
}

.browser-status-card {
  display: grid;
  gap: 16px;
  margin-top: 12px;
  padding: 16px;
  border: 1px solid var(--line-soft);
  border-radius: 16px;
  background: var(--surface-raised);
}

.browser-status-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px 16px;
}

.browser-status-item {
  display: grid;
  gap: 4px;
}

.browser-status-item__label {
  color: var(--ink-muted);
  font-size: 12px;
}

.browser-status-paths {
  display: grid;
  gap: 10px;
}

.browser-status-path {
  display: grid;
  gap: 4px;
}

.browser-status-path code {
  display: block;
  padding: 10px 12px;
  border-radius: 12px;
  background: var(--surface-ground);
  color: var(--ink-strong);
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-all;
}

.browser-status {
  margin-top: 12px;
  padding: 12px 14px;
  border-radius: 12px;
  font-size: 13px;
}

.browser-status--success {
  background: color-mix(in srgb, var(--brand-primary) 10%, white);
  color: var(--ink-strong);
}

.browser-status--error {
  background: color-mix(in srgb, #d9485f 10%, white);
  color: #8c1d2c;
}

.browser-status--muted {
  margin-top: 12px;
  border: 1px dashed var(--line-soft);
  color: var(--ink-muted);
  background: var(--surface-ground);
}

.config-table :deep(.ant-table-thead > tr > th) {
  white-space: nowrap;
}

.config-table :deep(.ant-table-cell) {
  vertical-align: middle;
}

@media (max-width: 960px) {
  .config-form-grid,
  .config-toggle-grid,
  .browser-status-grid {
    grid-template-columns: 1fr;
  }
}
</style>
