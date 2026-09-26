package org.rustls.platformverifier

/**
 * 证书链校验结果，字段名与 Rust 侧 rustls-platform-verifier 的 JNI 读取保持一致：
 * code: Int（0=Ok 1=Unavailable 2=Expired 3=UnknownCert 4=Revoked 5=InvalidEncoding 6=InvalidExtension）
 * message: String?
 */
class VerificationResult(val code: Int, val message: String?)
