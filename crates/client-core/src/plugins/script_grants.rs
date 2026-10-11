//! 脚本插件的授权：`<plugins root>/script-grants.json`，和 `mentions.json` 同级。只在桌面编译。
//!
//! ```json
//! {"format_version":1,"revision":3,"plugins":[{"id":"pangu-spacing","hash":"<64 位十六进制>","enabled":true,"granted":["read_commit","rewrite_commit"]}]}
//! ```
//!
//! - 放在插件目录而不放进偏好文档：偏好文档会被宿主复制、被账号同步读取，授权不该跟着走；它也不同步。`PluginPreferences` 带 `deny_unknown_fields`，往里加键会让旧版输入法读不了偏好。
//! - 数组顺序就是执行顺序。只有 `enabled` 的条目会运行，按顺序最多 `limits::MAX_ENABLED_SCRIPTS` 个。
//! - **哈希绑定**：条目的 `hash` 必须等于已安装包当前的内容哈希（`script_pack::content_hash`）才生效，包的清单或模块一变，授权全部作废。实际生效的能力是 `granted` 和清单 `permissions` 的交集。
//! - **只有一个写入方**，就是这里。写入时持有 `import::lock_plugin_root`（和导入、删包同一把锁），整份文件原子替换，每次写入 `revision` 加 1；读取方因此不加锁，看到的要么是旧文件，要么是新文件。删包、重新导入时在同一把锁里更新条目。`revision` 变化时宿主恢复被熔断的插件。
//! - **读取方容忍未知字段**（不用 `deny_unknown_fields`），给以后的版本加字段留余地；但 `format_version` 不认识、文件读不出或不是合法 JSON 时，一律按"没有任何脚本启用"处理。单个条目不合规（id 或哈希格式不对、授权违反依赖或互斥规则）只跳过这一条。
//! - `mac` 字段留给 macOS 的钥匙串 MAC（第 5 阶段）。在那之前，同一用户下的任何进程都能手写一条带正确哈希的授权，所以发布构建里 `msime_client_scripts_enable` 恒返回 false，这个文件只在开发构建里起作用。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use msime_script_runtime::{limits, Capability, CapabilitySet};
use serde::{Deserialize, Serialize};

use super::import::{lock_plugin_root, RootWrites};
use super::script_pack::{self, rule_message};
use crate::skin::catalog::safe_id;

/// 授权文件名。
pub const GRANTS_FILE: &str = "script-grants.json";
/// 本版读写的格式。
pub const FORMAT_VERSION: u64 = 1;
/// 授权文件的字节上限。
pub const MAX_DOCUMENT_BYTES: u64 = 256 * 1024;
/// 读取的条目上限，之后的忽略。
pub const MAX_ENTRIES: usize = 256;

/// 一个脚本插件的授权。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptGrant {
    pub id: String,
    /// 授权时包的内容哈希，64 位小写十六进制。
    pub hash: String,
    pub enabled: bool,
    /// 用户授予的能力。不认识的能力名在读取时丢掉。
    pub granted: CapabilitySet,
    /// macOS 钥匙串 MAC（第 5 阶段），本阶段只读不验。
    pub mac: Option<String>,
}

/// 整份授权。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptGrants {
    pub revision: u64,
    /// 按执行顺序。
    pub plugins: Vec<ScriptGrant>,
}

impl ScriptGrants {
    /// 会运行的条目：启用的、按执行顺序的前 `limits::MAX_ENABLED_SCRIPTS` 个。哈希是否和已安装的包一致由调用方核对。
    pub fn enabled(&self) -> impl Iterator<Item = &ScriptGrant> {
        self.plugins
            .iter()
            .filter(|grant| grant.enabled)
            .take(limits::MAX_ENABLED_SCRIPTS)
    }

    pub fn get(&self, id: &str) -> Option<&ScriptGrant> {
        self.plugins.iter().find(|grant| grant.id == id)
    }
}

/// 写入授权失败。
#[derive(Debug, thiserror::Error)]
pub enum ScriptGrantError {
    /// 没有装这个脚本包，或者它载入不了；文本说明原因。
    #[error("plugin_script_missing: {0}")]
    Missing(String),
    /// 包的内容哈希和调用方看到的不一致：授权页展示之后包被换过。
    #[error("plugin_script_changed")]
    Changed,
    /// 授权本身不合规；文本说明原因。
    #[error("plugin_script_grant_invalid: {0}")]
    Invalid(String),
    /// 已经启用了 `limits::MAX_ENABLED_SCRIPTS` 个脚本。
    #[error("plugin_script_limit")]
    Limit,
    /// 启用后超出进程内存总账（`limits::PROCESS_MEMORY_BYTES`）。
    #[error("plugin_script_memory")]
    Memory,
    /// 授权文件是更新的版本写的，这一版不认识，不覆盖它。
    #[error("plugin_script_grants_format")]
    Format,
    /// 插件目录是符号链接或不是目录。
    #[error("plugin_storage")]
    Storage,
    #[error("plugin_io: {0}")]
    Io(#[from] io::Error),
}

/// 授权文件的路径，给要按文件戳判断是否重读的宿主用。
pub fn document_path(root: &Path) -> PathBuf {
    root.join(GRANTS_FILE)
}

/// 当前的授权。不加锁：写入方整份原子替换。文件不存在、读不出、不是合法 JSON 或 `format_version` 不认识时，返回空授权（没有任何脚本启用）。
pub fn read(root: &Path) -> ScriptGrants {
    match load_document(root) {
        Stored::Parsed(grants) => grants,
        Stored::Missing | Stored::Unreadable | Stored::UnknownFormat => ScriptGrants::default(),
    }
}

/// 授予、调整或撤销脚本插件 `id` 的授权，返回写入后的整份授权。
///
/// `hash` 是授权页展示给用户的那个内容哈希：写入前在锁里重新读一遍包、算一遍哈希，不一致时返回 [`ScriptGrantError::Changed`]，免得用户授权的是一个版本、生效的是另一个。`granted` 必须是清单 `permissions` 的子集，并满足依赖和互斥规则。启用时检查同时启用的个数和进程内存总账。新条目排在最后执行。
pub fn set(
    root: &Path,
    id: &str,
    hash: &str,
    enabled: bool,
    granted: CapabilitySet,
) -> Result<ScriptGrants, ScriptGrantError> {
    if !safe_id(id) {
        return Err(ScriptGrantError::Invalid("插件 id 无效".into()));
    }
    crate::storage::reject_symlink(root).map_err(|_| ScriptGrantError::Storage)?;
    if !fs::symlink_metadata(root).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(ScriptGrantError::Missing("插件文件夹不存在".into()));
    }
    let writes = lock_plugin_root(root)?;
    let installed = script_pack::load(root, id).map_err(ScriptGrantError::Missing)?;
    if installed.pack.content_hash != hash {
        return Err(ScriptGrantError::Changed);
    }
    // 授权的必须是一个能运行的模块：和导入时同一份 ABI 校验，只编译，不执行。
    msime_script_runtime::validate(&installed.wasm, installed.pack.declaration()).map_err(
        |error| {
            ScriptGrantError::Invalid(format!(
                "{} 不是有效的脚本模块：{error}",
                installed.pack.module
            ))
        },
    )?;
    if let Some(extra) = granted
        .iter()
        .find(|capability| !installed.pack.permissions.contains(*capability))
    {
        return Err(ScriptGrantError::Invalid(format!(
            "清单没有申请 {}",
            extra.as_str()
        )));
    }
    granted
        .check_rules()
        .map_err(|error| ScriptGrantError::Invalid(rule_message(error)))?;
    let mut grants = match load_document(root) {
        Stored::Parsed(grants) => grants,
        // 读不出的文件本来就等于没有任何授权，整份重写。
        Stored::Missing | Stored::Unreadable => ScriptGrants::default(),
        Stored::UnknownFormat => return Err(ScriptGrantError::Format),
    };
    if enabled {
        admit(root, &grants, &installed)?;
    }
    let entry = ScriptGrant {
        id: id.to_owned(),
        hash: hash.to_owned(),
        enabled,
        granted,
        mac: None,
    };
    match grants.plugins.iter_mut().find(|grant| grant.id == id) {
        Some(existing) => *existing = entry,
        None => grants.plugins.push(entry),
    }
    grants.revision = grants.revision.saturating_add(1);
    write(root, &writes, &grants)?;
    Ok(grants)
}

/// 启用 `installed` 之前的准入检查：同时启用的个数，和进程内存总账。只算其他条目里真会运行的：启用的、包还在、哈希对得上的。
fn admit(
    root: &Path,
    grants: &ScriptGrants,
    installed: &script_pack::LoadedScript,
) -> Result<(), ScriptGrantError> {
    let others: Vec<script_pack::LoadedScript> = grants
        .enabled()
        .filter(|grant| grant.id != installed.id)
        .filter_map(|grant| {
            script_pack::load(root, &grant.id)
                .ok()
                .filter(|other| other.pack.content_hash == grant.hash)
        })
        .collect();
    if others.len() >= limits::MAX_ENABLED_SCRIPTS {
        return Err(ScriptGrantError::Limit);
    }
    let footprint =
        |script: &script_pack::LoadedScript| (script.wasm.len(), script.pack.memory_pages);
    if !limits::fits_memory_budget(
        others
            .iter()
            .map(footprint)
            .chain(std::iter::once(footprint(installed))),
    ) {
        return Err(ScriptGrantError::Memory);
    }
    Ok(())
}

/// 导入一个脚本包时、替换包目录之前调用，持有插件目录的写锁。已有这个 id 的条目时：内容哈希没变、并且是原地覆盖一个已安装的包，授权保留；否则（包换了内容，或者包目录在这之前已经不在了）停用并清空授予项，保留旧哈希，让授权页能显示"已更新，需要重新授权"。两种情况 `revision` 都加 1。文件不存在、读不出或格式不认识时不动它：那样本来就没有任何授权生效。
pub(super) fn installing(
    root: &Path,
    writes: &RootWrites,
    id: &str,
    hash: &str,
    replaces_installed: bool,
) -> io::Result<()> {
    let Stored::Parsed(mut grants) = load_document(root) else {
        return Ok(());
    };
    let Some(entry) = grants.plugins.iter_mut().find(|grant| grant.id == id) else {
        return Ok(());
    };
    if !(replaces_installed && entry.hash == hash) {
        entry.enabled = false;
        entry.granted = CapabilitySet::EMPTY;
        entry.mac = None;
    }
    grants.revision = grants.revision.saturating_add(1);
    write(root, writes, &grants)
}

/// 删除一个脚本包时、移走包目录之前调用，持有插件目录的写锁：去掉它的条目，`revision` 加 1。没有条目、文件不存在、读不出或格式不认识时不动它。
pub(super) fn removing(root: &Path, writes: &RootWrites, id: &str) -> io::Result<()> {
    let Stored::Parsed(mut grants) = load_document(root) else {
        return Ok(());
    };
    let before = grants.plugins.len();
    grants.plugins.retain(|grant| grant.id != id);
    if grants.plugins.len() == before {
        return Ok(());
    }
    grants.revision = grants.revision.saturating_add(1);
    write(root, writes, &grants)
}

/// 磁盘上的授权文件是什么状态。
enum Stored {
    Missing,
    Unreadable,
    UnknownFormat,
    Parsed(ScriptGrants),
}

#[derive(Deserialize)]
struct RawDocument {
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    plugins: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawEntry {
    id: String,
    hash: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    granted: Vec<String>,
    #[serde(default)]
    mac: Option<String>,
}

#[derive(Serialize)]
struct WireDocument<'a> {
    format_version: u64,
    revision: u64,
    plugins: Vec<WireEntry<'a>>,
}

#[derive(Serialize)]
struct WireEntry<'a> {
    id: &'a str,
    hash: &'a str,
    enabled: bool,
    granted: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mac: Option<&'a str>,
}

fn load_document(root: &Path) -> Stored {
    let path = document_path(root);
    if crate::storage::reject_symlink(&path).is_err() {
        return Stored::Unreadable;
    }
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Stored::Unreadable,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Stored::Missing,
        Err(_) => return Stored::Unreadable,
    }
    let Ok(bytes) = crate::storage::open_private_file_in(&path).and_then(|file| {
        crate::bounded_io::read_bounded_file(file, MAX_DOCUMENT_BYTES, || {
            io::Error::other("script grants are too large")
        })
    }) else {
        return Stored::Unreadable;
    };
    // 先只看 `format_version`：更新的版本可以改掉别的字段的形状，版本不认识时整份不解释。
    #[derive(Deserialize)]
    struct Version {
        format_version: serde_json::Value,
    }
    let Ok(version) = serde_json::from_slice::<Version>(&bytes) else {
        return Stored::Unreadable;
    };
    if version.format_version.as_u64() != Some(FORMAT_VERSION) {
        return Stored::UnknownFormat;
    }
    let Ok(document) = serde_json::from_slice::<RawDocument>(&bytes) else {
        return Stored::Unreadable;
    };
    let mut plugins: Vec<ScriptGrant> = Vec::new();
    for value in document.plugins.into_iter().take(MAX_ENTRIES) {
        let Some(grant) = entry(value) else {
            continue;
        };
        if plugins.iter().any(|existing| existing.id == grant.id) {
            continue;
        }
        plugins.push(grant);
    }
    Stored::Parsed(ScriptGrants {
        revision: document.revision,
        plugins,
    })
}

/// 一个条目，不合规时 `None`（跳过这一条，不影响别的）。
fn entry(value: serde_json::Value) -> Option<ScriptGrant> {
    let raw: RawEntry = serde_json::from_value(value).ok()?;
    if !safe_id(&raw.id) || !is_hash(&raw.hash) {
        return None;
    }
    let granted: CapabilitySet = raw
        .granted
        .iter()
        .filter_map(|name| Capability::parse(name))
        .collect();
    granted.check_rules().ok()?;
    Some(ScriptGrant {
        id: raw.id,
        hash: raw.hash,
        enabled: raw.enabled,
        granted,
        mac: raw.mac,
    })
}

/// 64 位小写十六进制，`script_pack::content_hash` 的形状。
fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// 整份原子替换。`_writes` 证明调用方持有插件目录的写锁。
fn write(root: &Path, _writes: &RootWrites, grants: &ScriptGrants) -> io::Result<()> {
    let document = WireDocument {
        format_version: FORMAT_VERSION,
        revision: grants.revision,
        plugins: grants
            .plugins
            .iter()
            .map(|grant| WireEntry {
                id: &grant.id,
                hash: &grant.hash,
                enabled: grant.enabled,
                granted: grant.granted.iter().map(Capability::as_str).collect(),
                mac: grant.mac.as_deref(),
            })
            .collect(),
    };
    let bytes = serde_json::to_vec_pretty(&document).map_err(io::Error::other)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(io::Error::other("script grants are too large"));
    }
    crate::storage::replace_private_file(&document_path(root), &bytes)
}
