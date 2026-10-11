//! Cloud clipboard requests: the shape every host accepts ([`validate_request`]) and the account-backed answer ([`account_request`]).

use msime_client_core::account::{
    AccountApi, AccountClipboardError, AccountError, AccountSessionStorage, BackendAccountSession,
};
use msime_client_core::{
    has_disallowed_control_with_options, is_bounded_ascii_identifier, is_bounded_text,
    is_bounded_utf16, is_lower_hex,
};
use serde_json::{json, Value};

pub fn validate_request(request: &Value) -> Result<(), &'static str> {
    let operation = request
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("invalid cloud clipboard request")?;
    match operation {
        "list" => {
            let search = request.get("search").and_then(Value::as_str).unwrap_or("");
            if !is_bounded_text(search, 1024) {
                return Err("invalid cloud clipboard request");
            }
        }
        "add" => {
            let text = request
                .get("text")
                .and_then(Value::as_str)
                .ok_or("invalid cloud clipboard request")?;
            if text.is_empty()
                || !is_bounded_utf16(text, 4000)
                || has_disallowed_control_with_options(text, true)
            {
                return Err("invalid cloud clipboard request");
            }
        }
        "delete" => {
            let id = request
                .get("id")
                .and_then(Value::as_str)
                .ok_or("invalid cloud clipboard request")?;
            if !is_bounded_ascii_identifier(id, 256) {
                return Err("invalid cloud clipboard request");
            }
        }
        "set_enabled" => {
            if !request.get("enabled").is_some_and(Value::is_boolean) {
                return Err("invalid cloud clipboard request");
            }
        }
        // 端到端加密的设备管理，见 `docs/cloud-clipboard-e2e.md` 的「共享面板的动作」。
        "devices" | "reset" => {}
        "approve" | "reject" | "remove" => {
            let device_id = request
                .get("device_id")
                .and_then(Value::as_str)
                .ok_or("invalid cloud clipboard request")?;
            // 设备编号是公钥 SHA-256 的前 16 字节，小写十六进制。
            if !is_lower_hex(device_id, 32) {
                return Err("invalid cloud clipboard request");
            }
        }
        _ => return Err("invalid cloud clipboard request"),
    }
    Ok(())
}

/// One cloud clipboard action answered through the account session: the `list`, `set_enabled`, `add` and `delete` the 云剪贴板 panel sends, plus the device management of end-to-end encryption (`devices`, `approve`, `reject`, `remove`, `reset`, see `docs/cloud-clipboard-e2e.md`). Blocks on the network, so callers run it off their UI thread. The Tauri shells pass the session their `AccountState` holds; the native Windows settings window (`msime_client_account_cloud_clipboard`) passes one over the signed-in session file the shell writes. An action that is not one of these, or lacks its field, is `invalid_cloud_clipboard` and never reaches the backend; a failed call is the account error's code, `account_unauthorized` when no one is signed in, or the driver's code (for example `clipboard_device_pending`).
pub fn account_request<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    action: &Value,
) -> Result<Value, &'static str> {
    const INVALID: &str = "invalid_cloud_clipboard";
    let operation = action
        .get("operation")
        .and_then(Value::as_str)
        .ok_or(INVALID)?;
    let field = |name: &str| action.get(name).and_then(Value::as_str).ok_or(INVALID);
    match operation {
        "list" => {
            let search = action
                .get("search")
                .and_then(Value::as_str)
                .unwrap_or_default();
            clipboard_value(session.clipboard(search))
        }
        "set_enabled" => {
            let enabled = action
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or(INVALID)?;
            session
                .set_clipboard_enabled(enabled)
                .map(|()| json!({ "enabled": enabled }))
                .map_err(|error| error.code())
        }
        "add" => {
            let text = field("text")?;
            clipboard_value(session.add_clipboard(text))
        }
        "delete" => {
            let id = field("id")?;
            session
                .delete_clipboard(Some(id))
                .map(|()| json!({}))
                .map_err(|error| error.code())
        }
        "devices" => clipboard_value(session.clipboard_devices()),
        "approve" => clipboard_value(session.approve_clipboard_device(field("device_id")?)),
        "reject" => clipboard_value(session.reject_clipboard_device(field("device_id")?)),
        "remove" => clipboard_value(session.remove_clipboard_device(field("device_id")?)),
        "reset" => clipboard_value(session.reset_clipboard()),
        _ => Err(INVALID),
    }
}

/// 经端到端加密驱动的结果交给页面；驱动或账号的错误换成页面认得的错误码，序列化不了按服务不可用处理。
fn clipboard_value<T: serde::Serialize>(
    result: Result<T, AccountClipboardError>,
) -> Result<Value, &'static str> {
    let value = result.map_err(|error| error.code())?;
    serde_json::to_value(value).map_err(|_| AccountError::Unavailable.code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_list_add_delete_and_toggle_requests() {
        assert!(validate_request(&json!({"operation":"list","search":"水"})).is_ok());
        assert!(validate_request(&json!({"operation":"add","text":"line\nfeed"})).is_ok());
        assert!(validate_request(&json!({"operation":"delete","id":"entry-1"})).is_ok());
        assert!(validate_request(&json!({"operation":"set_enabled","enabled":true})).is_ok());
    }

    #[test]
    fn enforces_utf16_clipboard_bound() {
        assert!(validate_request(&json!({"operation":"add","text":"界".repeat(4000)})).is_ok());
        assert!(validate_request(&json!({"operation":"add","text":"界".repeat(4001)})).is_err());
    }

    #[test]
    fn accepts_device_management_with_a_well_formed_device_id() {
        let id = "0123456789abcdef".repeat(2);
        assert!(validate_request(&json!({"operation":"devices"})).is_ok());
        assert!(validate_request(&json!({"operation":"reset"})).is_ok());
        for operation in ["approve", "reject", "remove"] {
            assert!(validate_request(&json!({"operation":operation,"device_id":id})).is_ok());
            assert!(validate_request(&json!({"operation":operation})).is_err());
            for bad in [
                id.to_uppercase(),
                format!("{id}0"),
                id[..31].to_owned(),
                "g".repeat(32),
                "../".repeat(11)[..32].to_owned(),
            ] {
                assert!(
                    validate_request(&json!({"operation":operation,"device_id":bad})).is_err(),
                    "{operation} {bad}"
                );
            }
            assert!(validate_request(&json!({"operation":operation,"device_id":32})).is_err());
        }
    }

    #[test]
    fn rejects_unsafe_or_unknown_requests() {
        assert!(validate_request(&json!({"operation":"list","search":"bad\n"})).is_err());
        assert!(validate_request(&json!({"operation":"add","text":"bad\u{0007}"})).is_err());
        assert!(validate_request(&json!({"operation":"delete","id":"bad/id"})).is_err());
        assert!(validate_request(&json!({"operation":"unknown"})).is_err());
    }

    mod account {
        use super::super::account_request;
        use msime_client_core::account::{
            AccountApi, AccountChallenge, AccountError, AccountProfile, AccountSessionStorage,
            AccountTokens, AccountUser, BackendAccountSession, SavedAccountSession,
        };
        use msime_client_core::cloud::clipboard_e2e::{HttpRequest, HttpResponse};
        use serde_json::json;
        use std::sync::{Arc, Mutex};

        /// The session validates stored tokens as 64 lowercase hex digits, so the synthetic ones have that shape.
        const ACCESS_TOKEN: &str =
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        /// A backend that records the clipboard calls it receives; every call outside the clipboard is a test failure.
        struct FakeApi {
            calls: Arc<Mutex<Vec<String>>>,
        }

        impl FakeApi {
            fn record(&self, call: String, token: &str) {
                assert_eq!(token, ACCESS_TOKEN);
                self.calls.lock().unwrap().push(call);
            }
        }

        impl AccountApi for FakeApi {
            fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
                unreachable!("providers")
            }
            fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
                unreachable!("challenge")
            }
            fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
                unreachable!("login")
            }
            fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
                unreachable!("refresh")
            }
            fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
                unreachable!("profile")
            }
            fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
                unreachable!("rename")
            }
            fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
                unreachable!("logout")
            }
            fn delete_account(&self, _: &str) -> Result<(), AccountError> {
                unreachable!("delete_account")
            }
            /// 云剪贴板关着的账号：驱动先取密钥状态，看到 `enabled: false` 就停下，列表答「已关闭」，上传和设备操作答 `clipboard_sync_disabled`。
            fn clipboard_e2e(
                &self,
                request: &HttpRequest,
                token: &str,
            ) -> Result<HttpResponse, AccountError> {
                let path = request.path.split('?').next().unwrap_or_default();
                self.record(format!("{} {path}", request.method), token);
                Ok(HttpResponse {
                    status: 200,
                    body: json!({ "enabled": false, "key_id": null, "wrap": null, "devices": [] })
                        .to_string(),
                })
            }
            fn set_clipboard_enabled(
                &self,
                enabled: bool,
                token: &str,
            ) -> Result<(), AccountError> {
                self.record(format!("set_enabled:{enabled}"), token);
                Ok(())
            }
            fn delete_clipboard(&self, id: Option<&str>, token: &str) -> Result<(), AccountError> {
                self.record(format!("delete:{}", id.unwrap_or("*")), token);
                Ok(())
            }
        }

        struct MemoryStorage(Option<SavedAccountSession>);

        impl AccountSessionStorage for MemoryStorage {
            fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
                Ok(self.0.clone())
            }
            fn save(&self, _: &SavedAccountSession) -> Result<(), AccountError> {
                Ok(())
            }
            fn clear(&self) -> Result<(), AccountError> {
                Ok(())
            }
            fn load_clipboard_keystore(&self) -> Result<Option<String>, AccountError> {
                Ok(None)
            }
            fn save_clipboard_keystore(&self, _: &str) -> Result<(), AccountError> {
                Ok(())
            }
        }

        fn signed_in() -> Option<SavedAccountSession> {
            Some(SavedAccountSession {
                tokens: AccountTokens {
                    access_token: ACCESS_TOKEN.into(),
                    refresh_token: "b".repeat(64),
                    token_type: "Bearer".into(),
                    expires_in: 3600,
                    user: AccountUser {
                        id: "synthetic-user".into(),
                        display_name: "合成用户".into(),
                        created_at: "2026-01-01T00:00:00Z".into(),
                        email: None,
                        avatar_url: None,
                    },
                },
                // An hour ahead, past the early-refresh window, so the session never asks the backend to refresh.
                expires_at_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64
                    + 3_600_000,
                session_id: None,
            })
        }

        type Calls = Arc<Mutex<Vec<String>>>;

        fn session(
            saved: Option<SavedAccountSession>,
        ) -> (BackendAccountSession<FakeApi, MemoryStorage>, Calls) {
            let calls = Calls::default();
            let api = FakeApi {
                calls: Arc::clone(&calls),
            };
            (BackendAccountSession::new(api, MemoryStorage(saved)), calls)
        }

        #[test]
        fn each_operation_reaches_its_backend_call_and_answers_what_the_page_reads() {
            let (session, calls) = session(signed_in());
            let list = account_request(&session, &json!({ "operation": "list", "search": "合成" }))
                .unwrap();
            assert_eq!(list["enabled"], false);
            assert_eq!(list["items"], json!([]));
            assert_eq!(list["e2e"]["status"], "disabled");
            assert_eq!(
                account_request(&session, &json!({ "operation": "add", "text": "合成上传" })),
                Err("clipboard_sync_disabled")
            );
            let devices = account_request(&session, &json!({ "operation": "devices" })).unwrap();
            assert_eq!(devices["status"], "disabled");
            assert_eq!(
                account_request(&session, &json!({ "operation": "reset" })),
                Err("clipboard_sync_disabled")
            );
            assert_eq!(
                account_request(
                    &session,
                    &json!({ "operation": "delete", "id": "synthetic-1" })
                ),
                Ok(json!({}))
            );
            assert_eq!(
                account_request(
                    &session,
                    &json!({ "operation": "set_enabled", "enabled": true })
                ),
                Ok(json!({ "enabled": true }))
            );
            assert_eq!(
                *calls.lock().unwrap(),
                [
                    "GET /v1/users/me/clipboard/keys",
                    "GET /v1/users/me/clipboard/keys",
                    "GET /v1/users/me/clipboard/keys",
                    "GET /v1/users/me/clipboard/keys",
                    "delete:synthetic-1",
                    "set_enabled:true"
                ]
            );
        }

        #[test]
        fn signed_out_and_malformed_requests_never_reach_the_backend() {
            let (session, calls) = session(None);
            assert_eq!(
                account_request(&session, &json!({ "operation": "add", "text": "合成上传" })),
                Err("account_unauthorized")
            );
            let (session, signed_in_calls) = self::session(signed_in());
            for action in [
                json!({}),
                json!({ "operation": "clear" }),
                json!({ "operation": "add" }),
                json!({ "operation": "approve" }),
                json!({ "operation": "set_enabled", "enabled": "yes" }),
            ] {
                assert_eq!(
                    account_request(&session, &action),
                    Err("invalid_cloud_clipboard")
                );
            }
            assert!(calls.lock().unwrap().is_empty());
            assert!(signed_in_calls.lock().unwrap().is_empty());
        }
    }
}
