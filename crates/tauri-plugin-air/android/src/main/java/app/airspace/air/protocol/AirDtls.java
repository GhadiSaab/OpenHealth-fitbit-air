package app.airspace.air.protocol;

import java.io.IOException;
import java.security.SecureRandom;
import org.bouncycastle.tls.*;
import org.bouncycastle.tls.crypto.impl.bc.BcTlsCrypto;

/** Maintained DTLS implementation; keys resolved only for the provisioned identity. */
public final class AirDtls {
  private AirDtls() {}

  public static DTLSTransport accept(DatagramTransport transport, byte[] identity, byte[] key)
      throws IOException {
    if (identity.length == 0 || key.length != 16)
      throw new IOException("Unsupported device credentials");
    TlsPSKIdentityManager keys =
        new TlsPSKIdentityManager() {
          public byte[] getHint() {
            return null;
          }

          public byte[] getPSK(byte[] requested) {
            return org.bouncycastle.util.Arrays.constantTimeAreEqual(identity, requested)
                ? key.clone()
                : null;
          }
        };
    PSKTlsServer server =
        new PSKTlsServer(new BcTlsCrypto(new SecureRandom()), keys) {
          protected ProtocolVersion[] getSupportedVersions() {
            return new ProtocolVersion[] {ProtocolVersion.DTLSv12};
          }

          protected int[] getSupportedCipherSuites() {
            return new int[] {
              CipherSuite.TLS_PSK_WITH_AES_128_CCM_8,
              CipherSuite.TLS_PSK_WITH_AES_128_CCM,
              CipherSuite.TLS_PSK_WITH_AES_128_CBC_SHA256
            };
          }

          public boolean requiresExtendedMasterSecret() {
            return false;
          }

          public int getHandshakeTimeoutMillis() {
            return 15000;
          }

          public int getHandshakeResendTimeMillis() {
            return 1000;
          }
        };
    return new DTLSServerProtocol().accept(server, transport);
  }
}
