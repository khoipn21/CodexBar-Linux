#if os(Linux)
import Glibc

let SQLITE_OK: Int32 = 0
let SQLITE_ROW: Int32 = 100
let SQLITE_NULL: Int32 = 5
let SQLITE_OPEN_READONLY: Int32 = 0x00000001

@_silgen_name("sqlite3_open_v2")
func sqlite3_open_v2(
    _ filename: UnsafePointer<CChar>?,
    _ ppDb: UnsafeMutablePointer<OpaquePointer?>?,
    _ flags: Int32,
    _ zVfs: UnsafePointer<CChar>?) -> Int32

@_silgen_name("sqlite3_close")
func sqlite3_close(_ db: OpaquePointer?) -> Int32

@_silgen_name("sqlite3_errmsg")
func sqlite3_errmsg(_ db: OpaquePointer?) -> UnsafePointer<CChar>

@_silgen_name("sqlite3_prepare_v2")
func sqlite3_prepare_v2(
    _ db: OpaquePointer?,
    _ zSql: UnsafePointer<CChar>?,
    _ nByte: Int32,
    _ ppStmt: UnsafeMutablePointer<OpaquePointer?>?,
    _ pzTail: UnsafeMutablePointer<UnsafePointer<CChar>?>?) -> Int32

@_silgen_name("sqlite3_step")
func sqlite3_step(_ stmt: OpaquePointer?) -> Int32

@_silgen_name("sqlite3_finalize")
func sqlite3_finalize(_ stmt: OpaquePointer?) -> Int32

@_silgen_name("sqlite3_column_type")
func sqlite3_column_type(_ stmt: OpaquePointer?, _ iCol: Int32) -> Int32

@_silgen_name("sqlite3_column_text")
func sqlite3_column_text(_ stmt: OpaquePointer?, _ iCol: Int32) -> UnsafePointer<UInt8>?

@_silgen_name("sqlite3_column_blob")
func sqlite3_column_blob(_ stmt: OpaquePointer?, _ iCol: Int32) -> UnsafeRawPointer?

@_silgen_name("sqlite3_column_bytes")
func sqlite3_column_bytes(_ stmt: OpaquePointer?, _ iCol: Int32) -> Int32

@_silgen_name("sqlite3_column_int")
func sqlite3_column_int(_ stmt: OpaquePointer?, _ iCol: Int32) -> Int32

@_silgen_name("sqlite3_column_int64")
func sqlite3_column_int64(_ stmt: OpaquePointer?, _ iCol: Int32) -> Int64
#endif
