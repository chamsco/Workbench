import SwiftUI
import AVFoundation
import UIKit

/// First run: scan the QR code from Settings → Phone on the Mac, type its
/// address and token, or try the demo.
struct PairView: View {
    @Environment(AppStore.self) private var store
    @State private var scanning = false
    @State private var byHand = false
    @State private var address = ""
    @State private var token = ""
    @State private var checking = false
    @State private var error: String?

    var body: some View {
        ScrollView {
            VStack(spacing: 18) {
                Orb(busy: checking, size: 96).padding(.top, 60)
                Text("Pair with your Mac").font(.largeTitle.weight(.semibold))
                Text("On the Mac, open Backspace → Settings → Phone, turn on **Reachable from my network** and **Let my phone connect**, then scan the code it shows.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 12)

                Button { scanning = true } label: {
                    Label("Scan the code", systemImage: "qrcode.viewfinder")
                        .font(.headline)
                        .foregroundStyle(Color.bsBg)
                        .frame(maxWidth: .infinity).padding(.vertical, 16)
                        .background(Capsule().fill(Color.primary))
                }
                .buttonStyle(.plain)
                .padding(.top, 8)

                // A PasteButton reads the clipboard without the paste prompt.
                PasteButton(payloadType: String.self) { strings in
                    let p = strings.lazy.compactMap { Pairing(link: $0) }.first
                    Task { @MainActor in
                        if let p { pair(p) } else { error = "That isn't a Backspace pairing link." }
                    }
                }
                .labelStyle(.titleAndIcon)
                .buttonBorderShape(.capsule)

                Button(byHand ? "Hide" : "Enter it by hand") { withAnimation { byHand.toggle() } }
                    .foregroundStyle(.secondary)

                if byHand {
                    VStack(spacing: 10) {
                        TextField("http://192.168.1.4:7421", text: $address)
                            .keyboardType(.URL)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                        Divider()
                        SecureField("Token", text: $token)
                    }
                    .padding(16)
                    .background(RoundedRectangle(cornerRadius: 16).fill(Color.bsCard))
                    Button("Pair") {
                        let a = address.hasPrefix("http") ? address : "http://\(address)"
                        guard let url = URL(string: a.trimmingCharacters(in: .whitespaces)), !token.isEmpty else {
                            error = "Type the address and the token from Settings → Phone."
                            return
                        }
                        pair(Pairing(url: url, token: token.trimmingCharacters(in: .whitespaces), name: url.host() ?? "Mac"))
                    }
                    .fontWeight(.semibold)
                }

                if checking { ProgressView() }
                if let error {
                    Text(error).font(.footnote).foregroundStyle(Color.bsRed).multilineTextAlignment(.center)
                }

                Button("Try a demo") { store.useDemo() }
                    .foregroundStyle(.secondary)
                    .padding(.top, 20)
            }
            .padding(24)
        }
        .scrollDismissesKeyboard(.interactively)
        .sheet(isPresented: $scanning) {
            ScannerSheet { code in
                scanning = false
                if let p = Pairing(link: code) {
                    pair(p)
                } else {
                    error = "That code isn't a Backspace pairing code."
                }
            }
        }
    }

    private func pair(_ p: Pairing) {
        checking = true
        error = nil
        Task {
            defer { checking = false }
            do {
                let hello = try await Companion.hello(at: p.url)
                var p = p
                // The link names the Mac; a hand-typed pairing takes the name it reports.
                if p.name == p.url.host() { p.name = hello.name }
                store.connect(p)
            } catch {
                self.error = error.localizedDescription
            }
        }
    }
}

struct ScannerSheet: View {
    let onCode: (String) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            QRScanner(onCode: onCode)
                .ignoresSafeArea(edges: .bottom)
                .navigationTitle("Scan the code")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }
    }
}

struct QRScanner: UIViewControllerRepresentable {
    let onCode: (String) -> Void

    func makeUIViewController(context: Context) -> ScannerController {
        let c = ScannerController()
        c.onCode = onCode
        return c
    }

    func updateUIViewController(_ c: ScannerController, context: Context) {}
}

final class ScannerController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var onCode: ((String) -> Void)?
    private let session = AVCaptureSession()
    private var preview: AVCaptureVideoPreviewLayer?
    private var done = false

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configure()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { ok in
                Task { @MainActor in ok ? self.configure() : self.say("Backspace needs the camera to scan the code. Allow it in Settings, or enter the code by hand.") }
            }
        default:
            say("Backspace needs the camera to scan the code. Allow it in Settings, or enter the code by hand.")
        }
    }

    private func configure() {
        guard let device = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: device),
              session.canAddInput(input)
        else { return say("No camera here. Enter the code by hand instead.") }
        session.addInput(input)
        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else { return say("The camera can't read codes right now.") }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        layer.frame = view.bounds
        view.layer.addSublayer(layer)
        preview = layer
        let s = session
        DispatchQueue.global(qos: .userInitiated).async { s.startRunning() }
    }

    private func say(_ text: String) {
        let label = UILabel()
        label.text = text
        label.textColor = .white
        label.numberOfLines = 0
        label.textAlignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(label)
        NSLayoutConstraint.activate([
            label.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            label.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 32),
            label.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -32),
        ])
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        preview?.frame = view.bounds
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        let s = session
        DispatchQueue.global(qos: .userInitiated).async { s.stopRunning() }
    }

    nonisolated func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard let code = (objects.first as? AVMetadataMachineReadableCodeObject)?.stringValue else { return }
        // The delegate queue is main.
        MainActor.assumeIsolated {
            guard !done else { return }
            done = true
            onCode?(code)
        }
    }
}
