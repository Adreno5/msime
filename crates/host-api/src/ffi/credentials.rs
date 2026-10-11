//! The settings pages' credential checks: the 「测试配置」 buttons and the translation endpoint hint.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;

/// The largest `msime_client_test_api_credential` request: a service name and a configuration of a few credentials and an endpoint.
const CREDENTIAL_TEST_REQUEST_LIMIT: usize = 64 * 1024;

/// One 「测试配置」 check, `{service, config}` as the shared settings page's `testApiCredential` sends it, through `credential::test_service`, the same function the Tauri settings shell calls. Value `{ok, message}`; the message is the Chinese line the page shows, and never carries the provider's reply or the credential. Blocks on the network for up to about 15 seconds: call off the UI thread.
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_test_api_credential(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let document: Value = unsafe {
            with_bounded_bytes(
                request,
                length,
                CREDENTIAL_TEST_REQUEST_LIMIT,
                "invalid credential test request",
                |bytes| {
                    serde_json::from_slice(bytes)
                        .map_err(|_| "invalid credential test request".to_owned())
                },
            )
        }?;
        let service = document
            .get("service")
            .and_then(Value::as_str)
            .ok_or("invalid credential test request")?;
        let config = document
            .get("config")
            .filter(|config| config.is_object())
            .ok_or("invalid credential test request")?;
        let result = msime_client_core::credential::test_service(service, config);
        Ok(json!({"ok": result.ok, "message": result.message}))
    })
}

/// The hint the settings page shows under the custom translation endpoint, `translation::endpoint_issue` (empty when there is none), and whether the preferences store accepts the endpoint (`accepted`: empty, or `translation::is_supported_endpoint`). Value `{issue, accepted}`. No I/O.
/// # Safety
/// `endpoint` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_translation_endpoint_issue(
    endpoint: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let endpoint = unsafe {
            with_bounded_bytes(endpoint, length, 64 * 1024, "invalid endpoint", |bytes| {
                std::str::from_utf8(bytes)
                    .map(str::to_owned)
                    .map_err(|_| "invalid endpoint".to_owned())
            })
        }?;
        Ok(json!({
            "issue": msime_client_core::translation::endpoint_issue(&endpoint),
            "accepted": endpoint.is_empty()
                || msime_client_core::translation::is_supported_endpoint(&endpoint),
        }))
    })
}
