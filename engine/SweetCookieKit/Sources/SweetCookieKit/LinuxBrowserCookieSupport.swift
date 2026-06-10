#if os(Linux)
import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

public enum Browser: String, Sendable, Hashable, CaseIterable {
    public static var allCases: [Browser] { [.chrome, .chromium, .brave, .edge, .firefox] }

    case chrome
    case chromium
    case brave
    case edge
    case firefox

    public var displayName: String {
        switch self {
        case .chrome: "Chrome"
        case .chromium: "Chromium"
        case .brave: "Brave"
        case .edge: "Microsoft Edge"
        case .firefox: "Firefox"
        }
    }

    public var usesChromiumProfileStore: Bool { self.engine == .chromium }
    public var usesGeckoProfileStore: Bool { self.engine == .gecko }
    public var safeStorageLabels: [(service: String, account: String)] { [] }
    public static var safeStorageLabels: [(service: String, account: String)] { [] }
    public static var defaultImportOrder: [Browser] { [.chrome, .chromium, .brave, .edge, .firefox] }

    var engine: BrowserEngine {
        switch self {
        case .chrome, .chromium, .brave, .edge: .chromium
        case .firefox: .gecko
        }
    }

    var linuxProfileRelativePath: String? {
        switch self {
        case .chrome: ".config/google-chrome"
        case .chromium: ".config/chromium"
        case .brave: ".config/BraveSoftware/Brave-Browser"
        case .edge: ".config/microsoft-edge"
        case .firefox: nil
        }
    }
}

enum BrowserEngine { case chromium, gecko }

public enum BrowserCookieDefaults {
    public static let importOrder: [Browser] = Browser.defaultImportOrder
}

public struct ChromiumProfileRoot: Sendable {
    public let browser: Browser
    public let url: URL
    public var labelPrefix: String { self.browser.displayName }
    public init(browser: Browser, url: URL) {
        self.browser = browser
        self.url = url
    }
}

public enum ChromiumProfileLocator {
    public static func roots(
        for browsers: [Browser] = Browser.defaultImportOrder,
        homeDirectories: [URL] = BrowserCookieClient.defaultHomeDirectories()) -> [ChromiumProfileRoot]
    {
        uniqueHomes(homeDirectories).flatMap { home in
            browsers.compactMap { browser in
                guard browser.engine == .chromium, let rel = browser.linuxProfileRelativePath else { return nil }
                return ChromiumProfileRoot(browser: browser, url: home.appendingPathComponent(rel))
            }
        }
    }

    private static func uniqueHomes(_ homes: [URL]) -> [URL] {
        var seen = Set<String>()
        return homes.filter { seen.insert($0.path).inserted }
    }
}

extension Collection<Browser> {
    public var displayLabel: String { map(\.displayName).joined(separator: " → ") }
    public var shortLabel: String { map(\.displayName).joined(separator: "/") }
    public var loginHint: String {
        let names = map(\.displayName)
        guard let last = names.last else { return "browser" }
        if names.count == 1 { return last }
        if names.count == 2 { return "\(names[0]) or \(last)" }
        return "\(names.dropLast().joined(separator: ", ")), or \(last)"
    }
}

public enum BrowserCookieDomainMatch: Sendable { case contains, suffix, exact }

public enum BrowserCookieOriginStrategy: Sendable {
    case domainBased
    case fixed(URL)
    case custom(@Sendable (String) -> URL?)

    func resolve(domain: String) -> URL? {
        switch self {
        case .domainBased: URL(string: "https://\(domain)")
        case let .fixed(url): url
        case let .custom(resolver): resolver(domain)
        }
    }
}

public struct BrowserCookieQuery: Sendable {
    public var domains: [String]
    public var domainMatch: BrowserCookieDomainMatch
    public var origin: BrowserCookieOriginStrategy
    public var includeExpired: Bool
    public var referenceDate: Date

    public init(
        domains: [String] = [],
        domainMatch: BrowserCookieDomainMatch = .contains,
        origin: BrowserCookieOriginStrategy = .domainBased,
        includeExpired: Bool = false,
        referenceDate: Date = Date())
    {
        self.domains = domains
        self.domainMatch = domainMatch
        self.origin = origin
        self.includeExpired = includeExpired
        self.referenceDate = referenceDate
    }
}

public struct BrowserProfile: Sendable, Hashable {
    public let id: String
    public let name: String
    public init(id: String, name: String) { self.id = id; self.name = name }
}

public enum BrowserCookieStoreKind: String, Sendable { case primary, network, safari }

public struct BrowserCookieStore: Sendable, Hashable {
    public let browser: Browser
    public let profile: BrowserProfile
    public let kind: BrowserCookieStoreKind
    public let label: String
    public let databaseURL: URL?
    public init(browser: Browser, profile: BrowserProfile, kind: BrowserCookieStoreKind, label: String, databaseURL: URL?) {
        self.browser = browser
        self.profile = profile
        self.kind = kind
        self.label = label
        self.databaseURL = databaseURL
    }
}

public struct BrowserCookieRecord: Sendable {
    public let domain: String
    public let name: String
    public let path: String
    public let value: String
    public let expires: Date?
    public let isSecure: Bool
    public let isHTTPOnly: Bool
    public init(domain: String, name: String, path: String, value: String, expires: Date?, isSecure: Bool, isHTTPOnly: Bool) {
        self.domain = domain
        self.name = name
        self.path = path
        self.value = value
        self.expires = expires
        self.isSecure = isSecure
        self.isHTTPOnly = isHTTPOnly
    }
}

public struct BrowserCookieStoreRecords: Sendable {
    public let store: BrowserCookieStore
    public let records: [BrowserCookieRecord]
    public init(store: BrowserCookieStore, records: [BrowserCookieRecord]) { self.store = store; self.records = records }
    public var label: String { self.store.label }
    public var browser: Browser { self.store.browser }
    public func cookies(origin: BrowserCookieOriginStrategy = .domainBased) -> [HTTPCookie] {
        BrowserCookieClient.makeHTTPCookies(self.records, origin: origin)
    }
}

public enum BrowserCookieError: LocalizedError, Sendable {
    case notFound(browser: Browser, details: String)
    case accessDenied(browser: Browser, details: String)
    case loadFailed(browser: Browser, details: String)

    public var errorDescription: String? {
        switch self {
        case let .notFound(_, details), let .accessDenied(_, details), let .loadFailed(_, details): details
        }
    }

    public var browser: Browser {
        switch self {
        case let .notFound(browser, _), let .accessDenied(browser, _), let .loadFailed(browser, _): browser
        }
    }

    public var accessDeniedHint: String? {
        if case let .accessDenied(_, details) = self { return details }
        return nil
    }
}

public struct BrowserCookieKeychainPromptContext: Sendable {
    public let service: String
    public let account: String
    public let label: String
    public init(service: String, account: String, label: String) {
        self.service = service
        self.account = account
        self.label = label
    }
}

public enum BrowserCookieKeychainPromptHandler {
    public nonisolated(unsafe) static var handler: ((BrowserCookieKeychainPromptContext) -> Void)?
}

public struct BrowserCookieClient: Sendable {
    public struct Configuration: Sendable {
        public var homeDirectories: [URL]
        public init(homeDirectories: [URL] = BrowserCookieClient.defaultHomeDirectories()) { self.homeDirectories = homeDirectories }
    }

    public let configuration: Configuration
    public init(configuration: Configuration = Configuration()) { self.configuration = configuration }

    public func stores(for browser: Browser) -> [BrowserCookieStore] {
        switch browser.engine {
        case .chromium: ChromeCookieImporter.availableStores(for: browser, homeDirectories: self.configuration.homeDirectories)
        case .gecko: GeckoCookieImporter.availableStores(for: browser, homeDirectories: self.configuration.homeDirectories)
        }
    }

    public func stores(in browsers: [Browser]) -> [BrowserCookieStore] { browsers.flatMap { self.stores(for: $0) } }

    public func records(matching query: BrowserCookieQuery, in browsers: [Browser], logger: ((String) -> Void)? = nil) throws -> [BrowserCookieStoreRecords] {
        try browsers.flatMap { try self.records(matching: query, in: $0, logger: logger) }
    }

    public func records(matching query: BrowserCookieQuery, in browser: Browser, logger: ((String) -> Void)? = nil) throws -> [BrowserCookieStoreRecords] {
        let stores = self.stores(for: browser)
        if stores.isEmpty { throw BrowserCookieError.notFound(browser: browser, details: "\(browser.displayName) cookie store not found.") }
        return try stores.compactMap { store in
            let records = try self.records(matching: query, in: store, logger: logger)
            guard !records.isEmpty else { return nil }
            return BrowserCookieStoreRecords(store: store, records: records)
        }
    }

    public func records(matching query: BrowserCookieQuery, in store: BrowserCookieStore, logger: ((String) -> Void)? = nil) throws -> [BrowserCookieRecord] {
        let records: [BrowserCookieRecord]
        switch store.browser.engine {
        case .chromium:
            do { records = try ChromeCookieImporter.loadCookies(from: store, matchingDomains: query.domains, domainMatch: query.domainMatch).map { $0.record } }
            catch let error as ChromeCookieImporter.ImportError { throw error.browserCookieError(browser: store.browser) }
        case .gecko:
            do { records = try GeckoCookieImporter.loadCookies(from: store, matchingDomains: query.domains, domainMatch: query.domainMatch).map { $0.record } }
            catch let error as GeckoCookieImporter.ImportError { throw error.browserCookieError(browser: store.browser) }
        }
        return BrowserCookieDomainMatcher.filterExpired(records, includeExpired: query.includeExpired, now: query.referenceDate)
    }

    public func cookies(matching query: BrowserCookieQuery, in store: BrowserCookieStore, logger: ((String) -> Void)? = nil) throws -> [HTTPCookie] {
        Self.makeHTTPCookies(try self.records(matching: query, in: store, logger: logger), origin: query.origin)
    }

    public func cookies(matching query: BrowserCookieQuery, in browser: Browser, logger: ((String) -> Void)? = nil) throws -> [HTTPCookie] {
        try self.records(matching: query, in: browser, logger: logger).flatMap { $0.cookies(origin: query.origin) }
    }

    public func cookies(matching query: BrowserCookieQuery, in browsers: [Browser], logger: ((String) -> Void)? = nil) throws -> [HTTPCookie] {
        try self.records(matching: query, in: browsers, logger: logger).flatMap { $0.cookies(origin: query.origin) }
    }

    public static func makeHTTPCookies(_ records: [BrowserCookieRecord], origin: BrowserCookieOriginStrategy = .domainBased) -> [HTTPCookie] {
        records.compactMap { record in
            let domain = BrowserCookieDomainMatcher.normalizeDomain(record.domain)
            guard !domain.isEmpty else { return nil }
            var props: [HTTPCookiePropertyKey: Any] = [
                .domain: domain,
                .path: record.path,
                .name: record.name,
                .value: record.value,
                .secure: record.isSecure,
            ]
            if let originURL = origin.resolve(domain: domain) { props[.originURL] = originURL }
            if record.isHTTPOnly { props[.init("HttpOnly")] = "TRUE" }
            if let expires = record.expires { props[.expires] = expires }
            return HTTPCookie(properties: props)
        }
    }

    public static func defaultHomeDirectories() -> [URL] {
        var homes = [FileManager.default.homeDirectoryForCurrentUser]
        if let envHome = ProcessInfo.processInfo.environment["HOME"], !envHome.isEmpty { homes.append(URL(fileURLWithPath: envHome)) }
        var seen = Set<String>()
        return homes.filter { seen.insert($0.path).inserted }
    }
}

enum BrowserCookieDomainMatcher {
    static func normalizeDomain(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.hasPrefix(".") ? String(trimmed.dropFirst()) : trimmed
    }

    static func matches(domain: String, patterns: [String], match: BrowserCookieDomainMatch) -> Bool {
        guard !patterns.isEmpty else { return true }
        let haystack = self.normalizeDomain(domain).lowercased()
        return patterns.contains { pattern in
            let needle = self.normalizeDomain(pattern).lowercased()
            switch match {
            case .contains:
                return haystack.contains(needle)
            case .suffix:
                return haystack.hasSuffix(needle)
            case .exact:
                return haystack == needle
            }
        }
    }

    static func filterExpired(_ records: [BrowserCookieRecord], includeExpired: Bool, now: Date) -> [BrowserCookieRecord] {
        includeExpired ? records : records.filter { $0.expires.map { $0 >= now } ?? true }
    }

    static func chromeExpiryDate(expiresUTC: Int64) -> Date? {
        guard expiresUTC > 0 else { return nil }
        let seconds = (Double(expiresUTC) / 1_000_000.0) - 11_644_473_600.0
        return seconds > 0 ? Date(timeIntervalSince1970: seconds) : nil
    }

    static func sqlCondition(column: String, patterns: [String], match: BrowserCookieDomainMatch) -> String {
        guard !patterns.isEmpty else { return "1=1" }
        return patterns.map { raw in
            let value = escapeForSQL(raw)
            switch match {
            case .contains:
                return "\(column) LIKE '%\(value)%'"
            case .suffix:
                return "\(column) LIKE '%\(value)'"
            case .exact:
                let normalized = escapeForSQL(normalizeDomain(raw))
                return "(\(column) = '\(normalized)' OR \(column) = '.\(normalized)')"
            }
        }.joined(separator: " OR ")
    }

    static func escapeForSQL(_ value: String) -> String { value.replacingOccurrences(of: "'", with: "''") }
}

#endif
