package org.rustls.platformverifier

import android.content.Context
import java.io.ByteArrayInputStream
import java.security.KeyStore
import java.security.cert.CertificateException
import java.security.cert.CertificateExpiredException
import java.security.cert.CertificateFactory
import java.security.cert.CertificateNotYetValidException
import java.security.cert.X509Certificate
import javax.net.ssl.TrustManagerFactory
import javax.net.ssl.X509TrustManager

/**
 * rustls-platform-verifier 的 Android Kotlin 组件。
 *
 * Rust 侧（rustls-platform-verifier crate）通过 JNI 以静态方法调用
 * verifyCertificateChain，用系统 CA 存储校验服务器证书链。
 * 官方分发的 AAR 组件因网络原因无法从 Gradle 拉取，此处按其 JNI 契约自行实现：
 * https://github.com/rustls/rustls-platform-verifier
 */
object CertificateVerifier {

    @JvmStatic
    fun verifyCertificateChain(
        context: Context,
        serverName: String,
        authType: String,
        allowedEkus: Array<String>,
        ocspResponse: ByteArray?,
        now: Long,
        chain: Array<ByteArray>
    ): VerificationResult {
        if (chain.isEmpty()) {
            return VerificationResult(5, "empty certificate chain")
        }

        // 解析 X.509 证书链
        val cf = try {
            CertificateFactory.getInstance("X.509")
        } catch (e: Exception) {
            return VerificationResult(5, "certificate factory unavailable: ${e.message}")
        }

        val certs = try {
            chain.map { cf.generateCertificate(ByteArrayInputStream(it)) as X509Certificate }
        } catch (e: CertificateException) {
            return VerificationResult(5, "certificate encoding error: ${e.message}")
        }

        // 用系统默认信任管理器（Android 系统 CA 存储）校验
        val trustManager = try {
            val tmf = TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm())
            tmf.init(null as KeyStore?)
            tmf.trustManagers.filterIsInstance<X509TrustManager>().firstOrNull()
                ?: return VerificationResult(1, "no X509TrustManager available")
        } catch (e: Exception) {
            return VerificationResult(1, "trust manager unavailable: ${e.message}")
        }

        return try {
            trustManager.checkServerTrusted(certs.toTypedArray(), authType)
            VerificationResult(0, null)
        } catch (e: CertificateExpiredException) {
            VerificationResult(2, e.message)
        } catch (e: CertificateNotYetValidException) {
            VerificationResult(2, e.message)
        } catch (e: CertificateException) {
            // 包含 CertPathValidatorException（签名链断裂、不受信任的签发者等）
            VerificationResult(3, e.message)
        } catch (e: Exception) {
            VerificationResult(3, e.message)
        }
    }
}
