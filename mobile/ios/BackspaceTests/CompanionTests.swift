import XCTest
@testable import Backspace

final class CompanionTests: XCTestCase {
    /// The same link core's `pair_link_escapes` test builds.
    func testPairLinkParses() throws {
        let p = try XCTUnwrap(Pairing(link: "backspace://pair?v=1&url=http%3A%2F%2F192.168.1.4%3A7421&token=a%20b%26c&name=Sam%27s%20Mac"))
        XCTAssertEqual(p.url.absoluteString, "http://192.168.1.4:7421")
        XCTAssertEqual(p.token, "a b&c")
        XCTAssertEqual(p.name, "Sam's Mac")
    }

    func testPairLinkRejectsOthers() {
        XCTAssertNil(Pairing(link: "https://pair?url=http%3A%2F%2Fx&token=t"))
        XCTAssertNil(Pairing(link: "backspace://pair?v=1&url=http%3A%2F%2F192.168.1.4%3A7421"))
        XCTAssertNil(Pairing(link: "backspace://pair?v=1&url=file%3A%2F%2F%2Fetc&token=t"))
    }

    func testDecodesThread() throws {
        let json = """
        {"id":"t1","title":"Rust","created":1,"updated":2,"pinned":false,
         "route":{"kind":"local","provider":"ollama","model":null},
         "messages":[
          {"id":"m1","role":"user","text":"hi","at":1,"status":"done","reply_to":null,"reactions":["👍"],"attachments":[]},
          {"id":"m2","role":"assistant","text":"","at":2,"status":"streaming","author_name":"Claude"}
         ],"life":"keep"}
        """
        let t = try Companion.decoder.decode(ChatThread.self, from: Data(json.utf8))
        XCTAssertEqual(t.messages.count, 2)
        XCTAssertEqual(t.messages[0].reactions, ["👍"])
        XCTAssertEqual(t.messages[1].authorName, "Claude")
        XCTAssertTrue(t.isBusy)
    }

    func testDecodesProject() throws {
        let json = """
        {"open":true,"workspace":"/w","state":{"name":"W","workspace":"/w","agents":[],"total_cost_usd":0,
         "approvals":[
          {"id":0,"kind":"plan","agent":0,"deliverable":{"summary":"s","files":[]},"tickets":["A-1"],"state":{"state":"pending"}},
          {"id":1,"kind":"deliverable","agent":2,"deliverable":{"summary":"d","files":["a.rs"],"diff_stat":"1 file"},"tickets":["A-1"],"state":{"state":"rejected","feedback":"no"}}
         ],
         "tickets":[{"num":1,"key":"A-1","title":"Do it","category":"feature","state":"in_progress","what_to_build":"x","acceptance":[],"assignee":2}]}}
        """
        let p = try Companion.decoder.decode(ProjectInfo.self, from: Data(json.utf8))
        let s = try XCTUnwrap(p.state)
        XCTAssertTrue(s.approvals[0].isPending)
        XCTAssertEqual(s.approvals[1].state.feedback, "no")
        XCTAssertEqual(s.approvals[1].deliverable.diffStat, "1 file")
        XCTAssertEqual(s.tickets[0].stateLabel, "In Progress")
    }

    func testAgentHarnessChatsOverA2A() {
        let r = RouteOption(id: "remote-1", name: "Remote", kind: "agent", models: [])
        XCTAssertEqual(r.route(model: nil), Route(kind: "a2a", provider: "remote-1", model: nil))
        XCTAssertEqual(RouteOption(id: "ollama", name: "Ollama", kind: "local", models: []).route(model: "qwen3:8b").kind, "local")
    }

    func testAge() {
        let now = Date(timeIntervalSince1970: 100_000)
        XCTAssertEqual(age(100_000_000, now: now), "now")
        XCTAssertEqual(age((100_000 - 19 * 60) * 1000, now: now), "19m")
        XCTAssertEqual(age((100_000 - 3 * 86400) * 1000, now: now), "3d")
    }
}
