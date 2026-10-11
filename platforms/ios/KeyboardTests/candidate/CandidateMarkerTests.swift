import UIKit
import XCTest

/// Cloud, AI and pinned candidates are marked the way the Windows candidate window marks them.
final class CandidateMarkerTests: XCTestCase {
  func testCandidateSnapshotRejectsFractionalGenerationAndIndex() {
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": NSNumber(value: 4.5), "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": 4, "index": 0]]],
    ]))
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": 4, "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": 4, "index": 0.5]]],
    ]))
    let oversized = NSNumber(value: 1e40)
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": oversized, "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": oversized, "index": 0]]],
    ]))
  }

  func testCandidateSnapshotRejectsFractionalMarkerMetadata() {
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": 4, "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": 4, "index": 0], "source": 2.5]],
    ]))
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": 4, "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": 4, "index": 0], "fixed_position": 1.5]],
    ]))
  }

  func testCandidateSnapshotAcceptsIntegerZeroAndOneValues() throws {
    let snapshot = try CandidatePanelSnapshot.decode([
      "generation": 1, "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": 1, "index": 0], "source": 1,
                      "fixed_position": 0]],
    ])
    XCTAssertEqual(snapshot.generation, 1)
    XCTAssertEqual(snapshot.entries.first?.index, 0)
    XCTAssertEqual(snapshot.entries.first?.source, 1)
  }

  func testOnlyNetworkSourcesAndPinnedWordsAreMarked() {
    XCTAssertEqual(CandidateMarker.markers(source: 0, fixedPosition: 0), [])
    XCTAssertEqual(CandidateMarker.markers(source: 1, fixedPosition: 0), [], "a user word is still a dictionary word")
    XCTAssertEqual(CandidateMarker.markers(source: 2, fixedPosition: 0).map(\.spoken), ["云候选"])
    XCTAssertEqual(CandidateMarker.markers(source: 3, fixedPosition: 0).map(\.spoken), ["AI 候选"])
    XCTAssertEqual(CandidateMarker.markers(source: 0, fixedPosition: 3).map(\.spoken), ["已固定第 3 位"])
    for marker in CandidateMarker.markers(source: 2, fixedPosition: 0) + CandidateMarker.markers(source: 3, fixedPosition: 1) {
      XCTAssertNotNil(UIImage(systemName: marker.symbol), marker.symbol)
    }
  }

  func testThePanelKeepsEachCandidatesSourceAndPinnedSlot() throws {
    let snapshot = try CandidatePanelSnapshot.decode([
      "generation": 4, "preedit": "ni",
      "candidates": [
        ["text": "你", "id": ["generation": 4, "index": 0], "source": 0, "fixed_position": 1],
        ["text": "妮", "id": ["generation": 4, "index": 1], "source": 2, "fixed_position": 0],
        ["text": "泥", "id": ["generation": 4, "index": 2]],
      ],
    ])
    XCTAssertEqual(snapshot.entries.map(\.fixedPosition), [1, 0, 0])
    XCTAssertEqual(snapshot.entries.map(\.source), [0, 2, 0])
  }

  /// 展开的候选网格和候选条一样，把 AI 座位上的占位行画成次要色，读屏念等待中或没有结果（#6888）。
  func testTheExpandedPanelDrawsTheAIPlaceholderAsSecondary() throws {
    let panel = KeyboardCandidatePanelView(
      candidates: ["你", "妮", "…", "泥"], preedit: "ni", aiPlaceholders: [2],
      display: { $0 }, onSelect: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 240)
    panel.layoutIfNeeded()
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }
    func chip(_ number: Int) throws -> UIButton {
      try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-\(number)" } as? UIButton)
    }
    let skin = KeyboardTheme.current
    XCTAssertEqual(try chip(3).configuration?.baseForegroundColor, skin.secondary)
    XCTAssertEqual(try chip(3).accessibilityLabel, "候选词 3：AI 候选等待中")
    XCTAssertEqual(try chip(4).configuration?.baseForegroundColor, skin.keyForeground)
    XCTAssertEqual(try chip(4).accessibilityLabel, "候选词 4：泥")

    panel.reload(candidates: ["你", "妮", "—"], annotations: [], markers: [], aiPlaceholders: [2])
    XCTAssertEqual(try chip(3).configuration?.baseForegroundColor, skin.secondary)
    XCTAssertEqual(try chip(3).accessibilityLabel, "候选词 3：AI 候选没有结果")
  }
}
