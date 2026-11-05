package app.airspace.air.protocol;

import java.io.*;
import java.security.SecureRandom;
import java.util.*;
import java.util.concurrent.*;
import org.bouncycastle.tls.*;
import org.bouncycastle.tls.crypto.impl.bc.BcTlsCrypto;

public final class DtlsTest {
  static final class Pipe implements DatagramTransport {
    final BlockingQueue<byte[]> incoming = new LinkedBlockingQueue<>();
    Pipe other;
    volatile boolean closed;

    public int getReceiveLimit() {
      return 1400;
    }

    public int getSendLimit() {
      return 1400;
    }

    public void send(byte[] b, int o, int n) throws IOException {
      if (closed) throw new IOException("closed");
      other.incoming.add(Arrays.copyOfRange(b, o, o + n));
    }

    public int receive(byte[] b, int o, int n, int wait) throws IOException {
      try {
        byte[] x = incoming.poll(wait, TimeUnit.MILLISECONDS);
        if (closed) throw new IOException("closed");
        if (x == null) return -1;
        if (x.length > n) throw new IOException("overflow");
        System.arraycopy(x, 0, b, o, x.length);
        return x.length;
      } catch (InterruptedException e) {
        throw new IOException("interrupted");
      }
    }

    public void close() {
      closed = true;
      incoming.offer(new byte[0]);
    }
  }

  public static void main(String[] args) throws Exception {
    for(int suite : new int[]{CipherSuite.TLS_PSK_WITH_AES_128_CCM_8,CipherSuite.TLS_PSK_WITH_AES_128_CCM}) {
    for (int mode : new int[] {0,1,2}) {
      boolean valid=mode==0;
      Pipe a = new Pipe(), b = new Pipe();
      a.other = b;
      b.other = a;
      byte[] id = "MD-00000001-00000002".getBytes("US-ASCII"), key = new byte[16];
      Arrays.fill(key, (byte) 7);
      ExecutorService ex = Executors.newSingleThreadExecutor();
      try {
      Future<Boolean> server =
          ex.submit(() -> {
            try {
              DTLSTransport t = AirDtls.accept(a, id, key);
              byte[] x = new byte[20];
              int n = t.receive(x, 0, 20, 2000);
              return n == 3 && x[0] == 42;
            } catch (IOException expected) {
              return false;
            }
          });
      boolean connected = false;
      try {
        PSKTlsClient c =
            new PSKTlsClient(
                new BcTlsCrypto(new SecureRandom()),
                mode==1 ? "BOOTSTRAP".getBytes("US-ASCII") : valid ? id : "wrong".getBytes("US-ASCII"),
                mode==1 ? new byte[]{(byte)0x81,0x06,0x54,(byte)0xe3,0x36,(byte)0xad,(byte)0xca,(byte)0xb0,(byte)0xa0,0x3c,0x60,(byte)0xf7,0x4a,(byte)0xa0,(byte)0xb6,(byte)0xfb} : key) {
              protected ProtocolVersion[] getSupportedVersions() {
                return new ProtocolVersion[] {ProtocolVersion.DTLSv12};
              }

              protected int[] getSupportedCipherSuites() {
                return new int[] {suite};
              }

              public int getHandshakeTimeoutMillis() {
                return 3000;
              }
            };
        DTLSTransport client = new DTLSClientProtocol().connect(c, b);
        connected = true;
        client.send(new byte[] {42, 2, 3}, 0, 3);
      } catch (IOException expected) {
      }
      if (connected != valid || server.get(5, TimeUnit.SECONDS) != valid)
        throw new AssertionError("Strict PSK handshake failed");
      } finally {
        a.close(); b.close(); ex.shutdownNow();
      }
    }
    }
    System.out.println("DTLS 1.2 PSK interoperability and public-bootstrap and wrong-identity rejection passed (CCM and CCM-8)");
  }
}
