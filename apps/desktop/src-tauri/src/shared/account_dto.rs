//! Redacted account responses the webview reads, shared by the iOS host and the three desktop hosts.
//!
//! The status, user, provider, challenge and profile shapes live in [`msime_client_core::account::page`], because the native Windows settings window returns the same ones through the C ABI (`msime_client_account`). The chat and preference-schema shapes below are only returned by the iOS and Android account commands.

#[cfg_attr(
    not(any(target_os = "ios", target_os = "android")),
    allow(unused_imports)
)]
pub(crate) use msime_client_core::account::page::providers_response_without_apple;
pub(crate) use msime_client_core::account::page::{
    providers_response, ChallengeResponse, ProfileResponse, ProvidersResponse, StatusResponse,
};
use msime_client_core::account::{AccountChatModels, AccountPreferenceSchema};
use serde::Serialize;
use std::collections::BTreeMap;

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModelResponse {
    id: String,
}

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModelsResponse {
    data: Vec<ChatModelResponse>,
    default_model: String,
}

impl From<AccountChatModels> for ChatModelsResponse {
    fn from(models: AccountChatModels) -> Self {
        Self {
            data: models
                .data
                .into_iter()
                .map(|model| ChatModelResponse { id: model.id })
                .collect(),
            default_model: models.default_model,
        }
    }
}

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub(crate) content: String,
}

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceSchemaResponse {
    fields: BTreeMap<String, msime_client_core::account::AccountPreferenceField>,
    maximum_bytes: usize,
    update_mode: String,
    revision_required: bool,
}

impl From<AccountPreferenceSchema> for PreferenceSchemaResponse {
    fn from(schema: AccountPreferenceSchema) -> Self {
        Self {
            fields: schema.fields,
            maximum_bytes: schema.maximum_bytes,
            update_mode: schema.update_mode,
            revision_required: schema.revision_required,
        }
    }
}
