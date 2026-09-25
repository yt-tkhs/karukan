import Cocoa

/// Custom candidate window (borderless non-activating NSPanel).
///
/// The engine pre-paginates: `show` receives only the visible page, so
/// this controller just renders rows. An optional aux line (reading hint /
/// model info from the engine, which also carries the page indicator) is
/// shown as a footer.
///
/// The rows sit on Liquid Glass (`NSGlassEffectView`, macOS 26+) so the
/// panel reads like the system's own menus and the built-in Japanese IME's
/// candidate window; older systems get the popover material with the same
/// rounded shape. The selected row is an accent-colored rounded highlight,
/// numbers sit in a narrow left column and annotations hug the right edge,
/// matching the built-in IME's layout.
class CandidateWindowController {
    // Visual scale of the panel. Candidate rows use a larger type size
    // than the number column, annotations and the footer, matching the
    // system Japanese IME's proportions.
    static let candidateFontSize: CGFloat = 18
    static let detailFontSize: CGFloat = 13
    static let footerFontSize: CGFloat = 12
    private static let minPanelWidth: CGFloat = 160
    private static let cornerRadius: CGFloat = 12
    /// Gap between the composition's line rect and the panel edge, so the
    /// glass rim doesn't touch the text line.
    private static let cursorGap: CGFloat = 4

    private let panel: NSPanel
    private let stackView: NSStackView
    private var rowViews: [NSView] = []
    private var auxText: String?

    private struct PageState {
        let candidates: [CandidateItem]
        let cursor: Int
    }
    private var pageState: PageState?

    init() {
        panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 200, height: 100),
            styleMask: [.nonactivatingPanel, .borderless],
            backing: .buffered,
            defer: true
        )
        panel.level = .popUpMenu
        panel.hidesOnDeactivate = false
        // A transparent window: the glass paints the panel's shape, and
        // the window shadow follows that rounded shape.
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.ignoresMouseEvents = true

        stackView = NSStackView()
        stackView.orientation = .vertical
        stackView.alignment = .leading
        stackView.spacing = 2
        stackView.edgeInsets = NSEdgeInsets(top: 6, left: 6, bottom: 6, right: 6)
        stackView.translatesAutoresizingMaskIntoConstraints = false

        let content = NSView()
        content.addSubview(stackView)
        NSLayoutConstraint.activate([
            stackView.topAnchor.constraint(equalTo: content.topAnchor),
            stackView.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            stackView.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            stackView.bottomAnchor.constraint(equalTo: content.bottomAnchor),
        ])

        let backdrop = Self.makeBackdrop(content: content)
        panel.contentView = backdrop
        content.frame = backdrop.bounds
    }

    /// The glass the rows sit on. `NSGlassEffectView` manages its
    /// `contentView`'s frame itself, so the content follows the backdrop
    /// by autoresizing mask rather than by constraints that could fight it.
    private static func makeBackdrop(content: NSView) -> NSView {
        content.autoresizingMask = [.width, .height]
        if #available(macOS 26.0, *) {
            let glass = NSGlassEffectView()
            glass.cornerRadius = cornerRadius
            // `.clear` is the style behind the built-in IME's unreadable
            // white-on-white rows in dark mode over light documents;
            // `.regular` keeps the appearance's own tone under the text.
            glass.style = .regular
            glass.contentView = content
            return glass
        }
        let effect = NSVisualEffectView()
        effect.material = .popover
        effect.blendingMode = .behindWindow
        effect.state = .active
        effect.maskImage = roundedRectMask(radius: cornerRadius)
        effect.addSubview(content)
        return effect
    }

    /// A stretchable rounded-rect mask: the corners come from a small
    /// image whose middle is stretched, so any panel size stays rounded.
    private static func roundedRectMask(radius: CGFloat) -> NSImage {
        let side = radius * 2 + 1
        let image = NSImage(size: NSSize(width: side, height: side), flipped: false) { rect in
            NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius).fill()
            return true
        }
        image.capInsets = NSEdgeInsets(top: radius, left: radius, bottom: radius, right: radius)
        image.resizingMode = .stretch
        return image
    }

    var isVisible: Bool { panel.isVisible }

    /// `cursorRect: nil` reuses the rect from the previous `show` — the
    /// caller can skip its (synchronous, per-keystroke) client IPC while
    /// the panel is already on screen, since the composition anchor
    /// doesn't move mid-composition.
    func show(candidates: [CandidateItem], cursor: Int, cursorRect: NSRect?) {
        pageState = PageState(candidates: candidates, cursor: cursor)
        render(cursorRect: cursorRect)
    }

    /// Update the aux footer; re-renders in place if the window is visible.
    /// Pass `deferRender: true` when a `show`/`hide` follows in the same
    /// action batch, so the panel is rendered once per batch instead of
    /// once for the aux change and again for the candidates.
    func setAux(_ text: String?, deferRender: Bool = false) {
        auxText = text
        if !deferRender, panel.isVisible, pageState != nil {
            render(cursorRect: nil)
        }
    }

    func hide() {
        pageState = nil
        panel.orderOut(nil)
    }

    private func render(cursorRect: NSRect?) {
        clearRows()
        guard let state = pageState else {
            hide()
            return
        }
        // An empty list still renders while the aux footer has text: a
        // source-filtered view narrowed to an empty source must show its
        // 「候補なし」 footer, not a silently vanishing panel.
        if state.candidates.isEmpty && (auxText ?? "").isEmpty {
            hide()
            return
        }

        for (index, candidate) in state.candidates.enumerated() {
            addRow(CandidateRowView(candidate: candidate, number: index + 1, selected: index == state.cursor))
        }
        if let aux = auxText, !aux.isEmpty {
            addFooter(aux, separated: !state.candidates.isEmpty)
        }

        positionPanel(cursorRect: cursorRect)
    }

    private func clearRows() {
        for view in rowViews {
            stackView.removeArrangedSubview(view)
            view.removeFromSuperview()
        }
        rowViews.removeAll()
    }

    /// Rows stretch to the panel width so the selection highlight spans
    /// the whole row, not just its text. Pinned explicitly: the stack's
    /// `.width` alignment leaves rows hugging their content.
    private func addRow(_ view: NSView) {
        stackView.addArrangedSubview(view)
        let insets = stackView.edgeInsets
        view.widthAnchor.constraint(
            equalTo: stackView.widthAnchor, constant: -(insets.left + insets.right)
        ).isActive = true
        rowViews.append(view)
    }

    /// The aux line, set off from the candidates by a hairline like a menu
    /// section. With no candidates above it the line would only underline
    /// the panel's top edge, so it is left out.
    private func addFooter(_ text: String, separated: Bool) {
        if separated {
            let separator = FilledView(color: .separatorColor)
            separator.heightAnchor.constraint(equalToConstant: 1).isActive = true
            addRow(separator)
            stackView.setCustomSpacing(4, after: separator)
        }
        let label = NSTextField(labelWithString: text)
        label.font = .systemFont(ofSize: Self.footerFontSize)
        label.textColor = .secondaryLabelColor
        let footer = NSStackView(views: [label])
        footer.orientation = .horizontal
        footer.edgeInsets = NSEdgeInsets(top: 2, left: 10, bottom: 2, right: 10)
        addRow(footer)
    }

    private var lastCursorRect: NSRect = .zero

    private func positionPanel(cursorRect: NSRect?) {
        if let rect = cursorRect {
            lastCursorRect = rect
        }
        let cursorRect = lastCursorRect

        // The stack's edge insets are the panel's padding, so its fitting
        // size is the panel size.
        stackView.layoutSubtreeIfNeeded()
        let contentSize = stackView.fittingSize
        let panelSize = NSSize(
            width: max(contentSize.width, Self.minPanelWidth), height: contentSize.height)

        guard cursorRect != .zero else {
            panel.setFrame(NSRect(origin: NSPoint(x: 100, y: 100), size: panelSize), display: true)
            panel.orderFront(nil)
            return
        }

        // The screen the composition is on, not `NSScreen.main`: with two
        // displays the main one can be the other, whose edges would flip
        // and clamp the panel for no reason.
        let screen =
            NSScreen.screens.first { $0.frame.contains(cursorRect.origin) } ?? NSScreen.main
        let frame = PanelPlacement.frame(
            for: panelSize, near: cursorRect, within: screen?.visibleFrame, gap: Self.cursorGap)
        panel.setFrame(frame, display: true)
        panel.orderFront(nil)
    }
}

/// Where the candidate panel goes relative to the composition's line rect:
/// under it when that fits on the screen, else above it, and never past a
/// screen edge — a composition near the right edge or the bottom used to
/// push the panel off screen. Pure, so it is unit-tested without a window.
enum PanelPlacement {
    /// The panel frame for `size` hanging `gap` points under `cursor`, kept
    /// inside `visible` (the screen's visible frame; `nil` skips the
    /// clamping when no screen is known).
    static func frame(for size: NSSize, near cursor: NSRect, within visible: NSRect?, gap: CGFloat)
        -> NSRect
    {
        let below = cursor.minY - gap - size.height
        guard let visible else {
            return NSRect(x: cursor.minX, y: below, width: size.width, height: size.height)
        }
        // Wider than the screen: show its left edge, which carries the
        // numbers and the candidates.
        let width = min(size.width, visible.width)
        let height = size.height

        // Under the line when it fits, above it when that fits, otherwise
        // wherever it fits at all — top rows first when even that is too
        // tall.
        let above = cursor.maxY + gap
        let y: CGFloat
        if below >= visible.minY {
            y = below
        } else if above + height <= visible.maxY {
            y = above
        } else if height <= visible.height {
            y = visible.minY
        } else {
            y = visible.maxY - height
        }

        // Start at the composition's left edge, pulled back from the right
        // edge as far as needed.
        let x = max(visible.minX, min(cursor.minX, visible.maxX - width))
        return NSRect(x: x, y: y, width: width, height: height)
    }
}

/// One candidate row: number column, surface, and the annotation hugging
/// the right edge. The selected row is painted as an accent-colored
/// rounded highlight; painting it in `updateLayer` keeps the color right
/// across appearance and accent-color changes.
private final class CandidateRowView: NSView {
    private static let cornerRadius: CGFloat = 6
    private static let numberColumnWidth: CGFloat = 14

    private let selected: Bool

    init(candidate: CandidateItem, number: Int, selected: Bool) {
        self.selected = selected
        super.init(frame: .zero)
        wantsLayer = true

        let primary: NSColor = selected ? .selectedMenuItemTextColor : .labelColor
        let secondary: NSColor =
            selected ? NSColor.selectedMenuItemTextColor.withAlphaComponent(0.8) : .secondaryLabelColor

        let numberLabel = NSTextField(labelWithString: "\(number)")
        numberLabel.font = .monospacedDigitSystemFont(
            ofSize: CandidateWindowController.detailFontSize, weight: .regular)
        numberLabel.alignment = .right
        numberLabel.textColor = secondary
        numberLabel.widthAnchor.constraint(equalToConstant: Self.numberColumnWidth).isActive = true

        let surfaceLabel = NSTextField(labelWithString: candidate.text)
        surfaceLabel.font = .systemFont(ofSize: CandidateWindowController.candidateFontSize)
        surfaceLabel.textColor = primary

        let row = NSStackView()
        row.orientation = .horizontal
        row.alignment = .firstBaseline
        row.spacing = 8
        row.edgeInsets = NSEdgeInsets(top: 4, left: 10, bottom: 4, right: 10)
        row.translatesAutoresizingMaskIntoConstraints = false
        row.addView(numberLabel, in: .leading)
        row.addView(surfaceLabel, in: .leading)
        if let description = candidate.description {
            let annotationLabel = NSTextField(labelWithString: description)
            annotationLabel.font = .systemFont(ofSize: CandidateWindowController.detailFontSize)
            annotationLabel.textColor = secondary
            row.addView(annotationLabel, in: .trailing)
        }

        addSubview(row)
        NSLayoutConstraint.activate([
            row.topAnchor.constraint(equalTo: topAnchor),
            row.leadingAnchor.constraint(equalTo: leadingAnchor),
            row.trailingAnchor.constraint(equalTo: trailingAnchor),
            row.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    override var wantsUpdateLayer: Bool { true }

    override func updateLayer() {
        guard let layer else { return }
        layer.cornerRadius = Self.cornerRadius
        layer.backgroundColor = selected ? NSColor.controlAccentColor.cgColor : nil
    }
}

/// A view filled with one dynamic color, resolved in `updateLayer` so it
/// follows appearance changes.
private final class FilledView: NSView {
    private let color: NSColor

    init(color: NSColor) {
        self.color = color
        super.init(frame: .zero)
        wantsLayer = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    override var wantsUpdateLayer: Bool { true }

    override func updateLayer() {
        layer?.backgroundColor = color.cgColor
    }
}
