#if os(Linux)
import Foundation

enum GeckoCookieImporter {
    enum ImportError: LocalizedError {
        case cookieDBNotFound(path: String, browser: Browser)
        case cookieDBNotReadable(path: String, browser: Browser)
        case sqliteFailed(message: String, browser: Browser)

        var errorDescription: String? {
            switch self {
            case let .cookieDBNotFound(path, browser): "\(browser.displayName) cookie DB not found at \(path)."
            case let .cookieDBNotReadable(path, browser): "\(browser.displayName) cookie DB exists but is not readable (\(path))."
            case let .sqliteFailed(message, browser): "Failed to read \(browser.displayName) cookies: \(message)"
            }
        }

        func browserCookieError(browser: Browser) -> BrowserCookieError {
            switch self {
            case .cookieDBNotFound:
                .notFound(browser: browser, details: self.localizedDescription)
            case .cookieDBNotReadable:
                .accessDenied(browser: browser, details: self.localizedDescription)
            case .sqliteFailed:
                .loadFailed(browser: browser, details: self.localizedDescription)
            }
        }
    }

    struct CookieRecord: Sendable {
        let host: String
        let name: String
        let path: String
        let value: String
        let expires: Date?
        let isSecure: Bool
        let isHTTPOnly: Bool

        var record: BrowserCookieRecord {
            BrowserCookieRecord(
                domain: BrowserCookieDomainMatcher.normalizeDomain(self.host),
                name: self.name,
                path: self.path,
                value: self.value,
                expires: self.expires,
                isSecure: self.isSecure,
                isHTTPOnly: self.isHTTPOnly)
        }
    }

    static func availableStores(for browser: Browser, homeDirectories: [URL]) -> [BrowserCookieStore] {
        guard browser.engine == .gecko else { return [] }
        let roots = homeDirectories.map { $0.appendingPathComponent(".mozilla/firefox") }
        return roots.flatMap { self.profileCookieDBs(root: $0, labelPrefix: browser.displayName, browser: browser) }
            .filter { FileManager.default.fileExists(atPath: $0.databaseURL?.path ?? "") }
    }

    static func loadCookies(
        from store: BrowserCookieStore,
        matchingDomains domains: [String],
        domainMatch: BrowserCookieDomainMatch) throws -> [CookieRecord]
    {
        guard let sourceDB = store.databaseURL else {
            throw ImportError.cookieDBNotFound(path: "Missing cookie DB for \(store.label)", browser: store.browser)
        }
        guard FileManager.default.isReadableFile(atPath: sourceDB.path) else {
            throw ImportError.cookieDBNotReadable(path: sourceDB.path, browser: store.browser)
        }
        return try self.readCookiesFromLockedDB(sourceDB: sourceDB, matchingDomains: domains, domainMatch: domainMatch, browser: store.browser)
    }

    private static func readCookiesFromLockedDB(
        sourceDB: URL,
        matchingDomains domains: [String],
        domainMatch: BrowserCookieDomainMatch,
        browser: Browser) throws -> [CookieRecord]
    {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("sweet-cookie-kit-linux-gecko-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let copiedDB = tempDir.appendingPathComponent("cookies.sqlite")
        try FileManager.default.copyItem(at: sourceDB, to: copiedDB)
        for suffix in ["-wal", "-shm"] {
            let src = URL(fileURLWithPath: sourceDB.path + suffix)
            if FileManager.default.fileExists(atPath: src.path) {
                try? FileManager.default.copyItem(at: src, to: URL(fileURLWithPath: copiedDB.path + suffix))
            }
        }
        return try self.readCookies(fromDB: copiedDB.path, matchingDomains: domains, domainMatch: domainMatch, browser: browser)
    }

    private static func readCookies(
        fromDB path: String,
        matchingDomains domains: [String],
        domainMatch: BrowserCookieDomainMatch,
        browser: Browser) throws -> [CookieRecord]
    {
        var db: OpaquePointer?
        if sqlite3_open_v2(path, &db, SQLITE_OPEN_READONLY, nil) != SQLITE_OK {
            throw ImportError.sqliteFailed(message: String(cString: sqlite3_errmsg(db)), browser: browser)
        }
        defer { sqlite3_close(db) }

        let conditions = BrowserCookieDomainMatcher.sqlCondition(column: "host", patterns: domains, match: domainMatch)
        let sql = """
        SELECT host, name, path, value, expiry, isSecure, isHttpOnly
        FROM moz_cookies
        WHERE \(conditions)
        """

        var stmt: OpaquePointer?
        if sqlite3_prepare_v2(db, sql, -1, &stmt, nil) != SQLITE_OK {
            throw ImportError.sqliteFailed(message: String(cString: sqlite3_errmsg(db)), browser: browser)
        }
        defer { sqlite3_finalize(stmt) }

        var out: [CookieRecord] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let host = readText(stmt, 0), let name = readText(stmt, 1), let cookiePath = readText(stmt, 2), let value = readText(stmt, 3) else { continue }
            let expiry = sqlite3_column_int64(stmt, 4)
            out.append(CookieRecord(
                host: host,
                name: name,
                path: cookiePath,
                value: value,
                expires: expiry > 0 ? Date(timeIntervalSince1970: TimeInterval(expiry)) : nil,
                isSecure: sqlite3_column_int(stmt, 5) != 0,
                isHTTPOnly: sqlite3_column_int(stmt, 6) != 0))
        }
        return out
    }

    private static func readText(_ stmt: OpaquePointer?, _ index: Int32) -> String? {
        guard sqlite3_column_type(stmt, index) != SQLITE_NULL, let c = sqlite3_column_text(stmt, index) else { return nil }
        return String(cString: c)
    }

    private static func profileCookieDBs(root: URL, labelPrefix: String, browser: Browser) -> [BrowserCookieStore] {
        guard let entries = try? FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) else { return [] }
        return entries.filter { (try? $0.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true }
            .sorted { profileSortKey($0.lastPathComponent) < profileSortKey($1.lastPathComponent) }
            .map { dir in
                let profile = BrowserProfile(id: dir.path, name: dir.lastPathComponent)
                return BrowserCookieStore(
                    browser: browser,
                    profile: profile,
                    kind: .primary,
                    label: "\(labelPrefix) \(profile.name)",
                    databaseURL: dir.appendingPathComponent("cookies.sqlite"))
            }
    }

    private static func profileSortKey(_ name: String) -> String {
        let lower = name.lowercased()
        if lower.contains("default-release") { return "0-\(lower)" }
        if lower.contains("default") { return "1-\(lower)" }
        return "2-\(lower)"
    }
}

#endif
