package app.airspace.air.protocol;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Bounded wire decoders. No secrets, Android APIs or cryptography here. */
public final class AirWire {
  private AirWire() {}

  private static int u(byte b) {
    return b & 255;
  }

  private static int word(byte[] b, int i) {
    return (u(b[i]) << 8) | u(b[i + 1]);
  }

  private static void put(byte[] b, int i, int v) {
    b[i] = (byte) (v >> 8);
    b[i + 1] = (byte) v;
  }

  private static void require(boolean b) {
    if (!b) throw new IllegalArgumentException("Malformed Air packet");
  }

  public static int checksum(byte[] b, int off, int len) {
    long n = 0;
    for (int i = 0; i < len; i += 2)
      n += (u(b[off + i]) << 8) + (i + 1 < len ? u(b[off + i + 1]) : 0);
    while ((n >>> 16) != 0) n = (n & 65535) + (n >>> 16);
    return (int) (~n) & 65535;
  }

  public static byte[] ipUdp(byte[] payload, int id, boolean host) {
    require(payload.length <= 1400);
    byte[] b = new byte[28 + payload.length];
    b[0] = 0x45;
    put(b, 2, b.length);
    put(b, 4, id);
    b[8] = (byte) 255;
    b[9] = 17;
    byte[] a = {
      (byte) 169,
      (byte) 254,
      0,
      (byte) (host ? 2 : 3),
      (byte) 169,
      (byte) 254,
      0,
      (byte) (host ? 3 : 2)
    };
    System.arraycopy(a, 0, b, 12, 8);
    put(b, 20, 5684);
    put(b, 22, 5684);
    put(b, 24, payload.length + 8);
    System.arraycopy(payload, 0, b, 28, payload.length);
    put(b, 10, checksum(b, 0, 20));
    byte[] ps = new byte[12 + 8 + payload.length];
    System.arraycopy(b, 12, ps, 0, 8);
    ps[9] = 17;
    put(ps, 10, payload.length + 8);
    System.arraycopy(b, 20, ps, 12, payload.length + 8);
    int c = checksum(ps, 0, ps.length);
    put(b, 26, c == 0 ? 65535 : c);
    return b;
  }

  public static byte[] stripIpUdp(byte[] b) {
    require(b.length >= 28 && u(b[0]) == 0x45 && u(b[9]) == 17);
    int len = word(b, 2);
    require(len == b.length && len <= 1500 && checksum(b, 0, 20) == 0);
    require(
        (word(b, 6) & 0x3fff) == 0
            && word(b, 20) == 5684
            && word(b, 22) == 5684
            && word(b, 24) == len - 20);
    require(u(b[12]) == 169 && u(b[13]) == 254 && u(b[14]) == 0 && u(b[15]) == 3);
    require(u(b[16]) == 169 && u(b[17]) == 254 && u(b[18]) == 0 && u(b[19]) == 2);
    if (word(b, 26) != 0) {
      byte[] ps = new byte[12 + len - 20];
      System.arraycopy(b, 12, ps, 0, 8);
      ps[9] = 17;
      put(ps, 10, len - 20);
      System.arraycopy(b, 20, ps, 12, len - 20);
      require(checksum(ps, 0, ps.length) == 0);
    }
    return Arrays.copyOfRange(b, 28, len);
  }

  public static Map<Integer, Long> protobuf(byte[] b) {
    require(b.length <= 4096);
    Map<Integer, Long> m = new HashMap<>();
    int[] pos = {0};
    while (pos[0] < b.length) {
      long tag = varint(b, pos);
      require((tag >>> 3) > 0 && (tag >>> 3) <= 536870911L);
      int f = (int) (tag >>> 3), wire = (int) (tag & 7);
      if (wire == 0) {
        long v = varint(b, pos);
        require(v >= 0 && !m.containsKey(f));
        m.put(f, v);
      } else {
        int n;
        if (wire == 1) n = 8;
        else if (wire == 5) n = 4;
        else if (wire == 2) {
          long l = varint(b, pos);
          require(l <= 4096);
          n = (int) l;
        } else throw new IllegalArgumentException("Unsupported protobuf wire type");
        require(n >= 0 && n <= b.length - pos[0]);
        pos[0] += n;
      }
    }
    return m;
  }

  private static long varint(byte[] b, int[] p) {
    long v = 0;
    for (int i = 0; i < 10; i++) {
      require(p[0] < b.length);
      int x = u(b[p[0]++]);
      if (i == 9) require(x <= 1);
      v |= (long) (x & 127) << (i * 7);
      if ((x & 128) == 0) return v;
    }
    throw new IllegalArgumentException("Varint overflow");
  }

  public static final class Coap {
    public final int type, code, mid;
    public final byte[] token, body;
    public final String path;

    Coap(int t, int c, int m, byte[] tok, String p, byte[] b) {
      type = t;
      code = c;
      mid = m;
      token = tok;
      path = p;
      body = b;
    }
  }

  private static int ext(byte[] b, int[] p, int n) {
    if (n < 13) return n;
    int count = n == 13 ? 1 : 2;
    require(n != 15 && p[0] + count <= b.length);
    int v = n == 13 ? u(b[p[0]++]) : word(b, p[0]);
    if (count == 2) p[0] += 2;
    return v + (n == 13 ? 13 : 269);
  }

  public static Coap parseCoap(byte[] b) {
    require(b.length >= 4 && (u(b[0]) >>> 6) == 1);
    int n = u(b[0]) & 15;
    require(n <= 8 && b.length >= 4 + n);
    byte[] tok = Arrays.copyOfRange(b, 4, 4 + n);
    int[] p = {4 + n};
    int opt = 0;
    List<String> path = new ArrayList<>();
    while (p[0] < b.length) {
      int h = u(b[p[0]++]);
      if (h == 255) {
        require(p[0] < b.length);
        return new Coap(
            (u(b[0]) >> 4) & 3,
            u(b[1]),
            word(b, 2),
            tok,
            String.join("/", path),
            Arrays.copyOfRange(b, p[0], b.length));
      }
      int delta = ext(b, p, h >>> 4), len = ext(b, p, h & 15);
      opt += delta;
      require(len <= b.length - p[0]);
      if (opt == 11) path.add(new String(b, p[0], len, StandardCharsets.UTF_8));
      p[0] += len;
    }
    return new Coap(
        (u(b[0]) >> 4) & 3, u(b[1]), word(b, 2), tok, String.join("/", path), new byte[0]);
  }

  private static void option(ByteArrayOutputStream o, int delta, byte[] value) {
    require(delta < 13 && value.length < 269);
    int n = value.length;
    o.write((delta << 4) | (n < 13 ? n : 13));
    if (n >= 13) o.write(n - 13);
    o.write(value, 0, n);
  }

  public static byte[] coap(int code, int mid, byte[] token, String path, byte[] body, int type) {
    require(token.length <= 8);
    ByteArrayOutputStream o = new ByteArrayOutputStream();
    o.write(0x40 | (type << 4) | token.length);
    o.write(code);
    o.write(mid >> 8);
    o.write(mid);
    o.write(token, 0, token.length);
    boolean first = true;
    if (!path.isEmpty())
      for (String s : path.split("/")) {
        option(o, first ? 11 : 0, s.getBytes(StandardCharsets.UTF_8));
        first = false;
      }
    if (body.length > 0) {
      o.write(255);
      o.write(body, 0, body.length);
    }
    return o.toByteArray();
  }
}
