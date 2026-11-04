package app.airspace.air;

import android.content.Context;
import android.security.keystore.*;
import android.util.Base64;
import java.security.KeyStore;
import javax.crypto.*;
import javax.crypto.spec.GCMParameterSpec;
import org.json.JSONObject;

/** Device-only secrets; the service can open this without an Activity. */
final class AirSecrets {
  private static final String ALIAS = "airspace_air_credentials_v1";

  private static javax.crypto.SecretKey key() throws Exception {
    KeyStore s = KeyStore.getInstance("AndroidKeyStore");
    s.load(null);
    if (s.containsAlias(ALIAS))
      return ((KeyStore.SecretKeyEntry) s.getEntry(ALIAS, null)).getSecretKey();
    KeyGenerator g = KeyGenerator.getInstance("AES", "AndroidKeyStore");
    g.init(
        new KeyGenParameterSpec.Builder(
                ALIAS, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .build());
    return g.generateKey();
  }

  static void save(Context c, JSONObject value) throws Exception {
    Cipher x = Cipher.getInstance("AES/GCM/NoPadding");
    x.init(Cipher.ENCRYPT_MODE, key());
    String v =
        Base64.encodeToString(x.getIV(), 2)
            + ":"
            + Base64.encodeToString(x.doFinal(value.toString().getBytes("UTF-8")), 2);
    if (!c.getSharedPreferences(ALIAS, 0).edit().putString("sealed", v).commit())
      throw new Exception("Secure storage failed");
  }

  static JSONObject load(Context c) throws Exception {
    String s = c.getSharedPreferences(ALIAS, 0).getString("sealed", null);
    if (s == null) return null;
    String[] p = s.split(":", 2);
    Cipher x = Cipher.getInstance("AES/GCM/NoPadding");
    x.init(Cipher.DECRYPT_MODE, key(), new GCMParameterSpec(128, Base64.decode(p[0], 2)));
    return new JSONObject(new String(x.doFinal(Base64.decode(p[1], 2)), "UTF-8"));
  }

  static void delete(Context c) {
    c.getSharedPreferences(ALIAS, 0).edit().clear().commit();
  }
}
