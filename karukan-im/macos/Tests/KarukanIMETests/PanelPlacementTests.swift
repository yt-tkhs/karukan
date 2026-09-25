import XCTest

@testable import KarukanIME

final class PanelPlacementTests: XCTestCase {
    // A 1000×800 screen with a 40-point menu bar and no dock.
    let visible = NSRect(x: 0, y: 0, width: 1000, height: 760)
    let size = NSSize(width: 200, height: 100)

    func testHangsUnderTheLineWhenThatFits() {
        let cursor = NSRect(x: 300, y: 400, width: 1, height: 20)
        let frame = PanelPlacement.frame(for: size, near: cursor, within: visible, gap: 4)
        XCTAssertEqual(frame, NSRect(x: 300, y: 296, width: 200, height: 100))
    }

    func testFlipsAboveWhenTheBottomIsTooClose() {
        let cursor = NSRect(x: 300, y: 50, width: 1, height: 20)
        let frame = PanelPlacement.frame(for: size, near: cursor, within: visible, gap: 4)
        XCTAssertEqual(frame.origin.y, 74)
        XCTAssertTrue(visible.contains(frame))
    }

    func testPullsBackFromTheRightEdge() {
        let cursor = NSRect(x: 950, y: 400, width: 1, height: 20)
        let frame = PanelPlacement.frame(for: size, near: cursor, within: visible, gap: 4)
        XCTAssertEqual(frame.maxX, visible.maxX)
        XCTAssertTrue(visible.contains(frame))
    }

    func testNeverStartsLeftOfTheScreen() {
        let cursor = NSRect(x: -30, y: 400, width: 1, height: 20)
        let frame = PanelPlacement.frame(for: size, near: cursor, within: visible, gap: 4)
        XCTAssertEqual(frame.minX, visible.minX)
    }

    func testSitsOnTheScreenBottomWhenNeitherSideFits() {
        let tall = NSSize(width: 200, height: 500)
        let cursor = NSRect(x: 300, y: 380, width: 1, height: 20)
        let frame = PanelPlacement.frame(for: tall, near: cursor, within: visible, gap: 4)
        XCTAssertTrue(visible.contains(frame))
        XCTAssertEqual(frame.minY, visible.minY)
    }

    func testShowsTheTopRowsWhenTallerThanTheScreen() {
        let huge = NSSize(width: 200, height: 900)
        let cursor = NSRect(x: 300, y: 380, width: 1, height: 20)
        let frame = PanelPlacement.frame(for: huge, near: cursor, within: visible, gap: 4)
        XCTAssertEqual(frame.maxY, visible.maxY)
    }

    func testCapsTheWidthToTheScreen() {
        let wide = NSSize(width: 1400, height: 100)
        let cursor = NSRect(x: 300, y: 400, width: 1, height: 20)
        let frame = PanelPlacement.frame(for: wide, near: cursor, within: visible, gap: 4)
        XCTAssertEqual(frame.width, visible.width)
        XCTAssertEqual(frame.minX, visible.minX)
    }

    func testNoScreenMeansNoClamping() {
        let cursor = NSRect(x: 950, y: 50, width: 1, height: 20)
        let frame = PanelPlacement.frame(for: size, near: cursor, within: nil, gap: 4)
        XCTAssertEqual(frame, NSRect(x: 950, y: -54, width: 200, height: 100))
    }
}
