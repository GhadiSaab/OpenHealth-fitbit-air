package app.airspace.vault

import android.app.Activity
import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class SetArgs {
    var key: String = ""
    var value: String = ""
}

@InvokeArg
class KeyArgs {
    var key: String = ""
}

/**
 * Stores small secrets (the user's AI API key) encrypted with AES-256-GCM.
 * The AES key is generated inside the Android Keystore and never leaves it;
 * only ciphertext is written to app-private SharedPreferences.
 * Values are never logged.
 */
@TauriPlugin
class VaultPlugin(private val activity: Activity) : Plugin(activity) {
    private val alias = "airspace_vault_aes"
    private val prefs by lazy {
        activity.applicationContext.getSharedPreferences("airspace_vault", Context.MODE_PRIVATE)
    }

    private fun secretKey(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getEntry(alias, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return gen.generateKey()
    }

    @Command
    fun set(invoke: Invoke) {
        val args = invoke.parseArgs(SetArgs::class.java)
        try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, secretKey())
            val ct = cipher.doFinal(args.value.toByteArray(Charsets.UTF_8))
            val stored = Base64.encodeToString(cipher.iv, Base64.NO_WRAP) + ":" + Base64.encodeToString(ct, Base64.NO_WRAP)
            prefs.edit().putString(args.key, stored).apply()
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("Secure storage is unavailable")
        }
    }

    @Command
    fun get(invoke: Invoke) {
        val args = invoke.parseArgs(KeyArgs::class.java)
        val ret = JSObject()
        val stored = prefs.getString(args.key, null)
        if (stored == null) {
            ret.put("found", false)
            ret.put("value", "")
            invoke.resolve(ret)
            return
        }
        try {
            val parts = stored.split(":")
            val iv = Base64.decode(parts[0], Base64.NO_WRAP)
            val ct = Base64.decode(parts[1], Base64.NO_WRAP)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(128, iv))
            ret.put("found", true)
            ret.put("value", String(cipher.doFinal(ct), Charsets.UTF_8))
            invoke.resolve(ret)
        } catch (e: Exception) {
            // Key invalidated (e.g. device restored to new hardware): drop the unreadable value.
            prefs.edit().remove(args.key).apply()
            ret.put("found", false)
            ret.put("value", "")
            invoke.resolve(ret)
        }
    }

    @Command
    fun delete(invoke: Invoke) {
        val args = invoke.parseArgs(KeyArgs::class.java)
        prefs.edit().remove(args.key).apply()
        invoke.resolve()
    }
}
