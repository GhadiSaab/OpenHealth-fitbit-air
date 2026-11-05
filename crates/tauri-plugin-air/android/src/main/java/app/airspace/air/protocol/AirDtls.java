package app.airspace.air.protocol;

import java.io.IOException;
import java.security.SecureRandom;
import java.util.function.BiConsumer;
import org.bouncycastle.tls.*;
import org.bouncycastle.tls.crypto.impl.bc.BcTlsCrypto;

/** Maintained DTLS implementation; keys resolved only for the provisioned identity. */
public final class AirDtls {
  private AirDtls() {}

  public static DTLSTransport accept(DatagramTransport transport, byte[] identity, byte[] key)
      throws IOException {
    return accept(transport,identity,key,(name,value)->{});
  }
  public static DTLSTransport accept(DatagramTransport transport,byte[] identity,byte[] key,BiConsumer<String,Long> observer)throws IOException{
    if (identity.length == 0 || key.length != 16)
      throw new IOException("Unsupported device credentials");
    TlsPSKIdentityManager keys =
        new TlsPSKIdentityManager() {
          public byte[] getHint() {
            return null;
          }

          public byte[] getPSK(byte[] requested) {
            boolean match=org.bouncycastle.util.Arrays.constantTimeAreEqual(identity,requested);
            observer.accept("key_lookup",1L);if(match)observer.accept("key_match",1L);
            if(java.util.Arrays.equals(requested,new byte[]{66,79,79,84,83,84,82,65,80}))observer.accept("bootstrap",1L);
            return match?key.clone():null;
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
