// Lists the words in data/tsi.csv with a character macOS cannot draw (no
// system font has it, so the candidate window would show a box), as
// lexicon-adjustments.csv removal lines. Rerun when the lexicon changes:
//   swift scripts/find-undrawable.swift [data/tsi.csv]
import AppKit

let path = CommandLine.arguments.dropFirst().first ?? "data/tsi.csv"
guard let source = try? String(contentsOfFile: path, encoding: .utf8) else {
    FileHandle.standardError.write("cannot read \(path)\n".data(using: .utf8)!)
    exit(1)
}

// As the candidate window draws them: the system font, falling back to
// whichever font the system picks for the character.
let font = NSFont.systemFont(ofSize: 18) as CTFont
var drawable: [Character: Bool] = [:]
func canDraw(_ c: Character) -> Bool {
    if let known = drawable[c] { return known }
    let units = Array(String(c).utf16)
    let fallback = CTFontCreateForString(font, String(c) as CFString, CFRange(location: 0, length: units.count))
    var glyphs = [CGGlyph](repeating: 0, count: units.count)
    let ok = CTFontGetGlyphsForCharacters(fallback, units, &glyphs, units.count)
        && CTFontCopyPostScriptName(fallback) as String != "LastResort"
    drawable[c] = ok
    return ok
}

var words = Set<String>()
for line in source.split(separator: "\n") where !line.hasPrefix("#") {
    guard let text = line.split(separator: ",").first else { continue }
    if !text.allSatisfy(canDraw) { words.insert(String(text)) }
}
let missing = drawable.filter { !$0.value }.count
FileHandle.standardError.write("\(drawable.count) characters, \(missing) undrawable, \(words.count) words\n".data(using: .utf8)!)
for word in words.sorted() { print("-\(word)") }
