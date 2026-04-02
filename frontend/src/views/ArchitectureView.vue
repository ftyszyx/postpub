<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <section class="content-grid">
    <article class="panel">
      <p class="eyebrow">{{ t("architecture.frontendEyebrow") }}</p>
      <h3 class="panel-title">{{ t("architecture.frontendTitle") }}</h3>
      <ul class="clean-list">
        <li>{{ t("architecture.frontendItems.one") }}</li>
        <li>{{ t("architecture.frontendItems.two") }}</li>
        <li>{{ t("architecture.frontendItems.three") }}</li>
        <li>{{ t("architecture.frontendItems.four") }}</li>
      </ul>
    </article>

    <article class="panel">
      <p class="eyebrow">{{ t("architecture.backendEyebrow") }}</p>
      <h3 class="panel-title">{{ t("architecture.backendTitle") }}</h3>
      <ul class="clean-list">
        <li>{{ t("architecture.backendItems.one") }}</li>
        <li>{{ t("architecture.backendItems.two") }}</li>
        <li>{{ t("architecture.backendItems.three") }}</li>
        <li>{{ t("architecture.backendItems.four") }}</li>
      </ul>
    </article>
  </section>

  <section class="panel">
    <p class="eyebrow">{{ t("architecture.aiforgeEyebrow") }}</p>
    <h3 class="panel-title">{{ t("architecture.aiforgeTitle") }}</h3>
    <div class="info-grid">
      <div class="info-row">
        <span>{{ t("architecture.aiforge.searchMode") }}</span>
        <code>{{ t("architecture.aiforge.searchValue") }}</code>
      </div>
      <div class="info-row">
        <span>{{ t("architecture.aiforge.referenceMode") }}</span>
        <code>{{ t("architecture.aiforge.referenceValue") }}</code>
      </div>
      <div class="info-row">
        <span>{{ t("architecture.aiforge.draftMode") }}</span>
        <code>{{ t("architecture.aiforge.draftValue") }}</code>
      </div>
      <div class="info-row">
        <span>{{ t("architecture.aiforge.templateMerge") }}</span>
        <code v-pre>{{title}} {{summary}} {{generated_at}} {{content}}</code>
      </div>
    </div>
  </section>

  <section class="content-grid">
    <article class="panel">
      <p class="eyebrow">{{ t("architecture.implementedEyebrow") }}</p>
      <ul class="clean-list">
        <li>{{ t("architecture.implementedItems.one") }}</li>
        <li>{{ t("architecture.implementedItems.two") }}</li>
        <li>{{ t("architecture.implementedItems.three") }}</li>
        <li>{{ t("architecture.implementedItems.four") }}</li>
        <li>{{ t("architecture.implementedItems.five") }}</li>
      </ul>
    </article>

    <article class="panel">
      <p class="eyebrow">{{ t("architecture.deferredEyebrow") }}</p>
      <ul class="clean-list">
        <li>{{ t("architecture.deferredItems.one") }}</li>
        <li>{{ t("architecture.deferredItems.two") }}</li>
        <li>{{ t("architecture.deferredItems.three") }}</li>
      </ul>
    </article>
  </section>
</template>
