// Server 的会话接入生命周期核心以后，宿主库调用的先后顺序。测试自己实现 msime_client.h 里 Server 用到的函数，记下每一次调用，在 FocusedSession 的真实按键路径上逐条核对：会话一建好就选择加入 FEEDBACK 和 HOST_HOLDS_PIECES；字段属性变了先报，再报按键，然后才把键交给引擎；交给应用的键带 PASSTHROUGH、不派发；TIP 自己写进文档的上屏前后打开 COMMIT_OBSERVE_ONLY；以词定字的回退用命令 19；旧的 key_sound、commit_sound、music_set_active 和 typing_effect 一次都不再调用，同一次上屏不会在宿主这边再响一遍。
#include "FocusedSession.h"
#include "LifecyclePolicy.h"
#include "msime_client.h"

#include <cstdlib>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

using namespace msime::windows;
namespace {
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("lifecycle call order failed at line " + std::to_string(line));
}
#define REQUIRE(...) require_at((__VA_ARGS__), __LINE__)

constexpr uint64_t fake_session = 9;
constexpr uint64_t client = 42;
constexpr uint64_t token = 77;

struct Call {
  std::string name;
  uint64_t value;
};
std::vector<Call> calls;

// 假会话的状态：正在组的字母、方案、整句改字的文本和生成号。
std::string fake_editing;
uint32_t fake_scheme = 0;
std::string fake_conversion;
uint64_t fake_generation = 1;
uint32_t fake_lifecycle = 0;

void record(const char *name, uint64_t value = 0) { calls.push_back({name, value}); }

char *answer(const nlohmann::json &value) {
  const auto text = nlohmann::json{{"ok", true}, {"value", value}}.dump();
  auto *raw = static_cast<char *>(std::malloc(text.size() + 1));
  std::memcpy(raw, text.c_str(), text.size() + 1);
  return raw;
}

nlohmann::json view() {
  auto candidates = nlohmann::json::array();
  if (!fake_editing.empty())
    candidates.push_back({{"id", {{"session", fake_session}, {"generation", fake_generation}, {"index", 0}}},
                          {"text", "啊"},
                          {"annotation", ""},
                          {"highlighted", true},
                          {"source", 0},
                          {"fixed_position", false},
                          {"translation", ""}});
  return {{"session", fake_session},  {"generation", fake_generation}, {"focused", true},
          {"editing_text", fake_editing},  {"preedit", fake_editing},       {"caret_position", fake_editing.size()},
          {"candidates", candidates}, {"local_mode", "none"},     {"scheme", fake_scheme},
          {"spelling_symbols", ""},   {"dedicated_english", false}, {"conversion", fake_conversion},
          {"page", 0}};
}

// 一次派发的回复。像核心一样：会话开着 FEEDBACK 时每个派发回复都带 feedback，有上屏时特效不为 0。
char *transition(const nlohmann::json &commit) {
  ++fake_generation;
  nlohmann::json value{{"handled", true}, {"commit", commit}, {"diagnostic", nullptr}, {"view", view()}};
  value["commit_context"] = commit.is_null() ? nlohmann::json(nullptr)
                                             : nlohmann::json{{"scheme", fake_scheme},
                                                              {"local_mode", "none"},
                                                              {"typing_statistics", true}};
  if (fake_lifecycle & MSIME_LIFECYCLE_FEEDBACK)
    value["feedback"] = {{"typing_effect", commit.is_null() ? 0u : (1u << 17) | 3u}};
  return answer(value);
}

// 只留和生命周期有关的调用；连续两次相同的字段属性（全屏前台判断没变）只算一次。全屏位取决于跑测试时的前台窗口，比较时去掉。
std::vector<std::string> trace() {
  std::vector<std::string> result;
  uint64_t traits = 0;
  for (const auto &call : calls) {
    if (call.name == "set_field_traits") {
      const auto masked = call.value & ~uint64_t{MSIME_TRAIT_FULLSCREEN};
      if (masked == traits)
        continue;
      traits = masked;
      result.push_back("set_field_traits:" + std::to_string(masked));
      continue;
    }
    for (const char *kept : {"create", "set_lifecycle", "key", "set_active", "focus", "character", "command", "select",
                             "select_edge", "destroy", "key_sound", "commit_sound", "music_set_active", "typing_effect"})
      if (call.name == kept)
        result.push_back(call.name + ":" + std::to_string(call.value));
  }
  return result;
}

void expect_trace(const std::vector<std::string> &expected, int line) {
  const auto actual = trace();
  if (actual != expected) {
    std::cerr << "expected:";
    for (const auto &entry : expected)
      std::cerr << ' ' << entry;
    std::cerr << "\nactual:  ";
    for (const auto &entry : actual)
      std::cerr << ' ' << entry;
    std::cerr << '\n';
  }
  require_at(actual == expected, line);
  calls.clear();
}
#define EXPECT_TRACE(...) expect_trace(std::vector<std::string>__VA_ARGS__, __LINE__)

FanyImeNamedpipeData key(uint32_t keycode, char16_t text, uint64_t request) {
  FanyImeNamedpipeData packet{};
  packet.event_type = FanyImePipeEventType::KeyEvent;
  packet.client_id = client;
  packet.request_id = request;
  packet.keycode = keycode;
  packet.wch = static_cast<FanyImeWireChar>(text);
  return packet;
}
} // namespace

extern "C" {
uint32_t msime_client_abi_version(void) { return 3; }
char *msime_client_create(const uint8_t *, size_t) {
  record("create");
  return answer({{"session", fake_session}});
}
char *msime_client_destroy(uint64_t) {
  record("destroy");
  return answer(nullptr);
}
void msime_client_string_free(char *value) { std::free(value); }
char *msime_client_view(uint64_t) { return answer(view()); }
char *msime_client_focus(uint64_t, bool focused) {
  record("focus", focused);
  if (!focused)
    fake_editing.clear();
  return answer({{"handled", true}, {"commit", nullptr}, {"diagnostic", nullptr}, {"view", view()}});
}
char *msime_client_character(uint64_t, uint8_t ascii, bool) {
  record("character", ascii);
  // 韩文这样的方案里，开始新音节的字母带着上一个音节上屏（TIP 已经从自己的会话把它写进了文档）。
  nlohmann::json commit = nullptr;
  if (fake_scheme == 4 && !fake_editing.empty()) {
    commit = fake_editing;
    fake_editing.clear();
  }
  fake_editing.push_back(static_cast<char>(ascii));
  return transition(commit);
}
char *msime_client_command(uint64_t, uint32_t command) {
  record("command", command);
  nlohmann::json commit = nullptr;
  if (command == MSIME_COMMIT_CANDIDATE && !fake_editing.empty()) {
    commit = "啊";
    fake_editing.clear();
  } else if (command == MSIME_CANCEL || command == MSIME_CANCEL_AFTER_HOST_COMMIT) {
    fake_editing.clear();
    fake_conversion.clear();
  }
  return transition(commit);
}
char *msime_client_select(uint64_t, uint64_t, size_t index) {
  record("select", index);
  fake_editing.clear();
  return transition("啊");
}
char *msime_client_select_edge(uint64_t, uint64_t, size_t index, uint8_t) {
  record("select_edge", index);
  // 选不出首字：Server 退回到自己取高亮候选的首字、宿主写入、命令 19 丢弃组字。
  nlohmann::json value{{"handled", false}, {"commit", nullptr}, {"diagnostic", nullptr}, {"view", view()}};
  return answer(value);
}
uint32_t msime_client_set_lifecycle(uint64_t, uint32_t flags) {
  record("set_lifecycle", flags);
  fake_lifecycle = flags;
  return flags;
}
uint32_t msime_client_key(uint64_t, uint32_t key) {
  record("key", key);
  return (1u << 17) | 1u;
}
uint32_t msime_client_set_field_traits(uint64_t, uint32_t traits) {
  record("set_field_traits", traits);
  return traits;
}
bool msime_client_set_active(uint64_t, bool active) {
  record("set_active", active);
  return true;
}
// 旧的四个调用：FEEDBACK 下它们在核心里是空操作，Server 也不该再调。
bool msime_client_key_sound(uint64_t, uint32_t key_class) {
  record("key_sound", key_class);
  return false;
}
bool msime_client_commit_sound(uint64_t) {
  record("commit_sound");
  return false;
}
bool msime_client_music_set_active(uint64_t, bool active) {
  record("music_set_active", active);
  return false;
}
uint32_t msime_client_typing_effect(uint64_t, uint32_t event) {
  record("typing_effect", event);
  return 0;
}
char *msime_client_typing_effect_settings(uint64_t) { return answer(nlohmann::json::object()); }
char *msime_client_online_query(uint64_t) { return answer(nullptr); }
char *msime_client_translation_query(uint64_t) { return answer(nullptr); }
// 以下这些在这里走到的路径上用不到，只为链接。
char *msime_client_ai_request_for_query(uint64_t, const uint8_t *, size_t) { return answer(nullptr); }
char *msime_client_apply_cloud_response(uint64_t, const uint8_t *, size_t, const uint8_t *, size_t) {
  return answer(nullptr);
}
char *msime_client_apply_online_candidates(uint64_t, const uint8_t *, size_t, const uint8_t *, size_t, uint8_t) {
  return answer(nullptr);
}
char *msime_client_resolve_online_candidates(uint64_t, const uint8_t *, size_t, uint8_t) { return answer(nullptr); }
char *msime_client_clear_online_candidates(uint64_t, uint8_t) { return answer(nullptr); }
char *msime_client_apply_translations(uint64_t, uint64_t, const uint8_t *, size_t) { return answer(nullptr); }
char *msime_client_rerank_settled(uint64_t) { return answer(nullptr); }
char *msime_client_reset_cache(uint64_t) { return answer(nullptr); }
char *msime_client_update_preferences(uint64_t, const uint8_t *, size_t) {
  return answer({{"handled", true}, {"commit", nullptr}, {"diagnostic", nullptr}, {"view", view()}});
}
char *msime_client_set_chinese_punctuation(uint64_t, bool) { return answer(nullptr); }
char *msime_client_set_english_mode(uint64_t, bool) { return answer(view()); }
char *msime_client_balance_paired_punctuation_after_auto_close(uint64_t, uint8_t) { return answer(nullptr); }
char *msime_client_punctuation(uint64_t, uint8_t) { return transition(nullptr); }
char *msime_client_punctuation_ascii(uint64_t, uint8_t) { return transition(nullptr); }
char *msime_client_pin_candidate(uint64_t, uint64_t, size_t) { return transition(nullptr); }
char *msime_client_remove_candidate(uint64_t, uint64_t, size_t) { return transition(nullptr); }
char *msime_client_fix_candidate_position(uint64_t, uint64_t, size_t, uint8_t) { return transition(nullptr); }
char *msime_client_clear_candidate_position(uint64_t, uint64_t, size_t) { return transition(nullptr); }
char *msime_client_simplified_to_traditional(const uint8_t *, size_t) { return nullptr; }
char *msime_client_typing_statistics(const uint8_t *, size_t) { return answer(nullptr); }
int32_t msime_client_typing_statistics_enabled(const uint8_t *, size_t) { return 0; }
}

int main() {
  try {
    std::vector<std::string> recorded;
    {
      FocusGate gate;
      FocusedSession focused(gate, client, R"({"preferences":{}})");
      focused.set_typing_statistics_sink(
          [&](const std::string &text, TypingSource) { recorded.push_back(text); });
      // 会话一建好就选择加入，不等第一次激活。
      EXPECT_TRACE({"create:0", "set_lifecycle:" + std::to_string(lifecycle_flags())});

      // 激活：先报字段属性（旧 TIP 什么都没报，属性未知、没有变化，不调用），再成为背景音乐的所有者。
      const PipeTicket ticket{client, {1, 2, 3}};
      const auto change = gate.begin(ticket, token);
      REQUIRE(change.has_value());
      const auto lease = change->pending;
      REQUIRE(focused.prepare(lease));
      REQUIRE(gate.acknowledge(lease, [] { return true; }));
      EXPECT_TRACE({"focus:1", "set_active:1"});

      // 旧 TIP 的一个字母：按键在引擎处理之前报给核心，属性仍然未知（不带 KNOWN）。
      uint64_t request = 2;
      REQUIRE(focused.configured_key(lease, key('A', u'a', request), TsfPreeditStyle::Pinyin, {}).has_value());
      REQUIRE(focused.confirm(lease, request++));
      EXPECT_TRACE({"key:0", "character:97"});

      // 新 TIP 报来密码框，又报回普通字段：每次都立刻交给核心，带 KNOWN；核心据此关掉或放回背景音乐。
      focused.set_field_traits(FanyImeFieldTraits::Secure);
      focused.set_field_traits(FanyImeFieldTraits::Secure);
      focused.set_field_traits(0);
      EXPECT_TRACE({"set_field_traits:" + std::to_string(MSIME_TRAIT_SECURE | MSIME_TRAIT_KNOWN),
                    "set_field_traits:" + std::to_string(MSIME_TRAIT_KNOWN)});

      // 空格上屏：按键音先于引擎，上屏音由核心在派发里出，确认送达时只记统计，宿主不再调 commit_sound。
      REQUIRE(focused.configured_key(lease, key(0x20, u' ', request), TsfPreeditStyle::Pinyin, {}).has_value());
      REQUIRE(focused.confirm(lease, request++));
      EXPECT_TRACE({"key:1", "command:" + std::to_string(MSIME_COMMIT_CANDIDATE)});
      REQUIRE(recorded == std::vector<std::string>{"啊"});

      // 按住不放的自动重复带 MSIME_KEY_REPEAT。
      auto repeated = key('B', u'b', request);
      repeated.modifiers_down |= PipeMetadata::AutoRepeat;
      REQUIRE(focused.configured_key(lease, repeated, TsfPreeditStyle::Pinyin, {}).has_value());
      REQUIRE(focused.confirm(lease, request++));
      EXPECT_TRACE({"key:" + std::to_string(lifecycle_key(0, true)), "character:98"});

      // 以词定字选不出首字时，Server 自己取字写进文档、用命令 19 丢弃组字：那是宿主合成的上屏，核心不出上屏音。
      REQUIRE(focused
                  .configured_key(lease, key(0xDB, u'[', request), TsfPreeditStyle::Pinyin, {}, std::nullopt,
                                  WordCharacterBinding::Brackets)
                  .has_value());
      REQUIRE(focused.confirm(lease, request++));
      EXPECT_TRACE({"key:0", "select_edge:0", "command:" + std::to_string(MSIME_CANCEL_AFTER_HOST_COMMIT)});

      // 交给应用的键（Aux 管道）：带 PASSTHROUGH，不派发，也不开新的一轮。
      REQUIRE(focused.passthrough_key(token, 3));
      EXPECT_TRACE({"key:" + std::to_string(lifecycle_passthrough_key(3))});
      REQUIRE(!focused.passthrough_key(token + 1, 3));
      EXPECT_TRACE({});

      // 韩文：TIP 已经从自己的宿主会话把上一个音节写进文档，这次派发的上屏只能观察；派发前打开、派发后关掉，上屏只在 Server 这一个会话里出一次声。
      fake_scheme = 4;
      REQUIRE(focused.configured_key(lease, key('R', u'r', request), TsfPreeditStyle::Pinyin, {}).has_value());
      REQUIRE(focused.confirm(lease, request++));
      EXPECT_TRACE({"key:0", "set_lifecycle:" + std::to_string(lifecycle_flags(true)), "character:114",
                    "set_lifecycle:" + std::to_string(lifecycle_flags())});
      REQUIRE(focused.configured_key(lease, key('K', u'k', request), TsfPreeditStyle::Pinyin, {}).has_value());
      REQUIRE(focused.confirm(lease, request++));
      EXPECT_TRACE({"key:0", "set_lifecycle:" + std::to_string(lifecycle_flags(true)), "character:107",
                    "set_lifecycle:" + std::to_string(lifecycle_flags())});
      REQUIRE(recorded == std::vector<std::string>({"啊", "啊", "r"}));
      fake_scheme = 0;
      REQUIRE(focused.cancel_composition(lease));
      calls.clear();

      // 整句改字时同样只能观察。
      REQUIRE(focused.configured_key(lease, key('C', u'c', request), TsfPreeditStyle::Pinyin, {}).has_value());
      REQUIRE(focused.confirm(lease, request++));
      fake_conversion = "此";
      calls.clear();
      REQUIRE(focused.configured_key(lease, key(0x20, u' ', request), TsfPreeditStyle::Pinyin, {}).has_value());
      REQUIRE(focused.confirm(lease, request++));
      EXPECT_TRACE({"key:1", "set_lifecycle:" + std::to_string(lifecycle_flags(true)),
                    "command:" + std::to_string(MSIME_COMMIT_CANDIDATE),
                    "set_lifecycle:" + std::to_string(lifecycle_flags())});
      fake_conversion.clear();

      // 鼠标点选：先报 MSIME_KEY_POINTER 开启新的一轮，再派发。
      REQUIRE(focused.configured_key(lease, key('D', u'd', request), TsfPreeditStyle::Pinyin, {}).has_value());
      REQUIRE(focused.confirm(lease, request++));
      calls.clear();
      REQUIRE(focused.select_candidate(lease, fake_session, fake_generation, 0).has_value());
      EXPECT_TRACE({"key:" + std::to_string(MSIME_KEY_POINTER), "select:0"});

      // 失去焦点：先交还音乐所有权，再让会话失焦。
      REQUIRE(focused.cancel(lease));
      EXPECT_TRACE({"set_active:0", "focus:0"});
    }
    // 销毁时核心自己关掉这个会话放的音乐，Server 不再调 music_set_active。
    EXPECT_TRACE({"destroy:0"});
    std::cout << "Windows lifecycle call order checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
