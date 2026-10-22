import SwiftUI
import UIKit

struct ChatView: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        if let chat = store.openChat, !chat.messages.isEmpty {
            thread(chat)
        } else if store.openChatId != nil && store.openChat == nil {
            ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            VStack(spacing: 6) {
                Spacer()
                Text("What should we work on?").font(.title3.weight(.semibold))
                Text(store.demo ? "Demo" : store.machineName).font(.subheadline).foregroundStyle(.secondary)
                Spacer()
                Spacer()
            }
            .frame(maxWidth: .infinity)
        }
    }

    private func thread(_ chat: ChatThread) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    Text(chat.title)
                        .font(.footnote.weight(.medium))
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity)
                        .padding(.bottom, 4)
                    ForEach(chat.messages) { m in
                        Bubble(msg: m).id(m.id)
                    }
                    Text("Backspace is AI and can make mistakes.")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .frame(maxWidth: .infinity)
                        .padding(.top, 8)
                        .id("end")
                }
                .padding(.horizontal, 16)
                .padding(.top, 8)
                // Room for the composer and the tab bar floating over it.
                .padding(.bottom, 150)
            }
            .scrollDismissesKeyboard(.interactively)
            .onAppear { proxy.scrollTo("end", anchor: .bottom) }
            .onChange(of: chat.messages.last?.text) { proxy.scrollTo("end", anchor: .bottom) }
            .onChange(of: chat.messages.count) { withAnimation { proxy.scrollTo("end", anchor: .bottom) } }
        }
    }
}

struct Bubble: View {
    @Environment(AppStore.self) private var store
    let msg: ChatMessage

    private static let tapbacks = ["👍", "❤️", "😂", "‼️", "❓"]

    var body: some View {
        VStack(alignment: msg.isUser ? .trailing : .leading, spacing: 6) {
            HStack {
                if msg.isUser { Spacer(minLength: 48) }
                content
                    .padding(.horizontal, 16).padding(.vertical, 12)
                    .background(RoundedRectangle(cornerRadius: 22).fill(msg.isUser ? Color.bsCard2 : Color.bsCard))
                    .contextMenu { menu }
                if !msg.isUser { Spacer(minLength: 32) }
            }
            if let r = msg.reactions, !r.isEmpty {
                Text(r.joined(separator: " "))
                    .font(.footnote)
                    .padding(.horizontal, 8).padding(.vertical, 3)
                    .background(Capsule().fill(Color.bsCard2))
            }
            if let error = msg.error {
                Label(error, systemImage: "clock")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 14).padding(.vertical, 9)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 14).fill(Color.bsCard))
            } else if msg.status == "stopped" {
                Text("Stopped").font(.caption).foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: msg.isUser ? .trailing : .leading)
    }

    @ViewBuilder private var content: some View {
        if msg.isLive && msg.text.isEmpty {
            TypingDots()
        } else {
            Text(markdown(msg.text))
                .font(.body)
                .textSelection(.enabled)
                .opacity(msg.status == "sending" ? 0.6 : 1)
        }
    }

    @ViewBuilder private var menu: some View {
        Button { UIPasteboard.general.string = msg.text } label: { Label("Copy", systemImage: "doc.on.doc") }
        Button { store.remember(msg.text) } label: { Label("Save to Memory", systemImage: "brain") }
        if !msg.id.hasPrefix("local-") {
            Menu {
                ForEach(Self.tapbacks, id: \.self) { e in
                    Button(e) { store.react(msg, e) }
                }
            } label: { Label("React", systemImage: "face.smiling") }
        }
    }
}

struct TypingDots: View {
    var body: some View {
        TimelineView(.animation) { tl in
            let t = tl.date.timeIntervalSinceReferenceDate
            HStack(spacing: 5) {
                ForEach(0..<3) { i in
                    Circle()
                        .frame(width: 7, height: 7)
                        .opacity(0.35 + 0.65 * max(0, sin(t * 5 - Double(i) * 0.8)))
                }
            }
            .foregroundStyle(.secondary)
            .padding(.vertical, 6)
        }
        .accessibilityLabel("Writing")
    }
}

/// "Message Backspace", floating over the chat. Sends into the open chat, or
/// starts one; turns into Stop while a reply comes in.
struct Composer: View {
    @Environment(AppStore.self) private var store
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        let busy = store.openChat?.isBusy ?? false
        let empty = text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        HStack(alignment: .bottom, spacing: 8) {
            TextField("Message \(store.machineName)", text: $text, axis: .vertical)
                .lineLimit(1...5)
                .focused($focused)
                .padding(.vertical, 9)
            Button {
                if busy && empty {
                    store.stop()
                } else {
                    store.send(text)
                    text = ""
                }
            } label: {
                Image(systemName: busy && empty ? "stop.fill" : "arrow.up")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(Color.bsBg)
                    .frame(width: 38, height: 38)
                    .background(Circle().fill(Color.primary.opacity(empty && !busy ? 0.3 : 1)))
            }
            .disabled(empty && !busy)
            .accessibilityLabel(busy && empty ? "Stop" : "Send")
        }
        .padding(.leading, 18).padding(.trailing, 6).padding(.vertical, 6)
        .background(RoundedRectangle(cornerRadius: 26).fill(Color.bsCard))
        .overlay(RoundedRectangle(cornerRadius: 26).stroke(Color.bsEdge))
        .padding(.horizontal, 14)
    }
}
