# Windows WinUI 3 设置

`MSIME.Settings.vcxproj` 是 Windows 设置窗口的原生宿主（安装后为 `msime-client-settings.exe`）。它使用 Windows App SDK 的 WinUI 3 控件，按 Windows 11 设置的样式绘制：标题栏搜索框、280 宽的侧栏（带页面筛选）、每行一张 Fluent 卡片。它不启动 Tauri 作为设置界面，也不拥有 TSF 或输入会话。

## 页面

侧栏的 18 个页面分成 6 组，依次是打字（输入、标点与翻译、快捷键、词库）、外观（主题、候选窗口、悬浮工具栏）、更多输入方式（屏幕键盘、语音输入、手写输入）、工具（剪贴板、打字统计、插件、AI 辅助）、账号（账号与同步）、支持（维护与诊断、帮助与反馈、关于），每组以组名开头。分组与顺序和共享设置界面的 `settingsNavGroups`（`packages/ui/src/settings/settings-page-registry.ts`）一致，定义在 `SettingsNavigation.h`，由 `tests/ui/settings_navigation.cpp` 核对，并对照共享路由词表检查；组名和页名在 `main.cpp` 中。

- 输入、标点与翻译、快捷键、词库、主题、候选窗口、悬浮工具栏、屏幕键盘、语音输入、手写输入、剪贴板、维护与诊断、帮助与反馈、关于：在本窗口中原生绘制。
- 打字统计、插件、AI 辅助、账号与同步：跨平台服务页，点击「打开」后用 `MSIME.exe --route=…` 在共享 Tauri 应用中显示对应页面。
- 桌面端没有社区页：社区皮肤在共享应用「主题」页的「社区皮肤」标签里，本窗口「主题」页的「社区皮肤」一行打开它，`community` 路由打开本窗口的「主题」页。AI 对话是 AI 辅助的子页，`chat` 路由打开「AI 辅助」。
- 帮助不单独成页：「帮助与反馈」页在反馈和诊断日志之后原生绘制共享「帮助」页（`packages/ui/src/settings/help-settings-page.tsx`）在 Windows 上的内容，文案取 `settings/platform-copy.ts` 的 Windows 分支，「完整文档」用默认浏览器打开 https://msime.app/docs/。
- 反馈不跳共享应用：「帮助与反馈」页照共享页（`packages/ui/src/settings/pages/feedback-page.tsx`）画「提交可复现的问题」和「反馈与交流」。选反馈类型、写描述（最多 4000 字），「复制报告」把「### 类型 / ### 描述 / ### 环境」格式的报告放进剪贴板，「在 GitHub 提交」用默认浏览器打开 `https://github.com/metasequoiaime/msime/issues/new?title=…&body=…`，正文截到 4000 个字符；附上的信息是版本号、系统版本（`RtlGetVersion`）和当前输入方案。渠道是 GitHub Issues、QQ 交流群（复制群号 829919142）和 Telegram 群组（https://t.me/msimegroup）。报告格式和地址编码在 `FeedbackReport.h`，由 `tests/ui/feedback_report.cpp` 核对。
- 屏幕键盘设置不跳共享应用：「屏幕键盘」页照共享页（`packages/ui/src/settings/pages/screen-keyboard-page.tsx` 和 `touch-keyboard-geometry-section.tsx`）在 Windows 上的内容画四组。「屏幕键盘」组是打开面板的一行（面板本身仍由 `MSIME.exe` 显示）和键盘预览：预览照 `ScreenKeyboardPreview` 的桌面布局画在 1100 单位宽的画布上、最宽缩到 760，配色取 `msime_client_theme_catalog` 的 `keyboard` 调色板（跟随系统和没有调色板的主题用共享预览的系统键盘色），明暗跟「屏幕键盘主题」，自定义主题带键盘设计时按设计画键形、材质、描边、渐变、底纹和照片；在预览上左右拖动调键距、上下拖动调行距，规则与共享页的 `useTouchKeyboardGeometryDrag` 相同。「尺寸」组是键盘高度（`touch_keyboard_height_adjustment`，-12 到 48）、按键间距（`touch_key_spacing_tenths`，30 到 60）、行间距（`touch_row_spacing_tenths`，40 到 100）和「恢复默认」，后者确认后从文档里删掉这三项和 `touch_voice_shortcut`、`touch_toolbar`。「工具栏」组只有「顶部语音入口」（Windows 不是 `touch_toolbar_components` 的宿主）；「外观」组的「前往」落到「主题」页「界面主题」组的「屏幕键盘主题」一行。手机才有的「布局」「手势」和触屏反馈不画。
- 在线翻译服务不跳共享应用：「标点与翻译」页「候选词翻译」组的「翻译服务」照共享页（`translation-service-selector-section.tsx`、`use-translation-settings.ts`）列出关闭、腾讯云机器翻译、小牛翻译（NiuTrans）、自定义 DeepLX 兼容服务和水杉账号，候选词翻译关着时不可选；选一种只打开它、关掉另外两种，选水杉账号时写 `translation_account`，否则去掉这一项。选腾讯云、小牛翻译或自定义服务时下面多一组它的设置（SecretId、SecretKey、地域；App ID、API Key；翻译 Endpoint、API Key），密钥框带「显示」「隐藏」按钮，文案、提示和警告与共享页相同。输入时就地更新提示，不重建页面；偏好库接受的值才写入（停下后保存），不接受的值输入框标红、不写入。每组的「测试配置」在工作线程上调用 `msime_client_test_api_credential`（与 Tauri 的 `test_api_credential` 同一个 `credential::test_service`），结果只在配置没改时显示。自定义地址的提示由 `msime_client_translation_endpoint_issue` 按 WHATWG 网址规则给出；其余规则在 `TranslationServiceSettings.h`，由 `tests/ui/translation_service_settings.cpp` 核对。
- 剪贴板不跳共享应用：「剪贴板」页（`tools` 路由）照共享页（`packages/ui/src/settings/pages/tools-page.tsx` 和 `clipboard-history-section.tsx`）在 Windows 上的内容画三组。「剪贴板」组是「剪贴板历史」开关（`clipboard_history`，从开到关时再清空已存的历史）、「从系统剪贴板同步」和有历史时的「清空历史」，后者点两次才清空（第二次按钮是「确认清空」）。历史开着时多一组「历史记录」：每条一行，文字单行显示、悬停看全文，下面是「已固定 · 」和本地时间（没有时间的旧记录写「旧记录」），按钮是「重新复制」「发到云剪贴板」「固定 / 取消固定」「删除」。历史文件与共享应用相同（偏好目录下的 `clipboard_history.json`），读、同步、清空、固定和删除都在工作线程上经 `msime_client_desktop_clipboard_history`；同步读系统剪贴板用与 Tauri 外壳相同的 `clipboard_history::windows_clipboard_text`，带密码管理器隐私标记的内容不记录。「重新复制」用 WinRT 剪贴板放回系统剪贴板，再像共享应用的 `copy_text` 一样经 `msime_client_capture_clipboard_history` 记进历史。「发到云剪贴板」经 `msime_client_account_cloud_clipboard` 使用共享应用登录时写在 `%LOCALAPPDATA%\<本版本的用户目录>\account` 的会话：每次进入页面查一次账号和服务端开关，未登录、服务端关着或服务不可用时按钮不可用并在组里说明，超过 4000 个 UTF-16 单元的记录不发送。「云剪贴板」组只有打开云剪贴板面板的一行，面板仍由 `MSIME.exe` 显示。
- 其他平台下载不单独成页：「关于」页的「版本与更新」组可以打开产品下载页或复制链接。

原生页面不绘制的部分（皮肤与自定义主题编辑、云词库和云剪贴板面板、语音识别服务）以按钮的形式跳到共享应用的对应路由，路由 id 与托盘和其他启动方使用的相同。

「主题」页「外观」组的候选字体各行照搬共享设置「候选窗口」页「字体与大小」组在 Windows 上的样子（`CandidateFontPresetRow` 与 `CandidateFontControls`），不跳共享应用：「候选字体」预设（默认、宋体、黑体、楷体、圆体，Windows 没有自带圆体，该项禁用）同时写 `candidate_font_family` 和有序的 `candidate_fallback_fonts`；「主字体」在 Windows 上同时排到回退列表最前；「英文字体」写 `candidate_english_font`，未设置时显示 Segoe UI；「系统字体列表」一行显示读取状态并可「刷新字体列表」。预设、主字体和字体名的规则在 `CandidateFontPresets.h`，由 `tests/ui/candidate_font_presets.cpp` 核对。字体列表由 `msime_client_list_font_families` 读取（与 Tauri 的 `list_font_families` 同一份 `system_fonts` 目录），第一次点开字体输入框时在工作线程上读，读取状态在原处更新，不重建页面，输入框不会丢掉焦点。下拉按用户区域设置不分大小写地筛选（`LCMapStringEx`）。名字不合法（空、超过 128 个 UTF-8 字节或含控制字符）时不写入，输入框标红，框下显示「名称不能为空、不能含控制字符或超过 128 个 UTF-8 字节。」，读屏从输入框的帮助文本读到同一句。

「输入」页的辅助码方案下拉框和共享设置的 `HelpcodeSettingsGroup`（`packages/ui/src/settings/pages/helpcode-page.tsx`）规则相同，辅助码插件也在这里选，不跳共享应用。选项依次是内置方案、资源目录 `helpcodes/custom` 里的自定义码表（`custom/<文件名>`，由 `msime_client_helpcode_schemas` 读宿主选项的 `resources`，名字是「中文名 (英文名)」）和已安装的辅助码表插件（「插件名（插件）」，取自 `msime_client_plugins` 的 `catalog` 中种类为 `helpcode` 的包）。选插件只写 `plugins.helpcode_pack_quanpin` / `helpcode_pack_shuangpin`，输入引擎用它替换内置方案；选内置或自定义方案时同时清空插件。已选的插件不在了显示为「id（插件，未找到）」，资源目录读不到时已选的自定义码表也留在列表里。自定义码表只读一次，插件每次进入「输入」页重读，都在工作线程上读。本版本提供全拼或双拼之一就显示这一组，两族都列出（共享页的 `editionUsesHelpcode`）；「在候选窗口中显示…辅助码」不随开关变灰；Windows 不是 Shift 输入辅助码的宿主（`helpcode_shift_entry` 为 false），所以没有共享页那段 Shift 说明。

`--route=settings:<id>` 指定打开的页面，可以是共享路由的设置分类 id（`appearance`、`dictionary`、`about` 等，映射见 `route_aliases`），也可以是本窗口的页面 id；其他值打开默认的「输入」页。

## 外观与主题预览

窗口本身按设计稿的 Fluent token 绘制，强调色固定为浅色 `#005FB8`、深色 `#60CDFF`（与候选窗口原生调色板相同），不跟随 Windows 的个性化强调色。「设置界面主题」只覆盖本窗口的明暗。

「主题」「候选窗口」「输入」「标点与翻译」页的候选窗口预览和「自定义」主题卡片不自己配色：它们用与 Server 相同的请求（全局主题、去掉键盘设计的 `custom_theme`、候选窗口自己的明暗与排列方式、数据目录下的 `skins`）调用 `msime_client_resolve_theme`，再用 `src/candidate/CandidatePalette.h` 合成调色板，并按 `CandidateWindow.cpp` 的规则绘制选中行、序号、选中条和翻译，所以自定义主题的 `candidate_colors` 与皮肤包会反映在预览里。其余主题卡片使用 `msime_client_theme_catalog` 给出的预览色。

## 默认输入法提示

每页标题上方的橙色提示条对应设计稿的 deskStrip：水杉输入法不在当前用户的键盘列表中时显示「去添加」，已添加但不是默认输入法时显示「设为默认」。状态通过 `input.dll` 的 `EnumEnabledLayoutOrTip` 读取（无头文件和导入库，运行时加载），按钮分别调用 `InstallLayoutOrTip` 和 `SetDefaultLayoutOrTip`；添加失败时打开「语言和区域」。`input.dll` 无法回答时不显示提示条。窗口重新获得焦点时重新检查。

## 保存

设置窗口通过 `msime_client_load_preferences` 和 `msime_client_save_preferences` 读取、校验并以 compare-and-swap 方式保存共享偏好。每个控件改动后立即保存，滑块停止拖动后保存；保存被拒绝（其他窗口已更新或与其他设置冲突）时重新读取并在页面顶部说明。窗口重新获得焦点时重新读取，以反映托盘、共享应用或同步带来的变化。数据目录必须与 Server 的状态根相同：Server 启动它时注入 `MSIME_CLIENT_STATE_DIR`，直接用这个目录；从开始菜单直接启动时没有这个变量，就按 Server 的方式自己解析，先用 `common/StateDirectory.h` 的 `resolve_state_directory()`（本版本的数据目录环境变量、安装器在 HKLM 记下的 `DataDir`、`%LOCALAPPDATA%\<本版本的状态目录>`），那里的 `runtime-options.json` 写了绝对路径的 `preferences_directory` 时再改用它，与 `server_main.cpp` 的 `production_preview_document` 和 Tauri 外壳的 `windows_server_preferences_directory` 一致。以前开始菜单启动直接回落到 `%LOCALAPPDATA%\MSIME-Client`，而默认安装的 Server 用的是安装器的 `DataDir`（`%LOCALAPPDATA%\metasequoiaime-full`），这个窗口里改的开关（包括 `/`、`@` 和 K 模式、辅助码）就都写进了 Server 不读的文件。顺序在 `SettingsStateDirectory.h`，由 `tests/ui/settings_state_directory.cpp` 核对。

控件按偏好的点分路径读写文档，路径可以任意深（`plugins.key_sound.enabled`、`custom_theme.candidate_colors.surface`）。写入时缺的中间层从随包默认值里同一路径的对象复制而来，默认值里也没有才是空对象，因为存储只整体接受一个嵌套对象；删除一项时不补中间层，读取随之回落到默认值。`ai_assistant.tokens` 这类以来源为键的表，表项的键里可能有点，所以单独给出、原样作为最后一段。列表有两种写法：顺序没有意义的列表排序并去重后写入，顺序由用户决定的列表原样写入。路径规则在 `PreferencePath.h`，与 JSON 实现无关，由 `tests/ui/preference_path.cpp` 用一棵假的树核对。

「候选窗口」页「游戏」组的两张程序列表（`game_compatibility.overlay_processes`、`excluded_processes`）在写入前先按偏好库的规则规范化和校验（`GameProcessList.h`）：去掉首尾空白、ASCII 字母转小写，要求以 `.exe` 结尾、不超过 64 个字符、不带路径和 `\ / : * ? " < > |` 及控制字符，两张表之间不重复、合计不超过 32 条。不合法时在列表下方说明具体原因，不写入文档：保存被拒时窗口只能给出上面那句笼统的说明，用户看不出是哪一项出了问题。这组设置只由 TSF DLL 在每次激活时读取，对已打开的游戏要切换一次输入法才生效，取舍见 [决策笔记](../../../.agents/notes/implemented/feature/2026-10-08-windows-game-candidate-overlay.md)。

## 设置文件、数据目录与许可声明

「维护与诊断」页的「设置文件」组导出和导入设置。「导出…」用系统的保存对话框选位置，`msime_client_export_settings` 读出已保存的偏好并换算成设置文件（`app.msime.client.preferences`，规则在 `crates/client-core/src/settings_document.rs`），再先写临时文件后改名写到所选位置。「导入…」用打开对话框选文件，读入（上限 1 MiB）后交给 `msime_client_import_settings`：它保留本机的语音、AI 辅助和翻译服务配置与密钥、诊断日志、使用统计和剪贴板历史开关（所以也不会清空已存的剪贴板历史），文件里的方案本版本不提供时保留本机的方案，然后按本窗口读到的修订号比较并交换写回。导入成功或被拒都会重新读取设置；错误码换成说明的规则在 `SettingsDocumentFile.h`，由 `tests/ui/settings_document_file.cpp` 核对。

同一页的「数据目录」行显示状态根并可在资源管理器中打开。移到其他磁盘要重新运行完整安装包：数据目录登记在 HKLM 的 `DataDir`，改它要管理员权限，安装器的「选择数据位置」一步已经负责复制、切换和删除旧目录，本窗口只说明这条路径。

「关于」页的产品名按版本取（`MSIME_EDITION_DISPLAY_NAME`），「许可与隐私」组的「第三方组件许可」用关联程序打开安装目录下的 `THIRD_PARTY_NOTICES.txt`（本窗口所在 `server` 目录的上一级，与安装器的 `DestDir` 一致）。取舍见 [决策笔记](../../../.agents/notes/implemented/feature/2026-10-10-windows-settings-file-and-local-data-parity.md)。

## 检查更新

「关于」页的「检查更新」在本窗口中完成，不再打开共享应用：工作线程调用 `msime_client_update_check`（`crates/client-core/src/update_check.rs`，与共享设置页、鸿蒙走同一份 Rust 逻辑），请求带 `platform: windows`、本版本 id 和当前版本，结果写在该行下方：「已是最新版本」「暂无可用发行版」「检查失败，请稍后重试」，或「发现新版本 vX」加未签名提示与安装包的 SHA256，按钮随之变成「前往下载」，打开该版本在 GitHub 上的发布页。只接受本仓库的 tag 页地址。当前版本是编译期宏 `MSIME_WINDOWS_VERSION`：`Build-Client.ps1` 用 `/p:MsimeVersion=<TargetVersion>` 传入，开发构建取 `platforms/windows/version.txt`，与 Server 上报的版本相同。

## 连接 AI 助手

「维护与诊断」页的「连接 AI 助手」即共享设置页的同名区块：通过 `msime_client_mcp_status` 和 `msime_client_mcp_install` 显示与 `msime-client-settings.exe` 同目录的 `msime-mcp.exe`、可复制的 MCP 配置，并把它写入 Claude Desktop 或 Cursor 的配置文件。服务器指向的运行时选项取自 Server 注入的 `MSIME_CLIENT_HOST_OPTIONS`，直接启动时取 Server 状态目录（`resolve_state_directory()`，不是 `preferences_directory`）下的 `runtime-options.json`，也就是 Server 启动时读的那一份；两者都没有时说明输入法尚未初始化，不提供配置。写入逻辑与 Tauri 外壳共用 host-api 的 `mcp_clients`。

## 与共享应用的分工

Windows 的表情、手写和屏幕键盘面板以及上面列出的跨平台页面由同目录的共享 Tauri 应用 `MSIME.exe` 提供。本窗口通过 `src/system/ShellLauncher.cpp`（与托盘相同的启动器）启动它，数据目录和运行时选项都已知时一并传入，保证两边读写同一份设置。Server 将设置路由发送给 `msime-client-settings.exe`，将面板路由发送给 `MSIME.exe`。

## 构建

在 Visual Studio 开发者命令行中构建：

```powershell
msbuild MSIME.Settings.vcxproj /p:Configuration=RelWithDebInfo /p:Platform=x64 `
  /p:HostApiLibrary=C:\path\to\msime_host_api.dll.lib
```

Windows App SDK 版本由 `WindowsAppSDKVersion` 属性控制，默认值与项目文件固定，发布构建使用 self-contained unpackaged 模式。项目没有 XAML 编译器：应用自己实现 `IXamlMetadataProvider` 并转交给 WinUI 控件库的元数据提供者，界面全部在 `main.cpp` 中用代码构建。
