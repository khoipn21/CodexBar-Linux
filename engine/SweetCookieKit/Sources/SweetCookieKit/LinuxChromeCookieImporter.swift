#if os(Linux)
import Foundation

enum ChromeCookieImporter {
    enum ImportError: LocalizedError {
        case cookieDBNotFound(path: String)
        case sqliteFailed(message: String)
        case keychainDenied

        var errorDescription: String? {
            switch self {
            case let .cookieDBNotFound(path): "Chromium Cookies DB not found at \(path)."
            case let .sqliteFailed(message): "Failed to read Chromium cookies: \(message)"
            case .keychainDenied: "Linux secret service denied access to Chromium Safe Storage."
            }
        }

        func browserCookieError(browser: Browser) -> BrowserCookieError {
            switch self {
            case .cookieDBNotFound:
                .notFound(browser: browser, details: self.localizedDescription)
            case .keychainDenied:
                .accessDenied(browser: browser, details: self.localizedDescription)
            case .sqliteFailed:
                .loadFailed(browser: browser, details: self.localizedDescription)
            }
        }
    }

    struct CookieRecord: Sendable {
        let hostKey: String
        let name: String
        let path: String
        let expiresUTC: Int64
        let isSecure: Bool
        let isHTTPOnly: Bool
        let value: String

        var record: BrowserCookieRecord {
            BrowserCookieRecord(
                domain: BrowserCookieDomainMatcher.normalizeDomain(self.hostKey),
                name: self.name,
                path: self.path,
                value: self.value,
                expires: BrowserCookieDomainMatcher.chromeExpiryDate(expiresUTC: self.expiresUTC),
                isSecure: self.isSecure,
                isHTTPOnly: self.isHTTPOnly)
        }
    }

    static func availableStores(for browser: Browser, homeDirectories: [URL]) -> [BrowserCookieStore] {
        guard browser.engine == .chromium else { return [] }
        return ChromiumProfileLocator.roots(for: [browser], homeDirectories: homeDirectories).flatMap { root in
            self.profileCookieDBs(root: root.url, labelPrefix: root.labelPrefix, browser: browser)
        }.filter { FileManager.default.fileExists(atPath: $0.databaseURL?.path ?? "") }
    }

    static func loadCookies(
        from store: BrowserCookieStore,
        matchingDomains domains: [String],
        domainMatch: BrowserCookieDomainMatch) throws -> [CookieRecord]
    {
        guard let sourceDB = store.databaseURL else {
            throw ImportError.cookieDBNotFound(path: "Missing cookie DB for \(store.label)")
        }
        return try self.readCookiesFromLockedDB(
            sourceDB: sourceDB,
            matchingDomains: domains,
            domainMatch: domainMatch)
    }

    private static func readCookiesFromLockedDB(
        sourceDB: URL,
        matchingDomains domains: [String],
        domainMatch: BrowserCookieDomainMatch) throws -> [CookieRecord]
    {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("sweet-cookie-kit-linux-chrome-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let copiedDB = tempDir.appendingPathComponent("Cookies")
        try FileManager.default.copyItem(at: sourceDB, to: copiedDB)
        for suffix in ["-wal", "-shm"] {
            let src = URL(fileURLWithPath: sourceDB.path + suffix)
            if FileManager.default.fileExists(atPath: src.path) {
                try? FileManager.default.copyItem(at: src, to: URL(fileURLWithPath: copiedDB.path + suffix))
            }
        }

        return try self.readCookies(fromDB: copiedDB.path, matchingDomains: domains, domainMatch: domainMatch)
    }

    private static func readCookies(
        fromDB path: String,
        matchingDomains domains: [String],
        domainMatch: BrowserCookieDomainMatch) throws -> [CookieRecord]
    {
        var db: OpaquePointer?
        if sqlite3_open_v2(path, &db, SQLITE_OPEN_READONLY, nil) != SQLITE_OK {
            throw ImportError.sqliteFailed(message: String(cString: sqlite3_errmsg(db)))
        }
        defer { sqlite3_close(db) }

        let conditions = BrowserCookieDomainMatcher.sqlCondition(column: "host_key", patterns: domains, match: domainMatch)
        let sql = """
        SELECT host_key, name, path, expires_utc, is_secure, is_httponly, value, encrypted_value
        FROM cookies
        WHERE \(conditions)
        """

        var stmt: OpaquePointer?
        if sqlite3_prepare_v2(db, sql, -1, &stmt, nil) != SQLITE_OK {
            throw ImportError.sqliteFailed(message: String(cString: sqlite3_errmsg(db)))
        }
        defer { sqlite3_finalize(stmt) }

        var out: [CookieRecord] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            guard let host = readText(stmt, 0), let name = readText(stmt, 1), let cookiePath = readText(stmt, 2) else { continue }
            let expires = sqlite3_column_int64(stmt, 3)
            let isSecure = sqlite3_column_int(stmt, 4) != 0
            let isHTTPOnly = sqlite3_column_int(stmt, 5) != 0
            let plain = readText(stmt, 6) ?? ""
            let encrypted = readBlob(stmt, 7)
            let value = !plain.isEmpty ? plain : decryptChromiumValue(encrypted ?? Data())
            guard !value.isEmpty else { continue }
            out.append(CookieRecord(hostKey: host, name: name, path: cookiePath, expiresUTC: expires, isSecure: isSecure, isHTTPOnly: isHTTPOnly, value: value))
        }
        return out
    }

    private static func readText(_ stmt: OpaquePointer?, _ index: Int32) -> String? {
        guard sqlite3_column_type(stmt, index) != SQLITE_NULL, let c = sqlite3_column_text(stmt, index) else { return nil }
        return String(cString: c)
    }

    private static func readBlob(_ stmt: OpaquePointer?, _ index: Int32) -> Data? {
        guard sqlite3_column_type(stmt, index) != SQLITE_NULL, let bytes = sqlite3_column_blob(stmt, index) else { return nil }
        return Data(bytes: bytes, count: Int(sqlite3_column_bytes(stmt, index)))
    }

    private static func decryptChromiumValue(_ data: Data) -> String {
        guard data.count > 3 else { return "" }
        if !data.starts(with: Data("v10".utf8)) && !data.starts(with: Data("v11".utf8)) {
            return String(data: data, encoding: .utf8) ?? ""
        }
        return ""
    }

    private static func profileCookieDBs(root: URL, labelPrefix: String, browser: Browser) -> [BrowserCookieStore] {
        guard let entries = try? FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) else { return [] }
        let profiles = entries.filter { url in
            guard (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true else { return false }
            let name = url.lastPathComponent
            return name == "Default" || name.hasPrefix("Profile ") || name.hasPrefix("user-")
        }.sorted { $0.lastPathComponent < $1.lastPathComponent }

        return profiles.flatMap { profileDir in
            let profile = BrowserProfile(id: profileDir.path, name: profileDir.lastPathComponent)
            let base = "\(labelPrefix) \(profile.name)"
            return [
                BrowserCookieStore(
                    browser: browser,
                    profile: profile,
                    kind: .network,
                    label: "\(base) (Network)",
                    databaseURL: profileDir.appendingPathComponent("Network").appendingPathComponent("Cookies")),
                BrowserCookieStore(
                    browser: browser,
                    profile: profile,
                    kind: .primary,
                    label: base,
                    databaseURL: profileDir.appendingPathComponent("Cookies")),
            ]
        }
    }
}

#endif
