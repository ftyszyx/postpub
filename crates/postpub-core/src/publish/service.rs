use async_trait::async_trait;
use postpub_types::{
    ArticleDocument, ArticleVariantDocument, PublishArticleRequest, PublishOutput,
    PublishTargetConfig,
};

use crate::{AppContext, PostpubError, Result};

use super::{PublishProgressReporter, WechatPublisher};
#[async_trait]
pub trait Publisher: Send + Sync {
    fn platform_type(&self) -> &'static str;

    async fn publish(
        &self,
        target: &PublishTargetConfig,
        article: &ArticleDocument,
        variant: &ArticleVariantDocument,
        request: &PublishArticleRequest,
        reporter: &PublishProgressReporter,
    ) -> Result<PublishOutput>;
}

pub struct PublishService {
    context: AppContext,
}

impl PublishService {
    pub fn new(context: AppContext) -> Self {
        Self { context }
    }

    pub async fn publish_with_progress(
        &self,
        request: PublishArticleRequest,
        reporter: PublishProgressReporter,
    ) -> Result<PublishOutput> {
        let mode = request.mode.trim().to_lowercase();
        if mode != "draft" && mode != "publish" {
            return Err(PostpubError::Validation(format!(
                "unsupported publish mode: {}",
                request.mode
            )));
        }

        reporter.report(
            "prepare",
            format!("加载文章 {}", request.article_relative_path),
        );
        let article = self
            .context
            .article_store()
            .get_article(&request.article_relative_path)?;

        reporter.report("target", format!("解析发布目标 {}", request.target_id));
        let bundle = self.context.config_store().load_bundle()?;
        let target = bundle
            .config
            .publish_targets
            .into_iter()
            .find(|item| item.id == request.target_id)
            .ok_or_else(|| {
                PostpubError::NotFound(format!("publish target not found: {}", request.target_id))
            })?;

        if !target.enabled {
            return Err(PostpubError::Conflict(format!(
                "publish target is disabled: {}",
                target.id
            )));
        }

        reporter.report("variant", format!("定位文章变体 {}", request.target_id));
        let variant = article
            .variants
            .iter()
            .find(|item| item.summary.target_id == request.target_id)
            .ok_or_else(|| {
                PostpubError::NotFound(format!(
                    "article variant not found for target: {}",
                    request.target_id
                ))
            })?;

        reporter.report(
            "platform",
            format!("调用平台适配器 {}", target.platform_type),
        );
        match target.platform_type.as_str() {
            "wechat" => {
                let publisher = WechatPublisher::new(self.context.clone());
                publisher
                    .publish(&target, &article, variant, &request, &reporter)
                    .await
            }
            other => Err(PostpubError::Validation(format!(
                "publish platform is not supported yet: {other}"
            ))),
        }
    }
}
