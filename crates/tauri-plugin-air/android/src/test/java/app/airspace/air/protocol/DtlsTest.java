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
    for (boolean valid : new boolean[] {true, false}) {
      Pipe a = new Pipe(), b = new Pipe();
      a.other = b;
      b.other = a;
      byte[] id = "MD-00000001-00000002".getBytes("US-ASCII"), key = new byte[16];
      Arrays.fill(key, (byte) 7);
      ExecutorService ex = Executors.newSingleThreadExecutor();
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
                valid ? id : "wrong".getBytes("US-ASCII"),
                key) {
              protected ProtocolVersion[] getSupportedVersions() {
                return new ProtocolVersion[] {ProtocolVersion.DTLSv12};
              }

              protected int[] getSupportedCipherSuites() {
                return new int[] {CipherSuite.TLS_PSK_WITH_AES_128_CCM_8};
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
      a.close();
      b.close();
      ex.shutdownNow();
    }
    System.out.println("DTLS 1.2 PSK interoperability and wrong-identity rejection passed");
  }
}
