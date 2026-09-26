// Injection bench: replays TTP's text-injection strategies into the focused
// field of a target app and reads back, over Accessibility, what landed.
//
//   swiftc -O scripts/inject-bench/bench.swift -o /tmp/inject-bench
//   /tmp/inject-bench probe com.anthropic.claudefordesktop
//   /tmp/inject-bench run   com.anthropic.claudefordesktop 6 [ttp,clip,...]
//
//   REFOCUS_MS=0  bounce focus through Finder, return, wait N ms, then type
//   HOLD_MS=800   leave a key-down without key-up before typing
//
// It takes the keyboard: the target comes to the front and text is typed into
// its focused field, then selected over Accessibility and deleted. Nothing is
// sent. Never uses a ⌘-shortcut to select: keycode 0 is Q on AZERTY, and ⌘ +
// keycode 0 quit the Claude app on the first attempt (2026-09-26).
//
// Findings on 2026-09-26, Claude desktop composer, 128 injections: every
// strategy lands intact when the field has focus; typed within 0 ms of the
// app regaining focus, the first two 16-char chunks are lost and ⌘V loses
// everything; at 100 ms nothing is lost. The 0024-8368 pattern (last char of
// each chunk) did not reproduce.
import AppKit
import ApplicationServices
import Foundation

setvbuf(stdout, nil, _IONBF, 0)

let texts = [
    "Je vais te demander des promptes, donc des choses à copier et des choses à coller. D'accord. Ne les surcomplique pas, juste donne l'idée globale, simplifie.",
    "Bon, alors là on a un vrai problème de fiabilité : quand je dicte une phrase un peu longue dans l'application, il arrive que seules quelques lettres éparpillées apparaissent au lieu du texte complet, et je ne m'en rends compte qu'après avoir envoyé le message. Il faut que ce soit réglé proprement.",
]

struct Variant {
    let name: String
    let chunk: Int        // UTF-16 units per event; 0 = clipboard ⌘V
    let keyUp: Bool
    let gapMs: Double
    let tap: CGEventTapLocation
}

let variants: [String: Variant] = [
    "ttp":        Variant(name: "ttp",        chunk: 16, keyUp: false, gapMs: 0, tap: .cghidEventTap),
    "ttp_up":     Variant(name: "ttp_up",     chunk: 16, keyUp: true,  gapMs: 0, tap: .cghidEventTap),
    "ttp_gap4":   Variant(name: "ttp_gap4",   chunk: 16, keyUp: false, gapMs: 4, tap: .cghidEventTap),
    "up_gap4":    Variant(name: "up_gap4",    chunk: 16, keyUp: true,  gapMs: 4, tap: .cghidEventTap),
    "c8_up_gap4": Variant(name: "c8_up_gap4", chunk: 8,  keyUp: true,  gapMs: 4, tap: .cghidEventTap),
    "c1_up":      Variant(name: "c1_up",      chunk: 1,  keyUp: true,  gapMs: 1, tap: .cghidEventTap),
    "session":    Variant(name: "session",    chunk: 16, keyUp: false, gapMs: 0, tap: .cgAnnotatedSessionEventTap),
    "clip":       Variant(name: "clip",       chunk: 0,  keyUp: true,  gapMs: 0, tap: .cgAnnotatedSessionEventTap),
]

// ── AX helpers ────────────────────────────────────────────────────────────
func wakeElectron(pid: pid_t) {
    AXUIElementSetAttributeValue(AXUIElementCreateApplication(pid), "AXManualAccessibility" as CFString, kCFBooleanTrue)
}

func focusedElement(pid: pid_t) -> AXUIElement? {
    let app = AXUIElementCreateApplication(pid)
    var v: CFTypeRef?
    guard AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute as CFString, &v) == .success else { return nil }
    return (v as! AXUIElement)
}

func readText(pid: pid_t) -> String? {
    guard let el = focusedElement(pid: pid) else { return nil }
    var v: CFTypeRef?
    if AXUIElementCopyAttributeValue(el, kAXValueAttribute as CFString, &v) == .success, let s = v as? String { return s }
    return nil
}

func describe(pid: pid_t) -> String {
    guard let el = focusedElement(pid: pid) else { return "no focused element" }
    var role: CFTypeRef?; var sub: CFTypeRef?; var desc: CFTypeRef?
    AXUIElementCopyAttributeValue(el, kAXRoleAttribute as CFString, &role)
    AXUIElementCopyAttributeValue(el, kAXSubroleAttribute as CFString, &sub)
    AXUIElementCopyAttributeValue(el, kAXDescriptionAttribute as CFString, &desc)
    let t = readText(pid: pid)
    return "role=\(role as? String ?? "-") sub=\(sub as? String ?? "-") desc=\(desc as? String ?? "-") value_chars=\(t.map { String($0.count) } ?? "unreadable")"
}

// ── Injection ─────────────────────────────────────────────────────────────
let source = CGEventSource(stateID: .privateState)!

func sleepMs(_ ms: Double) { if ms > 0 { usleep(useconds_t(ms * 1000)) } }

func chunks(_ text: String, _ max: Int) -> [String] {
    var out: [String] = []; var cur = ""; var units = 0
    for ch in text {
        let w = String(ch).utf16.count
        if units + w > max && !cur.isEmpty { out.append(cur); cur = ""; units = 0 }
        cur.append(ch); units += w
    }
    if !cur.isEmpty { out.append(cur) }
    return out
}

func postKey(_ code: CGKeyCode, flags: CGEventFlags, tap: CGEventTapLocation) {
    let d = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: true)!
    d.flags = flags; d.post(tap: tap)
    usleep(8000)
    let u = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: false)!
    u.flags = flags; u.post(tap: tap)
}

func type(_ text: String, _ v: Variant) {
    for c in chunks(text, v.chunk) {
        let units = Array(c.utf16)
        let d = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: true)!
        d.flags = []
        d.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
        d.post(tap: v.tap)
        if v.keyUp {
            let u = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: false)!
            u.flags = []
            u.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
            u.post(tap: v.tap)
        }
        sleepMs(v.gapMs)
    }
}

func paste(_ text: String) {
    let pb = NSPasteboard.general
    let saved = pb.string(forType: .string)
    pb.clearContents(); pb.setString(text, forType: .string)
    postKey(9, flags: .maskCommand, tap: .cgAnnotatedSessionEventTap)
    usleep(600_000)
    pb.clearContents(); if let s = saved { pb.setString(s, forType: .string) }
}

/// Select the whole field through Accessibility, then Delete (keycode 51 is
/// the same key on every layout). Never a ⌘-shortcut: keycode 0 is Q on AZERTY,
/// and ⌘+keycode0 quit the Claude app on the first attempt.
func clearField(pid: pid_t) {
    guard let el = focusedElement(pid: pid), let t = readText(pid: pid), !t.isEmpty else { return }
    var range = CFRange(location: 0, length: (t as NSString).length)
    let v = AXValueCreate(.cfRange, &range)!
    AXUIElementSetAttributeValue(el, kAXSelectedTextRangeAttribute as CFString, v)
    usleep(60_000)
    postKey(51, flags: [], tap: .cgAnnotatedSessionEventTap)
    usleep(150_000)
}

/// Poll until the field stops changing for 400 ms (max 3 s).
func settledText(pid: pid_t) -> (String?, Int) {
    var last = readText(pid: pid); var stableSince = Date(); let start = Date()
    while Date().timeIntervalSince(start) < 3 {
        usleep(20_000)
        let t = readText(pid: pid)
        if t != last { last = t; stableSince = Date() }
        else if Date().timeIntervalSince(stableSince) > 0.4 { break }
    }
    return (last, Int(Date().timeIntervalSince(start) * 1000))
}

// ── Main ──────────────────────────────────────────────────────────────────
let args = CommandLine.arguments
guard args.count >= 3, let target = NSRunningApplication.runningApplications(withBundleIdentifier: args[2]).first else {
    print("usage: bench.swift probe|run <bundle-id> [reps] [variants]"); exit(2)
}
let pid = target.processIdentifier
wakeElectron(pid: pid)
usleep(300_000)

if args[1] == "probe" { print(describe(pid: pid)); exit(0) }

let reps = args.count > 3 ? Int(args[3]) ?? 5 : 5
let chosen = args.count > 4 ? args[4].split(separator: ",").map(String.init) : Array(variants.keys).sorted()
let previous = NSWorkspace.shared.frontmostApplication
let refocusMs: Int? = ProcessInfo.processInfo.environment["REFOCUS_MS"].flatMap { Int($0) }
let bounce = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.finder").first

target.activate()
usleep(700_000)
print("# target:", describe(pid: pid))
guard readText(pid: pid) != nil else { print("focused element is not readable — aborting"); exit(1) }
clearField(pid: pid)

var tally: [String: (n: Int, exact: Int, lost: Int)] = [:]
for rep in 0..<reps {
    for name in chosen {
        guard let v = variants[name] else { continue }
        guard refocusMs != nil || NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
            print("# focus left the target — stopping"); exit(1)
        }
        let text = texts[rep % texts.count]
        if let ms = refocusMs, let other = bounce {
            other.activate(); usleep(400_000)
            target.activate(); usleep(useconds_t(ms * 1000))
        }
        if let hold = ProcessInfo.processInfo.environment["HOLD_MS"].flatMap({ Double($0) }) {
            // A keyDown with no keyUp, like every TTP chunk: macOS now believes
            // the key is held. Press-and-hold kicks in after its delay.
            let d = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: true)!
            d.flags = []; let u: [UniChar] = Array("x".utf16)
            d.keyboardSetUnicodeString(stringLength: 1, unicodeString: u); d.post(tap: .cghidEventTap)
            sleepMs(hold)
        }
        let before = readText(pid: pid) ?? ""
        let t0 = Date()
        if v.chunk == 0 { paste(text) } else { type(text, v) }
        let injectMs = Int(Date().timeIntervalSince(t0) * 1000)
        let (after, waitMs) = settledText(pid: pid)
        let a = Array(after ?? ""), b = Array(before)
        var pre = 0; while pre < a.count && pre < b.count && a[pre] == b[pre] { pre += 1 }
        var suf = 0; while suf < a.count - pre && suf < b.count - pre && a[a.count-1-suf] == b[b.count-1-suf] { suf += 1 }
        let landed = String(a[pre..<(a.count - suf)])
        let exact = landed == text
        var t = tally[name] ?? (0, 0, 0)
        t.n += 1; if exact { t.exact += 1 } else { t.lost += text.count - landed.count }
        tally[name] = t
        let rec: [String: Any] = ["rep": rep, "variant": name, "expected": text.count, "landed": landed.count,
                                  "exact": exact, "inject_ms": injectMs, "settle_ms": waitMs,
                                  "got": exact ? "" : landed]
        let data = try! JSONSerialization.data(withJSONObject: rec, options: [.sortedKeys])
        print(String(data: data, encoding: .utf8)!)
        clearField(pid: pid)
        if let left = readText(pid: pid), left.count > 40 {
            print("# field did not clear (\(left.count) chars left) — stopping"); previous?.activate(); exit(1)
        }
    }
}

print("\n# variant      runs  exact  chars_lost")
for name in chosen { if let t = tally[name] { print(String(format: "  %-12@ %4d  %5d  %10d", name as NSString, t.n, t.exact, t.lost)) } }
previous?.activate()
