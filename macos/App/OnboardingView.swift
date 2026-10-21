import AppKit
import SwiftUI

/// The first-launch tutorial: practise with the real engine inside the app
/// (what it learns stays in memory), then switch to 輕注音.
struct OnboardingView: View {
    /// Set once the tutorial is finished or skipped.
    static let doneKey = "tutorialDone"

    var onFinish: () -> Void
    @StateObject private var tutor = Tutor()

    private let cardSize = CGSize(width: 440, height: 620)

    var body: some View {
        GeometryReader { geometry in
            ZStack {
                Backdrop()
                Card(tutor: tutor, onFinish: onFinish)
                    .frame(width: cardSize.width, height: cardSize.height)
                    .background {
                        // Frosted glass: the same backdrop, blurred and tinted.
                        ZStack {
                            Backdrop()
                                .frame(width: geometry.size.width, height: geometry.size.height)
                                .blur(radius: 44)
                            Color.black.opacity(0.32)
                            LinearGradient(
                                colors: [.white.opacity(0.10), .clear, .white.opacity(0.03)],
                                startPoint: .topLeading, endPoint: .bottomTrailing)
                        }
                        .frame(width: cardSize.width, height: cardSize.height)
                        .clipShape(RoundedRectangle(cornerRadius: 38, style: .continuous))
                    }
                    .overlay {
                        RoundedRectangle(cornerRadius: 38, style: .continuous)
                            .strokeBorder(
                                LinearGradient(
                                    colors: [.white.opacity(0.45), .white.opacity(0.08), .white.opacity(0.22)],
                                    startPoint: .topLeading, endPoint: .bottomTrailing),
                                lineWidth: 1)
                    }
                    .shadow(color: .black.opacity(0.28), radius: 40, y: 24)
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
        }
        .ignoresSafeArea()
        .frame(minWidth: 760, minHeight: 720)
        // Keys go to the lessons; the try-out has a real text field instead.
        .background(KeyCatcher(active: !tutor.isTryStep) { event in
            if tutor.practice != nil {
                tutor.handle(event)
            } else if tutor.isAddStep, event.keyCode == 36 || event.keyCode == 76 {
                tutor.go(to: tutor.step + 1)
            }
        })
        .background(TransparentTitleBar())
        .environment(\.colorScheme, .dark)
    }
}

// MARK: - State

@MainActor
private final class Tutor: ObservableObject {
    /// The lessons, then adding 輕注音 in System Settings, then typing with it.
    static let stepCount = Lesson.tutorial.count + 2

    @Published private(set) var step = 0
    /// +1 going forward, -1 going back (for the transition).
    @Published private(set) var direction = 1
    /// Bumped after every key, to redraw the practice.
    @Published private(set) var revision = 0
    /// Typed 你好嗎 with 輕注音 on the try-out page.
    @Published var triedOut = false
    private(set) var practice: Practice?
    private var engine: Engine?

    private static var lexiconPath: String {
        AppModel.bundledInputMethod.appendingPathComponent("Contents/Resources/lexicon.bin").path
    }

    init() {
        start()
    }

    var isAddStep: Bool { step == Lesson.tutorial.count }
    var isTryStep: Bool { step == Lesson.tutorial.count + 1 }

    func go(to newStep: Int) {
        guard (0..<Tutor.stepCount).contains(newStep), newStep != step else { return }
        withAnimation(.spring(response: 0.55, dampingFraction: 0.88)) {
            direction = newStep > step ? 1 : -1
            step = newStep
            start()
        }
    }

    private func start() {
        guard step < Lesson.tutorial.count else {
            practice = nil
            return
        }
        let lesson = Lesson.tutorial[step]
        let isLast = step == Lesson.tutorial.count - 1
        // Each lesson starts from a fresh memory, except the last, which
        // shows what the one before taught.
        if !isLast || engine == nil {
            engine = Engine(lexiconPath: Tutor.lexiconPath, historyPath: nil)
        }
        guard let engine else {
            // No lexicon to practise with: on to adding 輕注音.
            step = Lesson.tutorial.count
            practice = nil
            return
        }
        if isLast, engine.query(lesson.symbols).first?.text != lesson.target {
            engine.learn(lesson.symbols, text: lesson.target)  // the lesson before was skipped
        }
        practice = Practice(lesson, engine: engine)
    }

    func handle(_ event: NSEvent) {
        guard let practice else { return }
        let key = Key(event: event, composing: !practice.composer.isEmpty)
        if practice.isDone {
            if key == .commit { go(to: step + 1) }
            return
        }
        withAnimation(.spring(response: 0.32, dampingFraction: 0.82)) {
            practice.handle(key)
            revision += 1
        }
        if practice.wentWrong {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.1) { [weak self, weak practice] in
                guard let self, let practice, practice === self.practice, practice.wentWrong else { return }
                withAnimation(.easeOut(duration: 0.25)) {
                    practice.restart()
                    self.revision += 1
                }
            }
        }
    }
}

// MARK: - Look

private enum Palette {
    static let cream = Color(red: 0.97, green: 0.91, blue: 0.74)
    static let sky = Color(red: 0.16, green: 0.52, blue: 0.80)
}

/// The sky over the hill, filling the window: cropped to keep the clouds,
/// the stars and the top of the hill in view.
private struct Backdrop: View {
    private static let image = Bundle.main.url(forResource: "tutorial-background", withExtension: "jpg")
        .flatMap(NSImage.init(contentsOf:))
    /// Where the window sits along the picture's height when it is cropped.
    private static let focus: CGFloat = 0.6

    var body: some View {
        GeometryReader { geometry in
            let size = geometry.size
            if let image = Backdrop.image, image.size.width > 0, image.size.height > 0 {
                let scale = max(size.width / image.size.width, size.height / image.size.height)
                let drawn = CGSize(width: image.size.width * scale, height: image.size.height * scale)
                Image(nsImage: image)
                    .resizable()
                    .interpolation(.high)
                    .frame(width: drawn.width, height: drawn.height)
                    .position(
                        x: size.width / 2,
                        y: size.height / 2 - (drawn.height - size.height) * (Backdrop.focus - 0.5))
            } else {
                Palette.sky
            }
        }
    }
}

// MARK: - Card

private struct Card: View {
    @ObservedObject var tutor: Tutor
    @EnvironmentObject private var model: AppModel
    var onFinish: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Header(tutor: tutor, onClose: onFinish)
            Group {
                if let practice = tutor.practice {
                    PracticeStep(step: tutor.step, practice: practice, revision: tutor.revision)
                } else if tutor.isAddStep {
                    AddStep()
                } else {
                    TryStep(tutor: tutor, onFinish: onFinish)
                }
            }
            .id(tutor.step)
            .transition(.asymmetric(
                insertion: .offset(x: 36 * CGFloat(tutor.direction)).combined(with: .opacity),
                removal: .offset(x: -24 * CGFloat(tutor.direction)).combined(with: .opacity)))
            .frame(maxHeight: .infinity, alignment: .top)

            ContinueButton(title: tutor.isTryStep ? "開始使用" : "繼續", ready: ready) {
                if tutor.isTryStep { onFinish() } else { tutor.go(to: tutor.step + 1) }
            }
            .disabled(tutor.practice == nil && !ready)
            if tutor.practice == nil {
                Button("稍後再設定", action: onFinish)
                    .buttonStyle(.plain)
                    .font(.system(size: 13))
                    .foregroundStyle(.white.opacity(0.6))
                    .frame(maxWidth: .infinity)
                    .padding(.top, 10)
            }
        }
        .padding(.horizontal, 30)
        .padding(.top, 26)
        .padding(.bottom, 30)
        .foregroundStyle(.white)
    }

    private var ready: Bool {
        if let practice = tutor.practice { return practice.isDone }
        if tutor.isAddStep { return model.status.isReady }
        return tutor.triedOut
    }
}

private struct Header: View {
    @ObservedObject var tutor: Tutor
    var onClose: () -> Void

    var body: some View {
        HStack(spacing: 18) {
            IconButton(symbol: "arrow.left") { tutor.go(to: tutor.step - 1) }
                .opacity(tutor.step == 0 ? 0 : 1)
                .disabled(tutor.step == 0)
            ProgressTrack(step: tutor.step + 1, count: Tutor.stepCount)
            IconButton(symbol: "xmark", action: onClose)
                .help("略過教學")
        }
        .frame(height: 30)
    }
}

private struct IconButton: View {
    let symbol: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 16, weight: .semibold))
                .frame(width: 30, height: 30)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

/// A thin track with the step count riding on it in a white pill.
private struct ProgressTrack: View {
    let step: Int
    let count: Int

    var body: some View {
        GeometryReader { geometry in
            let width = geometry.size.width
            let fraction = CGFloat(step) / CGFloat(count)
            let pill: CGFloat = 46
            let x = min(max(width * fraction, pill / 2), width - pill / 2)
            ZStack(alignment: .leading) {
                Capsule().fill(.white.opacity(0.16)).frame(height: 4)
                Capsule()
                    .fill(LinearGradient(
                        colors: [Palette.cream.opacity(0.35), Palette.cream],
                        startPoint: .leading, endPoint: .trailing))
                    .frame(width: x, height: 4)
                Text("\(step)/\(count)")
                    .font(.system(size: 12, weight: .semibold).monospacedDigit())
                    .foregroundStyle(.black)
                    .frame(width: pill, height: 24)
                    .background(Capsule().fill(.white))
                    .shadow(color: .black.opacity(0.18), radius: 6, y: 2)
                    .position(x: x, y: geometry.size.height / 2)
            }
            .frame(maxHeight: .infinity)
        }
    }
}

private struct Title: View {
    let text: String
    let chip: String

    var body: some View {
        Text(text)
            .font(.system(size: 31, weight: .bold))
            .lineSpacing(4)
            .foregroundStyle(Palette.cream)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            .overlay(alignment: .topTrailing) {
                GlassChip(text: chip)
                    .rotationEffect(.degrees(-9))
                    .offset(x: -6, y: -6)
            }
    }
}

/// A tilted glass key floating by the title.
private struct GlassChip: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 17, weight: .semibold))
            .foregroundStyle(.white)
            .padding(.horizontal, 16)
            .padding(.vertical, 9)
            .background {
                Capsule()
                    .fill(LinearGradient(
                        colors: [.white.opacity(0.32), .white.opacity(0.06)],
                        startPoint: .top, endPoint: .bottom))
                    .overlay(Capsule().strokeBorder(
                        LinearGradient(
                            colors: [.white.opacity(0.7), .white.opacity(0.1)],
                            startPoint: .topLeading, endPoint: .bottomTrailing),
                        lineWidth: 1))
            }
            .shadow(color: .black.opacity(0.25), radius: 12, y: 8)
    }
}

private struct Subtitle: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 14.5))
            .foregroundStyle(.white.opacity(0.68))
            .fixedSize(horizontal: false, vertical: true)
    }
}

private struct ContinueButton: View {
    let title: String
    let ready: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(ready ? .black : .white.opacity(0.55))
                .frame(maxWidth: .infinity)
                .frame(height: 54)
                .background {
                    Capsule().fill(ready ? .white : .white.opacity(0.08))
                }
                .overlay {
                    Capsule().strokeBorder(.white.opacity(ready ? 0 : 0.14), lineWidth: 1)
                }
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .animation(.easeOut(duration: 0.3), value: ready)
    }
}

// MARK: - Lessons

private struct PracticeStep: View {
    let step: Int
    let practice: Practice
    /// Changes after every key, so the view redraws.
    let revision: Int

    private static let copy: [(title: String, chip: String, subtitle: String)] = [
        ("每個字\n只打第一個注音", "ㄋㄏㄇ", "打 ㄋㄏㄇ，按 Enter 送出「你好嗎」。"),
        ("按 Tab 或 → 選字", "Tab / →", "打 ㄇㄊ，按 Tab 或 → 選「明天」。Shift + Tab 或 ← 往回。"),
        ("找不到？\n按 ] 翻頁", "[ ]", "打 ㄏㄏ，到下一頁找「還好」。[ 回上一頁。"),
        ("整句不對？\n分段選", "分段", "要打「媽媽謝謝你」。打 ㄇㄇㄒㄒㄋ，第一個候選是「密碼謝謝你」，不是要的：先選「媽媽」，再選「謝謝你」。"),
        ("輕注音\n記住你的選擇", "記住了", "再打一次 ㄇㄇㄒㄒㄋ，「媽媽謝謝你」已經排在第一個。"),
    ]

    var body: some View {
        let copy = PracticeStep.copy[step]
        VStack(alignment: .leading, spacing: 0) {
            Title(text: copy.title, chip: copy.chip)
                .padding(.top, 34)
            Subtitle(text: copy.subtitle)
                .padding(.top, 12)
            Goal(lesson: practice.lesson)
                .padding(.top, 22)
            Field(practice: practice, revision: revision)
                .padding(.top, 10)
            CandidateStrip(practice: practice, revision: revision)
                .padding(.top, 12)
            KeyGuide(practice: practice, revision: revision)
                .padding(.top, 22)
        }
    }
}

/// What the lesson asks for, shown before anything is typed: the text, in
/// the pieces it is chosen in when there are several.
private struct Goal: View {
    let lesson: Lesson

    var body: some View {
        HStack(spacing: 10) {
            Text("要打出")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.white.opacity(0.55))
            Text(lesson.pieces.joined(separator: " ＋ "))
                .font(.system(size: 19, weight: .semibold))
                .foregroundStyle(Palette.cream)
            if lesson.pieces.count > 1 {
                Text("分 \(lesson.pieces.count) 段選")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.white.opacity(0.7))
                    .padding(.horizontal, 9)
                    .padding(.vertical, 3)
                    .background(Capsule().fill(.white.opacity(0.12)))
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 4)
    }
}

/// What has been typed: committed text, then the bopomofo being composed.
private struct Field: View {
    let practice: Practice
    let revision: Int

    var body: some View {
        HStack(spacing: 0) {
            Text(practice.committed)
                .foregroundStyle(.white)
            Text(practice.composer.composingText)
                .foregroundStyle(Palette.cream)
                .underline(color: Palette.cream.opacity(0.6))
            if !practice.isDone {
                Caret().padding(.leading, 2)
            }
            Spacer(minLength: 0)
            if practice.isDone {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 22))
                    .foregroundStyle(Palette.cream)
                    .transition(.scale.combined(with: .opacity))
            }
        }
        .font(.system(size: 28, weight: .medium))
        .lineLimit(1)
        .padding(.horizontal, 20)
        .frame(height: 74)
        .background {
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .fill(.white.opacity(practice.isDone ? 0.1 : 0.06))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .strokeBorder(
                    practice.isDone ? Palette.cream.opacity(0.55) : .white.opacity(0.12),
                    lineWidth: 1)
        }
        .modifier(Shake(amount: practice.wentWrong ? 1 : 0))
    }
}

private struct Caret: View {
    var body: some View {
        TimelineView(.periodic(from: .now, by: 0.55)) { context in
            let on = Int(context.date.timeIntervalSinceReferenceDate / 0.55) % 2 == 0
            RoundedRectangle(cornerRadius: 1)
                .fill(Palette.cream)
                .frame(width: 2, height: 30)
                .opacity(on ? 1 : 0.15)
        }
    }
}

private struct Shake: GeometryEffect {
    var amount: CGFloat
    var animatableData: CGFloat {
        get { amount }
        set { amount = newValue }
    }

    func effectValue(size: CGSize) -> ProjectionTransform {
        ProjectionTransform(CGAffineTransform(translationX: 8 * sin(amount * .pi * 4), y: 0))
    }
}

/// The current page of candidates, as in the real candidate window.
private struct CandidateStrip: View {
    let practice: Practice
    let revision: Int
    @Namespace private var selection

    var body: some View {
        let composer = practice.composer
        let size = Composer.pageSize
        let page = composer.selected / size
        let start = page * size
        let end = min(start + size, composer.candidates.count)
        let pages = (composer.candidates.count + size - 1) / size
        HStack(spacing: 8) {
            ScrollViewReader { proxy in
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 2) {
                        ForEach(start..<end, id: \.self) { i in
                            cell(i - start + 1, composer.candidates[i].text, selected: i == composer.selected)
                                .id(i)
                        }
                    }
                    .padding(.horizontal, 4)
                }
                .mask(LinearGradient(
                    stops: [
                        .init(color: .black, location: 0),
                        .init(color: .black, location: 0.86),
                        .init(color: .clear, location: 1),
                    ],
                    startPoint: .leading, endPoint: .trailing))
                .onChange(of: composer.selected) { selected in
                    withAnimation(.easeOut(duration: 0.25)) { proxy.scrollTo(selected, anchor: .center) }
                }
            }
            if pages > 1 {
                Text("\(page + 1)/\(pages)")
                    .font(.system(size: 11, weight: .medium).monospacedDigit())
                    .foregroundStyle(.white.opacity(0.5))
                    .fixedSize()
            }
        }
        .frame(height: 42)
        .opacity(composer.isEmpty ? 0 : 1)
    }

    private func cell(_ number: Int, _ text: String, selected: Bool) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 5) {
            Text("\(number)")
                .font(.system(size: 11, weight: .semibold).monospacedDigit())
                .opacity(0.5)
            Text(text)
                .font(.system(size: 17, weight: .medium))
                .fixedSize()
        }
        .foregroundStyle(selected ? .black : .white)
        .padding(.horizontal, 12)
        .frame(height: 38)
        .background {
            if selected {
                Capsule()
                    .fill(.white)
                    .matchedGeometryEffect(id: "selected", in: selection)
            }
        }
    }
}

/// The keys of the lesson; the next one lights up.
private struct KeyGuide: View {
    let practice: Practice
    let revision: Int

    var body: some View {
        let progress = practice.progress
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 6) {
                ForEach(KeyGuide.runs(practice.plan), id: \.start) { run in
                    let end = run.start + run.count
                    let state: Keycap.State =
                        progress.map { end <= $0 ? .done : run.start <= $0 ? .next : .waiting } ?? .waiting
                    let left = end - max(progress ?? run.start, run.start)
                    Keycap(key: run.key, state: state, count: state == .done ? run.count : left)
                }
            }
            Group {
                if practice.isDone {
                    Text("完成。按 Enter 繼續")
                } else if practice.wentWrong {
                    Text("不是「\(practice.lesson.target)」，再試一次")
                } else if progress == nil {
                    HStack(spacing: 8) {
                        Keycap(key: .escape, state: .next)
                        Text("重來")
                    }
                } else {
                    Text(" ")
                }
            }
            .font(.system(size: 13.5, weight: .medium))
            .foregroundStyle(.white.opacity(0.62))
            .frame(height: 30, alignment: .leading)
        }
    }

    /// The plan with repeats of a key (Tab Tab Tab) folded into one.
    static func runs(_ plan: [Key]) -> [(key: Key, start: Int, count: Int)] {
        var runs: [(key: Key, start: Int, count: Int)] = []
        for (i, key) in plan.enumerated() {
            if case .symbol = key {
                runs.append((key, i, 1))
            } else if let last = runs.last, last.key == key {
                runs[runs.count - 1].count += 1
            } else {
                runs.append((key, i, 1))
            }
        }
        return runs
    }
}

private struct Keycap: View {
    enum State { case done, next, waiting }

    let key: Key
    let state: State
    var count = 1

    var body: some View {
        let (label, hint) = Keycap.labels(key)
        VStack(spacing: 1) {
            HStack(alignment: .firstTextBaseline, spacing: 2) {
                Text(label).font(.system(size: 16, weight: .semibold))
                if count > 1 {
                    Text("×\(count)").font(.system(size: 11, weight: .bold).monospacedDigit())
                }
            }
            if let hint {
                Text(hint).font(.system(size: 9.5, weight: .semibold)).opacity(0.55)
            }
        }
        .foregroundStyle(state == .next ? .black : .white)
        .opacity(state == .done ? 0.35 : 1)
        .frame(minWidth: 36)
        .padding(.horizontal, 3)
        .frame(height: 46)
        .background {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(state == .next ? .white : .white.opacity(state == .done ? 0.03 : 0.09))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(.white.opacity(state == .waiting ? 0.16 : 0), lineWidth: 1)
        }
        .shadow(color: .white.opacity(state == .next ? 0.35 : 0), radius: 12)
        .scaleEffect(state == .next ? 1.06 : 1)
    }

    private static func labels(_ key: Key) -> (String, String?) {
        switch key {
        case .symbol(let s): return (String(s), Engine.key(forBopomofo: s).map { String($0).uppercased() })
        case .next: return ("Tab", "或 →")
        case .previous: return ("⇧Tab", "或 ←")
        case .pageNext: return ("]", nil)
        case .pagePrevious: return ("[", nil)
        case .commit: return ("⏎", "Enter")
        case .escape: return ("esc", nil)
        default: return ("", nil)
        }
    }
}

// MARK: - Switching to 輕注音

/// Adding 輕注音 in System Settings: every step with its screenshot at once;
/// ticks as soon as it is there.
private struct AddStep: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Title(text: "加入輕注音", chip: "設定")
                .padding(.top, 34)
            VStack(alignment: .leading, spacing: 14) {
                SetupStep(number: 1, text: "打開「系統設定」→「鍵盤」，按「輸入方式」旁的「編輯…」")
                HStack(alignment: .top, spacing: 14) {
                    SetupStep(number: 2, text: "在跳出的視窗，\n按左下角「＋」")
                    SetupStep(number: 3, text: "左邊選「繁體中文」，右邊選「輕注音」，按「加入」")
                }
            }
            .padding(.top, 16)
            HStack(spacing: 8) {
                Image(systemName: model.status.isReady ? "checkmark.circle.fill" : "circle.dotted")
                Text(model.status.isReady ? "已加入" : "還沒加入")
                Spacer()
                Button("打開設定") { model.openInputSourceSettings() }
                    .buttonStyle(.plain)
                    .font(.system(size: 13.5, weight: .semibold))
                    .padding(.horizontal, 14)
                    .frame(height: 30)
                    .background(Capsule().fill(.white.opacity(0.14)))
            }
            .font(.system(size: 13.5, weight: .semibold))
            .foregroundStyle(model.status.isReady ? Palette.cream : .white.opacity(0.75))
            .padding(.top, 14)
        }
        // Look again every second until it is added.
        .task {
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 1_000_000_000)
                model.refreshStatus()
            }
        }
    }
}

/// One step of adding 輕注音: number and words, its screenshot (setup-N.png)
/// below; a click shows the screenshot large.
private struct SetupStep: View {
    let number: Int
    let text: String
    @State private var enlarged = false

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text("\(number)")
                    .font(.system(size: 12, weight: .bold).monospacedDigit())
                    .foregroundStyle(.black)
                    .frame(width: 20, height: 20)
                    .background(Circle().fill(Palette.cream))
                Text(text)
                    .font(.system(size: 13.5))
                    .foregroundStyle(.white.opacity(0.95))
                    .fixedSize(horizontal: false, vertical: true)
            }
            if let url = Bundle.main.url(forResource: "setup-\(number)", withExtension: "png"),
               let image = NSImage(contentsOf: url)
            {
                Button { enlarged = true } label: {
                    Image(nsImage: image)
                        .resizable()
                        .scaledToFit()
                        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                }
                .buttonStyle(.plain)
                .help("點一下放大")
                .onHover { inside in
                    if inside { NSCursor.pointingHand.push() } else { NSCursor.pop() }
                }
                .popover(isPresented: $enlarged) {
                    // Screenshots are @2x: half their pixels, at most 640 pt wide.
                    Image(nsImage: image)
                        .resizable()
                        .scaledToFit()
                        .frame(width: min(image.size.width / 2, 640))
                        .padding(10)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Typing 你好嗎 with 輕注音 in a real text field: proof it can be switched to
/// and works. Then 開始使用.
private struct TryStep: View {
    @ObservedObject var tutor: Tutor
    var onFinish: () -> Void
    @EnvironmentObject private var model: AppModel
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Title(text: "試打一次", chip: "ㄋㄏㄇ")
                .padding(.top, 34)
            Subtitle(text: "在選單列切到「輕注音」（或按 Control + 空白鍵），打 ㄋㄏㄇ 送出「你好嗎」。")
                .padding(.top, 12)
            HStack {
                TextField("", text: $text)
                    .textFieldStyle(.plain)
                    .font(.system(size: 28, weight: .medium))
                    .focused($focused)
                    .onSubmit { if tutor.triedOut { onFinish() } }
                if tutor.triedOut {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 22))
                        .foregroundStyle(Palette.cream)
                }
            }
            .padding(.horizontal, 20)
            .frame(height: 74)
            .background(RoundedRectangle(cornerRadius: 22, style: .continuous).fill(.white.opacity(0.06)))
            .overlay {
                RoundedRectangle(cornerRadius: 22, style: .continuous)
                    .strokeBorder(tutor.triedOut ? Palette.cream.opacity(0.55) : .white.opacity(0.12), lineWidth: 1)
            }
            .padding(.top, 24)
            Subtitle(text: hint)
                .padding(.top, 12)
            ShareWordsBox()
                .padding(.top, 22)
        }
        .onAppear { focused = true }
        .onChange(of: text) { new in
            if new.contains("你好嗎"), model.inputMethodSelected {
                withAnimation { tutor.triedOut = true }
            }
        }
    }

    private var hint: String {
        if tutor.triedOut { return "成功了，可以開始用輕注音打字。" }
        if !text.isEmpty, text.unicodeScalars.contains(where: { $0.isASCII }) {
            return "打出來是英文：先切到「輕注音」再打。"
        }
        return "選單裡找不到輕注音？到設定刪掉再重新加入。"
    }
}

/// 分享新詞, checked to begin with: 開始使用 agrees, unchecking declines.
private struct ShareWordsBox: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Button { model.shareWords.toggle() } label: {
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Image(systemName: model.shareWords ? "checkmark.square.fill" : "square")
                        .font(.system(size: 16, weight: .semibold))
                        .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 3 }
                    Text("分享新詞：把你選過、詞庫沒有的詞傳給我們，審核後加進詞庫，讓大家更好打。")
                        .font(.system(size: 13.5))
                        .multilineTextAlignment(.leading)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityValue(model.shareWords ? "已勾選" : "未勾選")
            Link("隱私權政策", destination: WordSharing.privacyURL)
                .font(.system(size: 13.5, weight: .semibold))
                .foregroundStyle(.white)
                .underline()
        }
        .foregroundStyle(.white.opacity(0.9))
    }
}

/// A row like the input menu's.
private struct Option: View {
    let icon: String
    let title: String
    let selected: Bool

    var body: some View {
        HStack(spacing: 14) {
            Text(icon)
                .font(.system(size: 13, weight: .bold))
                .frame(width: 26, height: 22)
                .background(RoundedRectangle(cornerRadius: 5).strokeBorder(.white.opacity(0.7), lineWidth: 1.2))
            Text(title).font(.system(size: 15.5, weight: .medium))
            Spacer()
            if selected {
                Image(systemName: "checkmark").font(.system(size: 14, weight: .semibold))
            }
        }
        .padding(.horizontal, 20)
        .frame(height: 60)
        .background {
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(selected ? Color.black.opacity(0.72) : .white.opacity(0.06))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .strokeBorder(.white.opacity(selected ? 0 : 0.1), lineWidth: 1)
        }
    }
}

// MARK: - AppKit

/// Receives the keys while the tutorial is shown. A plain view is no text
/// input client, so input methods leave its keys alone.
private struct KeyCatcher: NSViewRepresentable {
    /// Whether it takes the keys (keeps the focus).
    var active = true
    let onKey: (NSEvent) -> Void

    final class View: NSView {
        var onKey: ((NSEvent) -> Void)?
        override var acceptsFirstResponder: Bool { true }
        override func keyDown(with event: NSEvent) { onKey?(event) }
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            DispatchQueue.main.async { [weak self] in
                guard let self, let window = self.window else { return }
                window.makeFirstResponder(self)
            }
        }
    }

    func makeNSView(context: Context) -> View {
        let view = View()
        view.onKey = onKey
        return view
    }

    func updateNSView(_ view: View, context: Context) {
        view.onKey = onKey
        if active, let window = view.window, window.firstResponder !== view {
            window.makeFirstResponder(view)
        }
    }
}

/// Lets the backdrop run under the title bar while the tutorial is shown.
private struct TransparentTitleBar: NSViewRepresentable {
    final class View: NSView {
        private weak var styled: NSWindow?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let window {
                styled = window
                window.titlebarAppearsTransparent = true
                window.titleVisibility = .hidden
                window.styleMask.insert(.fullSizeContentView)
                window.isMovableByWindowBackground = true
            } else {
                restore()
            }
        }

        func restore() {
            guard let window = styled else { return }
            window.titlebarAppearsTransparent = false
            window.titleVisibility = .visible
            window.styleMask.remove(.fullSizeContentView)
            window.isMovableByWindowBackground = false
            styled = nil
        }
    }

    func makeNSView(context: Context) -> View { View() }
    func updateNSView(_ view: View, context: Context) {}
    static func dismantleNSView(_ view: View, coordinator: ()) { view.restore() }
}
