import SwiftUI

/// 应用的云同步控制器，对应 Android 的 `home/CloudSync`：设置（含当前自定义键盘皮肤）和无编码常用语。
///
/// 只在真实账号且用户打开了同步开关时运行，登录本身不触发。应用每次回到前台（`scenePhase` 变为 `.active`）调用 `onActive`，距上次尝试满 5 分钟、或有分类带着待上传标记且满 30 秒时才真正跑（`BackendSyncPolicy.due`）。某个分类第一次同步而云端已有数据时，先弹「合并 / 使用云端」让用户选，选了才执行。
///
/// 失败静默，下次回到前台再试；最近一次的失败原因只留一行给「我的」页的同步行（`statusLine`）。个人词库不在自动同步里：用户词库由键盘里的 Engine 持有，应用进程导不出来，仍在「手动同步」里上传下载。
@MainActor
final class CloudSync: ObservableObject {
  static let shared = CloudSync()

  /// 「我的」页显示的状态，每次跑完和每次 `refresh` 后更新。
  @Published private(set) var record = CloudSyncRecord()
  /// 「云端已有同步数据」的选择是否正在显示。
  @Published var asksFirstRun = false

  private let state: CloudSyncState
  private let session: BackendAccountSession
  private let client: BackendAccountClient
  private var running = false

  init(state: CloudSyncState = .shared, session: BackendAccountSession = .shared,
       client: BackendAccountClient = BackendAccountClient()) {
    self.state = state
    self.session = session
    self.client = client
  }

  /// 回到前台时调用，按节流决定要不要跑。
  func onActive() { request(choice: nil, force: false) }

  /// 用户刚打开同步开关时调用：不等节流，马上比对一次（首次需要选择时会弹出选择）。
  func runNow() { request(choice: nil, force: true) }

  /// 用户在「云端已有同步数据」里选了一项。
  func choose(_ choice: BackendSyncPolicy.Choice) { request(choice: choice, force: true) }

  /// 与当前登录对齐后重新读状态。换了账号或已经退出时，这一步会关掉开关、清空进度。
  func refresh() async {
    if let record = try? await reconcile() { self.record = record }
  }

  /// 打开或关闭同步。打开时把全部分类标成待上传，并马上比对一次。没有真实账号时抛 `CloudSyncState.Failure.notSignedIn`。
  func setEnabled(_ enabled: Bool) async throws {
    _ = try await reconcile()
    try state.setEnabled(enabled)
    record = try state.read()
    if enabled { runNow() }
  }

  /// 「我的」页同步行的一行状态（`BackendSyncPolicy.statusLine`）。
  static func statusLine(_ record: CloudSyncRecord, now: Date = Date()) -> String {
    BackendSyncPolicy.statusLine(enabled: record.enabled, error: record.error, unheldPhrases: record.unheldPhrases.count,
                                 lastSynced: record.lastSynced, now: now)
  }

  private func reconcile() async throws -> CloudSyncRecord {
    let login = try await session.login()
    return try state.bind(accountID: login?.userID, sessionID: login?.sessionID)
  }

  private func request(choice: BackendSyncPolicy.Choice?, force: Bool) {
    guard !running else { return }
    running = true
    let state = state, session = session, client = client
    Task {
      let outcome = await Task.detached(priority: .utility) {
        await CloudSyncRun.perform(state: state, session: session, client: client, choice: choice, force: force)
      }.value
      running = false
      if let record = try? state.read() { self.record = record }
      if outcome == .needsChoice { asksFirstRun = true }
    }
  }
}

/// 一次同步，在后台执行。先把各分类的云端状态都读一遍，决定要不要问用户，再逐类执行；一类失败不挡其他类。
struct CloudSyncRun {
  enum Outcome: Equatable { case skipped, needsChoice, finished }

  let state: CloudSyncState
  let session: BackendAccountSession
  let client: BackendAccountClient
  let choice: BackendSyncPolicy.Choice?
  let accountID: String
  let sessionID: UUID
  let binding: Int64

  static func perform(state: CloudSyncState, session: BackendAccountSession, client: BackendAccountClient,
                      choice: BackendSyncPolicy.Choice?, force: Bool) async -> Outcome {
    // 钥匙串读不出来时不对账：那不等于退出登录，清掉进度会让下一次变成首次同步。
    guard let login = try? await session.login(),
          let record = try? state.bind(accountID: login.userID, sessionID: login.sessionID),
          record.enabled else { return .skipped }
    let now = Date()
    guard force || choice != nil || BackendSyncPolicy.due(now: now, lastAttempt: record.lastAttempt, anyDirty: record.anyDirty)
    else { return .skipped }
    let binding = record.bindingGeneration
    guard (try? state.updateIfCurrent(binding, { $0.lastAttempt = now })) == true else { return .skipped }
    var run = CloudSyncRun(state: state, session: session, client: client, choice: choice,
                           accountID: login.userID, sessionID: login.sessionID, binding: binding)
    let failure: String?
    do {
      guard try await run.probe() else {
        guard await run.current() else { return .skipped }
        guard BackendSyncPolicy.promptDue(now: now, promptedAt: record.promptedAt, force: force) else { return .skipped }
        _ = try? state.updateIfCurrent(binding) { $0.promptedAt = now }
        return .needsChoice
      }
      failure = try await run.run()
    } catch {
      // 任务取消或跑的过程中退出登录、换了账号：结果作废，不写进度。
      guard let message = BackendSyncPolicy.failureMessage(error, fallback: "同步失败，稍后自动重试") else { return .skipped }
      failure = message
    }
    guard await run.current() else { return .skipped }
    _ = try? state.updateIfCurrent(binding) { record in
      if let failure {
        record.error = failure
      } else {
        record.error = nil
        record.lastSynced = Date()
      }
    }
    return .finished
  }

  /// 开始这一轮的那次登录仍然是当前登录，开关也还开着。每次写本机之前都要再确认一次。
  func current() async -> Bool {
    guard let login = try? await session.login(), login.userID == accountID, login.sessionID == sessionID,
          let record = try? state.read() else { return false }
    return record.enabled && record.bindingGeneration == binding
  }

  private func first(_ section: CloudSyncSection) throws -> Bool {
    try state.read().cursor(section).isEmpty
  }

  /// 带着这次登录的令牌发一个请求；令牌被拒时换一枚重试一次。登录在这期间变了时抛 `CancellationError`。
  private func call<T: Sendable>(_ operation: @Sendable (String) async throws -> T) async throws -> T {
    try await session.authenticated(matchingUserID: accountID, matchingSessionID: sessionID, operation).value
  }

  /// 读云端状态；返回 false 表示有分类第一次同步而云端已有数据，需要先问用户。
  mutating func probe() async throws -> Bool {
    guard await current() else { throw CancellationError() }
    return true
  }

  /// 逐类执行，一类失败不挡其他类；返回第一条要展示的失败原因，全部成功时为 nil。任务取消、登录变了时抛 `CancellationError`，这一轮作废。
  func run() async throws -> String? {
    nil
  }

  private func mode(_ section: CloudSyncSection, cloudHasData: Bool, cloudChanged: Bool, dirty: Bool) throws -> BackendSyncPolicy.Mode {
    if try first(section) {
      return BackendSyncPolicy.firstRun(cloudHasData: cloudHasData, choice: choice) ?? .none
    }
    return BackendSyncPolicy.incremental(cloudChanged: cloudChanged, localDirty: dirty)
  }
}

/// 首次同步时「云端已有同步数据」的选择，挂在应用根视图上：回到前台时用户在哪个标签页都能看到。
struct CloudSyncFirstRunPrompt: ViewModifier {
  @ObservedObject var sync: CloudSync

  func body(content: Content) -> some View {
    content.confirmationDialog("云端已有同步数据", isPresented: $sync.asksFirstRun, titleVisibility: .visible) {
      Button("合并") { sync.choose(.merge) }
      Button("使用云端") { sync.choose(.useCloud) }
      Button("取消", role: .cancel) {}
    } message: {
      Text("合并会保留两边的内容，同一项以本机为准；使用云端会用云端的内容替换本机")
    }
  }
}
