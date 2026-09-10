package com.campuslogin.plugin.keystore

import android.security.keystore.KeyGenParameterSpec
import android.util.Log
import android.security.keystore.KeyProperties
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import java.security.SecureRandom
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class EncryptArgs {
  lateinit var text: String
}

@InvokeArg
class DecryptArgs {
  lateinit var data: String
}

@TauriPlugin
class KeystorePlugin(private val activity: android.app.Activity) : Plugin(activity) {

    companion object {
        private const val KEY_ALIAS = "campus_login_master"
        private const val IV_LEN = 12
        private const val TAG = "CampusKeystore"
    }

    private fun getOrCreateKey(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return generator.generateKey()
    }

    @Command
    fun encrypt(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(EncryptArgs::class.java)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
            val iv = cipher.iv
            val ct = cipher.doFinal(args.text.toByteArray(Charsets.UTF_8))
            val ret = JSObject()
            ret.put("data", Base64.getEncoder().encodeToString(iv + ct))
            invoke.resolve(ret)
        } catch (e: Exception) {
            Log.e(TAG, "encrypt failed: ${e.javaClass.name}: ${e.message}", e)
            invoke.reject("Keystore 加密失败: ${e.message}")
        }
    }

    @Command
    fun decrypt(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(DecryptArgs::class.java)
            val raw = Base64.getDecoder().decode(args.data)
            require(raw.size > IV_LEN) { "密文长度非法" }
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(128, raw, 0, IV_LEN))
            val plain = cipher.doFinal(raw, IV_LEN, raw.size - IV_LEN)
            val ret = JSObject()
            ret.put("text", String(plain, Charsets.UTF_8))
            invoke.resolve(ret)
        } catch (e: Exception) {
            Log.e(TAG, "decrypt failed: ${e.javaClass.name}: ${e.message}", e)
            invoke.reject("Keystore 解密失败: ${e.message}")
        }
    }
}
