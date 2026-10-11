#include "../../settings/CommunityPageModel.h"
#include <cstdio>
#include <string>

using namespace msime::settings::community;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
} // namespace

int main() {
  // boundedGraphemes：按字截断，组合符号、表情序列和旗帜不被拆开。
  check(bounded_graphemes(L"水杉输入法", 3) == L"水杉输", "cjk truncation");
  check(bounded_graphemes(L"abc", 5) == L"abc", "shorter text is kept");
  check(bounded_graphemes(L"éx", 1) == L"é", "combining mark stays with its base");
  check(bounded_graphemes(L"\U0001F468‍\U0001F469‍\U0001F467!", 1) ==
            L"\U0001F468‍\U0001F469‍\U0001F467",
        "zwj sequence is one grapheme");
  check(bounded_graphemes(L"\U0001F44D\U0001F3FD\U0001F44D", 1) == L"\U0001F44D\U0001F3FD",
        "skin tone modifier");
  check(bounded_graphemes(L"\U0001F1E8\U0001F1F3\U0001F1EF\U0001F1F5", 1) ==
            L"\U0001F1E8\U0001F1F3",
        "regional indicator pair is one flag");
  check(bounded_graphemes(L"\U0001F1E8\U0001F1F3\U0001F1EF\U0001F1F5!", 2) == L"\U0001F1E8\U0001F1F3\U0001F1EF\U0001F1F5",
        "two flags");
  check(bounded_graphemes(L"a\r\nb", 2) == L"a\r\n", "crlf is one grapheme");
  check(bounded_graphemes(L"❤️❤", 1) == L"❤️", "variation selector");

  // communityPublishFields：名称去掉首尾空白后 1–32 个字，说明最多 280 个字。
  {
    const auto fields = publish_fields(L"  我的皮肤 ", L" 说明\n");
    check(fields.name == L"我的皮肤" && fields.description == L"说明", "fields are trimmed");
    check(fields.name_valid && fields.description_valid, "short fields are valid");
  }
  check(!publish_fields(L"   ", L"").name_valid, "blank name");
  check(publish_fields(std::wstring(32, L'名'), L"").name_valid, "32 characters");
  check(!publish_fields(std::wstring(33, L'名'), L"").name_valid, "33 characters");
  check(publish_fields(L"x", std::wstring(280, L'字')).description_valid, "280 characters");
  check(!publish_fields(L"x", std::wstring(281, L'字')).description_valid, "281 characters");
  check(publish_fields(L"x", L"").description_valid, "empty description is fine");

  // 「其他」素材授权：非空，UTF-8 不超过 120 字节（40 个汉字）。
  check(asset_license_valid(L" 仅限个人使用 "), "custom license");
  check(!asset_license_valid(L"  "), "blank license");
  check(asset_license_valid(std::wstring(40, L'授')), "40 chinese characters");
  check(!asset_license_valid(std::wstring(41, L'授')), "41 chinese characters");
  check(utf8_length(L"aé水\U0001F98C") == 1 + 2 + 3 + 4, "utf8 length");

  // 数字、评分和大小。
  check(zh_number(0) == L"0" && zh_number(999) == L"999" && zh_number(1234) == L"1,234" &&
            zh_number(1234567) == L"1,234,567",
        "zh-CN grouping");
  check(rating(0, 0) == L"暂无评分" && rating(3, 4.25) == L"4.3 分" && rating(2, 4.75) == L"4.8 分" &&
            rating(1, 4.04) == L"4.0 分" && rating(1, 5) == L"5.0 分",
        "rating");
  check(rating_metrics(1200, L"下载", 3, 4.5) == L"1,200 人下载 · 4.5 分 · 3 人评分",
        "rating metrics");
  check(megabytes(1024 * 1024 * 1.5) == L"1.5 MB" && megabytes(0) == L"0.0 MB", "megabytes");

  // 错误码的句子：审核类的三种各宿主相同；发布时 community_invalid 另有一句。
  check(candidate_skin_message(L"community_blocked_content") ==
            L"内容包含不允许发布的词语，请修改后再提交",
        "moderation sentence");
  check(candidate_skin_message(L"community_invalid") == L"内容无效，请修改后重试。" &&
            candidate_skin_message(L"community_invalid", true).starts_with(L"服务器未接受这款皮肤"),
        "publish invalid");
  check(candidate_skin_message(L"candidate_skin_exists") == L"已存在同名皮肤，安装会整体替换它。",
        "exists");
  check(candidate_skin_message(L"whatever") == L"社区暂时不可用，请稍后重试。", "skin fallback");
  check(plugin_message(L"community_conflict", L"", true) ==
            L"发布信息已变更，或已达到发布上限（最多 20 个、合计 32 MB）。" &&
            plugin_message(L"community_conflict", L"") == L"作品状态已变化，请刷新后重试。",
        "plugin conflict");
  check(plugin_message(L"plugin_invalid", L"缺少 license") == L"插件不符合要求（缺少 license）。",
        "plugin pack failure reads as on the 插件 page");
  check(plugin_message(L"whatever", L"") == L"社区暂时不可用，请稍后重试。", "plugin fallback");
  check(needs_sign_in(L"community_unauthorized") && !needs_sign_in(L"community_forbidden"),
        "sign in");

  // 授权行、作者行、分类和本机选项。
  check(license_line(L" CC-BY-4.0 ", L"", L"https://x") ==
            L"素材授权 CC-BY-4.0 / 来源 https://x",
        "license line");
  check(license_line(L"", L"", L"").empty(), "empty license line");
  check(card_author(L"自然", L"小鹿", false, false, false) == L"自然 · 小鹿", "author");
  check(card_author(L"", L"小鹿", true, true, true) == L"我的作品 · 私有 · 已下架", "own author");
  check(card_author(L"", L"小鹿", false, false, true) == L"小鹿", "removed only for own items");
  check(category_label(L"tech") == L"科技夜色" && category_label(L"").empty() &&
            category_label(L"unknown").empty(),
        "category label");
  check(local_option_label(L"a", L"a") == L"a" && local_option_label(L"名字", L"id") == L"名字（id）",
        "local option label");
  // 多行输入框的 CR 和 CR LF 都按 textarea 写成 LF。
  check(line_feeds(L"a\rb\r\nc\n") == L"a\nb\nc\n", "line feeds");
  check(line_feeds(L"\r\r\n") == L"\n\n", "consecutive breaks");
  check(join_dot({L"自然", L"", L"小鹿", L"v1"}) == L"自然 · 小鹿 · v1", "join");

  // 社区插件空列表的句子。
  check(plugin_empty_notice(true, false, false, true).ends_with(L"查看更早发布的插件。"),
        "skipped pages");
  check(plugin_empty_notice(false, false, true, true) == L"你还没有发布过插件。", "mine empty");
  check(plugin_empty_notice(false, true, false, true) == L"没有匹配的插件。", "filtered empty");
  check(plugin_empty_notice(false, false, false, true).starts_with(L"社区里还没有插件，"),
        "publishable empty");

  if (failures)
    return 1;
  std::puts("community page model: ok");
  return 0;
}
