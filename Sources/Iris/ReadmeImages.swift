import AppKit
import IrisCore
import IrisEngine
import SwiftUI

/// Design check: `Iris --readme-images <dir>` draws the README's pictures from the real
/// components, with sample text that suits each one, so they can be redrawn whenever the
/// design changes (copy them to docs/images). Nothing is read, checked or pasted.
@MainActor
enum ReadmeImages {
    static func render(into dir: String) {
        FontRegistry.registerAll()
        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        let draft = "hey, i have checked the PR and its look fine but one thing need to change before merge @lead"
        let fixed = "hey, I have checked the PR and it looks fine, but one thing needs to change before merge @lead"
        let professional = "Hi @lead, I've reviewed the PR and it looks good. One thing needs to change before we merge."
        let modes = ToneList.newMac.shown
        let tone = modes.first { $0.id == "professional" } ?? (modes.count > 1 ? modes[1] : .fix)

        // The banner: setup's printed sky plate, the wordmark, a stair headline and the garden
        // in full bloom with its butterflies, a giant ghost word cut off by the edge.
        ComponentSnapshot.hosted("hero", width: 1280, dir: dir, prefix: "readme") {
            AnyView(ZStack(alignment: .topLeading) {
                ScreenPlate(ink: .sky, seed: 11)
                Text("WRITE").font(DS.Font.chunky(300)).foregroundStyle(DS.Surface.overlay.opacity(0.3))
                    .fixedSize().offset(x: -30, y: 190).accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 26) {
                    IrisLogo(size: 52)
                    VStack(alignment: .leading, spacing: -10) {
                        Text("FIX YOUR GRAMMAR.").font(DS.Font.display(66))
                        HStack(spacing: 14) {
                            Text("✻").font(DS.Font.display(66))
                            Text("KEEP YOUR VOICE.").font(DS.Font.display(66))
                        }
                        .padding(.leading, 64)
                    }
                    .foregroundStyle(DS.Ink.i12).fixedSize()
                    HStack(spacing: 12) {
                        Text("Grammar and tone in every app on your Mac.").font(DS.Font.martian(19)).foregroundStyle(DS.Ink.i12)
                        InkLabel(text: "Nothing leaves your Mac.", size: 16)
                    }
                    .fixedSize()
                }
                .padding(.leading, 64).padding(.top, 58)
                GardenView(stage: .tried, progress: 1, frozenAt: 1003)
                    .scaleEffect(2.1, anchor: .bottomTrailing)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                    .padding(.trailing, 70).padding(.bottom, 18)
            }
            .frame(width: 1280, height: 440)
            .clipped())
        }

        // The Download button: ink printed through the screen with the pink plate off register,
        // like the app's primary button, big enough to find at a glance.
        ComponentSnapshot.pure("download", width: 800, dir: dir, prefix: "readme") {
            HStack(spacing: 22) {
                HStack(spacing: 14) {
                    PenGlyph(name: "arrow.down", size: 26, weight: 3).foregroundStyle(DS.Surface.overlay)
                    Text("Download Iris").font(DS.Font.display(40)).foregroundStyle(DS.Surface.overlay)
                }
                .padding(.horizontal, 30).padding(.vertical, 18)
                .background {
                    ZStack {
                        HandRect(seed: 4, wobble: 0.4, closed: true).printedInk(Riso.pink, seed: 3).offset(x: -6, y: -5)
                        HandRect(seed: 5, wobble: 0.4, closed: true).printedInk(Riso.ink, seed: 9)
                    }
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text("free · for Mac").font(DS.Font.brush(30)).foregroundStyle(DS.Ink.i12)
                    Text("macOS 15+ · Apple Silicon").font(DS.Font.martian(15)).foregroundStyle(DS.Ink.i10)
                }
            }
            .padding(.horizontal, 28).padding(.vertical, 24)
            .fixedSize()
            .frame(width: 800)
            .background(DS.Surface.overlay)
        }

        // The one-time "Open Anyway", drawn in Iris's pen rather than screenshotted: what macOS
        // shows, where to click, and the brush ring round the button.
        ComponentSnapshot.pure("open-anyway", width: 1240, dir: dir, prefix: "readme") {
            InstallSteps().padding(36).background(DS.Surface.overlay)
        }

        func panel(_ result: RewriteResult, selected: Int) -> some View {
            let model = PanelModel()
            model.preview(original: draft, result: result, selected: selected, engine: .ready(.best))
            return PanelView(model: model).padding(28).background(DS.Surface.ground)
        }
        ComponentSnapshot.pure("fix", width: 760, dir: dir, prefix: "readme") {
            panel(RewriteResult(mode: .fix, original: draft, text: fixed, segments: WordDiff.diff(draft, fixed),
                                verdict: .minor, latencyMs: 300), selected: 0)
        }
        ComponentSnapshot.pure("tone", width: 760, dir: dir, prefix: "readme") {
            panel(RewriteResult(mode: tone, original: draft, text: professional, segments: WordDiff.diff(draft, professional),
                                verdict: nil, latencyMs: 600), selected: modes.firstIndex { $0.id == tone.id } ?? 1)
        }
        ComponentSnapshot.pure("quick-fix", width: 520, dir: dir, prefix: "readme") {
            ToastView(message: .fixed(4, undoKeys: true))
                .padding(28).frame(maxWidth: .infinity, alignment: .leading).background(DS.Surface.ground)
        }
        ComponentSnapshot.pure("menu", width: 400, dir: dir, prefix: "readme") {
            let model = StatusMenu.Model()
            model.engine = .ready(.best)
            // The rows the app shows (`SlipRows`), with the default shortcuts.
            model.groups = SlipRows.build(.init(openShortcut: KeyCombo.openDefault.symbols,
                                                quickFixShortcut: KeyCombo.quickFixDefault.symbols, modelReady: true))
            return StatusMenuView(model: model, pick: { _ in }, frozenAt: 1000).padding(24).background(Color(white: 0.82))
        }

        ComponentSnapshot.hosted("setup", width: OnboardingView.width, dir: dir, prefix: "readme") {
            AnyView(OnboardingView(frozen: .init(facts: .init(trusted: false, modelOnDisk: false)), scrolls: false, onFinish: {}).frame(height: OnboardingView.height, alignment: .top).clipped())
        }

        // Words Iris never changes, with sample words, put back afterwards.
        let settings = AppSettings.shared
        let saved = settings.glossary
        settings.glossary = ["Northwind", "OKR", "Kubernetes", "ACME-2041"]
        ComponentSnapshot.hosted("words", width: SettingsView.size.width, dir: dir, prefix: "readme") {
            AnyView(SettingsView(page: .words, scrolls: false))
        }
        settings.glossary = saved
        ComponentSnapshot.hosted("tones", width: SettingsView.size.width, dir: dir, prefix: "readme") {
            AnyView(SettingsView(page: .tones, scrolls: false))
        }
    }
}

/// The three moments of opening Iris for the first time, drawn as paper slips in pen.
private struct InstallSteps: View {
    private let icon = NSImage(contentsOfFile: Bundle.main.bundlePath + "/Contents/Resources/AppIcon.icns")
        ?? RisoArt.flower(points: 64, crop: .icon)

    var body: some View {
        HStack(alignment: .top, spacing: 26) {
            step(1, "Open Iris", "macOS can't check apps that aren't from the App Store, so it stops the first open. Click Done.") {
                VStack(spacing: 10) {
                    Image(nsImage: icon).resizable().frame(width: 54, height: 54)
                    Text("“Iris” Not Opened").font(DS.Font.martian(15)).bold().foregroundStyle(DS.Ink.i12)
                    Text("Apple could not verify “Iris” is free of malware…").font(DS.Font.martian(11.5))
                        .foregroundStyle(DS.Ink.i10).multilineTextAlignment(.center).fixedSize(horizontal: false, vertical: true)
                    HStack(spacing: 8) {
                        fakeButton("Done", ringed: true)
                        fakeButton("Move to Trash", ringed: false)
                    }
                }
            }
            step(2, "Open Anyway", "System Settings › Privacy & Security. Scroll down to Security and click Open Anyway.") {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Security").font(DS.Font.martian(13)).bold().foregroundStyle(DS.Ink.i12)
                    PenRule(seed: 71)
                    Text("“Iris” was blocked to protect your Mac.").font(DS.Font.martian(11.5)).foregroundStyle(DS.Ink.i11)
                        .fixedSize(horizontal: false, vertical: true)
                    HStack { Spacer(); fakeButton("Open Anyway", ringed: true) }
                }
            }
            step(3, "Confirm, once", "macOS asks one more time. Click Open Anyway and enter your password. From then on Iris opens like any app.") {
                VStack(spacing: 12) {
                    Image(nsImage: icon).resizable().frame(width: 54, height: 54)
                    Text("Open “Iris”?").font(DS.Font.martian(15)).bold().foregroundStyle(DS.Ink.i12)
                    HStack(spacing: 8) {
                        fakeButton("Open Anyway", ringed: true)
                        fakeButton("Done", ringed: false)
                    }
                }
            }
        }
    }

    private func step<Art: View>(_ n: Int, _ title: String, _ words: String, @ViewBuilder art: () -> Art) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(String(format: "%02d", n)).font(DS.Font.martian(16)).foregroundStyle(DS.Hue.pinkText)
                Text(title).font(DS.Font.brush(30)).foregroundStyle(DS.Ink.i12)
            }
            art()
                .padding(18)
                .frame(maxWidth: .infinity, minHeight: 190)
                .background(HandRect(seed: 30 + n, wobble: 0.5, closed: true).fill(DS.Surface.well))
                .overlay(HandRect(seed: 40 + n, wobble: 0.5, closed: true).stroke(DS.Ink.i12.opacity(0.55), lineWidth: 1.3))
            Text(words).font(DS.Font.martian(14)).foregroundStyle(DS.Ink.i11).lineSpacing(3)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(width: 370, alignment: .topLeading)
    }

    private func fakeButton(_ title: String, ringed: Bool) -> some View {
        Text(title).font(DS.Font.martian(12.5)).foregroundStyle(DS.Ink.i12)
            .padding(.horizontal, 12).padding(.vertical, 6)
            .background(HandRect(seed: title.count, wobble: 0.4, closed: true).fill(DS.Surface.overlay))
            .overlay(HandRect(seed: title.count + 3, wobble: 0.4, closed: true).stroke(DS.Ink.i12.opacity(0.6), lineWidth: 1))
            .overlay { if ringed { BrushRing(rgb: Riso.pink, seed: title.count).padding(-12) } }
            .fixedSize()
    }
}
