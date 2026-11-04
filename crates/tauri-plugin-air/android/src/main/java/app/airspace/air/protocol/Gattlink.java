package app.airspace.air.protocol;

import java.io.ByteArrayOutputStream;
import java.util.*;
import java.util.function.Consumer;

/** Small-window Gattlink endpoint. All callbacks run under the endpoint lock. */
public final class Gattlink {
  private final Consumer<byte[]> write, datagram;
  private final Runnable ready;
  private final Consumer<String> failure;
  private final Deque<byte[]> pending = new ArrayDeque<>();
  private final LinkedHashMap<Integer, byte[]> flight = new LinkedHashMap<>();
  private final ByteArrayOutputStream rx = new ByteArrayOutputStream();
  private int tx = 0, expected = 0, retries = 0;
  private long sent = 0;
  private boolean active = false;

  public Gattlink(Consumer<byte[]> w, Consumer<byte[]> d, Runnable r, Consumer<String> f) {
    write = w;
    datagram = d;
    ready = r;
    failure = f;
  }

  public synchronized void reset() {
    active = false;
    tx = expected = retries = 0;
    rx.reset();
    pending.clear();
    flight.clear();
    write.accept(new byte[] {(byte) 0x80});
  }

  public synchronized void notification(byte[] b) {
    if (b.length == 0) return;
    int h = b[0] & 255;
    if ((h & 128) != 0) {
      int cmd = h & 127;
      if (cmd == 0) {
        tx = expected = 0;
        flight.clear();
        pending.clear();
        rx.reset();
        write.accept(new byte[] {(byte) 0x81, 0, 0, 0x20, 0x20});
      } else if (cmd == 1) {
        if (!active) {
          active = true;
          ready.run();
        }
        flush();
      }
      return;
    }
    int off = 0;
    if ((h & 64) != 0) {
      int ack = h & 31;
      if (flight.containsKey(ack)) {
        Iterator<Integer> it = flight.keySet().iterator();
        while (it.hasNext()) {
          int key = it.next();
          it.remove();
          if (key == ack) break;
        }
        retries = 0;
        sent = System.currentTimeMillis();
        flush();
      }
      off = 1;
    }
    if (b.length <= off) return;
    int seq = b[off] & 31;
    if (seq != expected) {
      write.accept(new byte[] {(byte) (0x40 | ((expected + 31) & 31))});
      return;
    }
    expected = (expected + 1) & 31;
    write.accept(new byte[] {(byte) (0x40 | seq)});
    rx.write(b, off + 1, b.length - off - 1);
    byte[] a = rx.toByteArray();
    if (a.length > 4096) {
      rx.reset();
      failure.accept("Gattlink reassembly overflow");
      return;
    }
    while (a.length >= 4) {
      int total = ((a[2] & 255) << 8) | (a[3] & 255);
      if (a[0] != 0x45 || total < 28 || total > 1500) {
        rx.reset();
        failure.accept("Invalid Gattlink IP frame");
        return;
      }
      if (a.length < total) break;
      datagram.accept(Arrays.copyOfRange(a, 0, total));
      a = Arrays.copyOfRange(a, total, a.length);
    }
    rx.reset();
    rx.write(a, 0, a.length);
  }

  public synchronized void send(byte[] d) {
    if (d.length > 1500) throw new IllegalArgumentException("Oversized datagram");
    for (int i = 0; i < d.length; i += 19) {
      if (pending.size() >= 512) throw new IllegalStateException("Gattlink output overflow");
      pending.add(Arrays.copyOfRange(d, i, Math.min(i + 19, d.length)));
    }
    flush();
  }

  private void flush() {
    while (active && flight.size() < 4 && !pending.isEmpty()) {
      byte[] body = pending.removeFirst(), p = new byte[body.length + 1];
      p[0] = (byte) tx;
      System.arraycopy(body, 0, p, 1, body.length);
      flight.put(tx, p);
      tx = (tx + 1) & 31;
      write.accept(p);
      sent = System.currentTimeMillis();
    }
  }

  public synchronized void tick(long now) {
    if (!flight.isEmpty() && now - sent >= 600) {
      if (++retries > 20) {
        flight.clear();
        failure.accept("Gattlink acknowledgement timeout");
        return;
      }
      for (byte[] p : flight.values()) write.accept(p);
      sent = now;
    }
  }
}
