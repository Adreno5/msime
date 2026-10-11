# Agent Note: 外接键盘快捷键的入口从手机「设置」根页面移到「键盘」页

Status: implemented

## Problem

HarmonyOS 手机的「设置」根页面（`keyboard/root-settings-list.tsx` 的 `rootGroupLayout`）在「输入 / 表达 / 词库」这一组里列着「外接键盘快捷键」。只有接了实体键盘的人才用得到它，却和每天都会进的输入、词库排在同一级。用户要求它不要放在根页面上。

## Decision

- `rootGroupLayout` 去掉 `shortcuts`。`nestedPages` 记录「入口放在别的页里」的页面：`shortcuts` 的 `parent` 是 `screen-keyboard`（手机上叫「键盘」）。`rootSettingsGroups` 在父页面也被宿主提供时不把它放进最后一组；父页面不存在时它照旧落在最后一组，不会失去入口。
- 「键盘」页（`ScreenKeyboardSettingsPage`）在手机上于「外观」组之前加一组「外接键盘」，用 `SubPageEntries` 画一行「外接键盘快捷键」。
- 根页面搜索改用 `rootSearchRows`：根页面上的行加上 `nestedPages` 里被挪走的页面，搜「键盘」仍然能找到「外接键盘快捷键」。
- `pageEntry` 对手机上隐藏的页面（`mobileHiddenPageIds`）返回 `undefined`。`selectPage` 本来就不会去这些页面，以前 `SubPageEntries` 却会给它们画一行点了没反应的入口；宿主没有路由任何实体键盘快捷键时快捷键页正是这样被隐藏的，「外接键盘」这一组因此不出现。
- 桌面侧栏的「快捷键」和手机「全部设置」列表不变。

这推翻了 [iOS 与 HarmonyOS 全平台设计](2026-10-07-ios-harmony-all-platform-design.md) 里「鸿蒙首页另外保留……外接键盘快捷键……」中关于外接键盘快捷键的那一项；那条里候选栏、剪贴板、AI 辅助仍留在根页面。

## Alternatives considered

- 放到「输入」页底部。理由：快捷键管的是中英切换、选词和翻页，都是输入行为，「输入」页也有「候选与联想」。没采用：「输入」页讲的是方案和联想，与是不是实体键盘无关；「键盘」页讲的是用什么键盘、键盘长什么样，实体键盘的按键放在这里，用户找「键盘」时就能看到。
- 从手机上完全去掉这一页。理由：根页面更干净，鸿蒙手机接实体键盘的人很少。没采用：宿主确实读这些开关（中英切换、简繁切换、翻页键），接了键盘的用户需要能改；`index.tsx` 里隐藏它的条件是「宿主不路由任何快捷键」，而不是「平台是手机」，正是为了不让这些开关无处可改。
- 把 `shortcuts` 加进 `subPageParents`。理由：那里就是「从别的页进入的页面」的登记处，返回按钮还能写出父页面的名字。没采用：`subPageParents` 是所有宿主共用的，桌面侧栏会因此把「快捷键」从一级导航里拿掉、选中时高亮「屏幕键盘」；这次只想改手机根页面。

## Consequences

- 手机根页面少一行，外接键盘快捷键要从「键盘」页进入，多一步；搜索仍可直达。
- 从「键盘」页进入快捷键页后，返回回到「键盘」页（WebView 历史）。若设置是在快捷键页上打开的（启动时恢复到该页），返回按钮向上去的是「设置」标签根页面，因为 `subPageParents` 里没有它。
- 以后要把别的页面从根页面挪进某一页，在 `nestedPages` 登记并在父页面里加入口，搜索会自动带上它。

## Verification

- `apps/desktop/tests/core/home-page.test.tsx` 改为根页面不再有「外接键盘快捷键」一行，搜索「键盘」仍列出它；新增 `tests/settings/keyboard-page-harmony.test.tsx`：鸿蒙手机「键盘」页的「外接键盘」组打开快捷键页，宿主不路由任何快捷键时没有这一组。全部测试通过。
- HarmonyOS 模拟器（6.0.0.130）上看过根页面和「键盘」页，从「键盘」页进入快捷键页再返回。没有在真机上验证。
