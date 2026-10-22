import SwiftUI
import UIKit

// The desktop's palette (crates/tauri-app/ui/style.css): the same accent,
// near-black sheets in dark mode and the warm paper of light mode.

extension Color {
    static let bsBg = Color(light: 0xF4F1F5, dark: 0x0D0D0F)
    static let bsCard = Color(light: 0xFFFFFF, dark: 0x1C1C1F)
    static let bsCard2 = Color(light: 0xE9E5EC, dark: 0x2A2A2E)
    static let bsEdge = Color(light: 0x18121E, dark: 0xFFFFFF).opacity(0.08)
    static let bsAccent = Color(light: 0xCF5A24, dark: 0xF08A5D)
    static let bsGreen = Color(light: 0x187F45, dark: 0x2FB566)
    static let bsRed = Color(light: 0xBF3A2F, dark: 0xFF7A6E)

    init(light: UInt32, dark: UInt32) {
        self.init(UIColor { $0.userInterfaceStyle == .dark ? UIColor(hex: dark) : UIColor(hex: light) })
    }
}

extension UIColor {
    convenience init(hex: UInt32) {
        self.init(red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
                  blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
    }
}

/// The mark in the middle of the header: an accent disc crossed by a wave
/// that moves while a reply is coming in.
struct Orb: View {
    var busy = false
    var size: CGFloat = 64

    var body: some View {
        TimelineView(.animation(paused: !busy)) { tl in
            let phase = busy ? tl.date.timeIntervalSinceReferenceDate * 3 : 0
            ZStack {
                Circle().fill(Color.bsCard2)
                Circle().fill(Color.bsAccent).frame(width: size * 0.4, height: size * 0.4)
                Wave(phase: phase)
                    .stroke(Color.white, style: StrokeStyle(lineWidth: max(2, size * 0.045), lineCap: .round))
                    .frame(width: size * 0.62, height: size * 0.16)
            }
            .frame(width: size, height: size)
            .shadow(color: .black.opacity(0.25), radius: 10, y: 4)
        }
        .accessibilityLabel(busy ? "Working" : "Backspace")
    }
}

struct Wave: Shape {
    var phase: Double

    func path(in rect: CGRect) -> Path {
        var p = Path()
        let steps = 40
        for i in 0...steps {
            let t = Double(i) / Double(steps)
            let y = rect.midY - sin(t * .pi * 2 + phase) * rect.height / 2
            let pt = CGPoint(x: rect.minX + t * rect.width, y: y)
            if i == 0 { p.move(to: pt) } else { p.addLine(to: pt) }
        }
        return p
    }
}

/// The round buttons in the header and the drawer.
struct RoundButton: View {
    var system: String
    var label: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: system)
                .font(.system(size: 18, weight: .medium))
                .frame(width: 46, height: 46)
                .background(Circle().fill(Color.bsCard2))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }
}

/// A centred icon, title and line, for screens with nothing on them yet.
struct EmptyState: View {
    var icon: String
    var title: String
    var line: String

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: icon)
                .font(.system(size: 26))
                .foregroundStyle(.secondary)
                .frame(width: 60, height: 60)
                .background(RoundedRectangle(cornerRadius: 16).fill(Color.bsCard))
            Text(title).font(.title3.weight(.semibold)).multilineTextAlignment(.center)
            Text(line).font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
        }
        .padding(.horizontal, 36)
        .frame(maxWidth: .infinity)
    }
}

/// Inline Markdown (bold, code, links) the way the desktop renders replies;
/// plain text when it doesn't parse.
func markdown(_ s: String) -> AttributedString {
    (try? AttributedString(markdown: s, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(s)
}
