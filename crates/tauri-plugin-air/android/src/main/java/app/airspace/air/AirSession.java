package app.airspace.air;

import app.airspace.air.protocol.*;
import java.io.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.function.Consumer;
import org.bouncycastle.tls.*;

/** One link attempt, owned by the foreground service. No historical writes. */
final class AirSession implements DatagramTransport, AutoCloseable {
  final Gattlink gl;
  private volatile long ipInput=0,dtlsInput=0,dtlsOutput=0;
  private volatile int lastType=0;
  private final BlockingQueue<byte[]> input = new ArrayBlockingQueue<>(128);
  private final CountDownLatch ready = new CountDownLatch(1);
  private volatile boolean closed;
  private int ipId = 1, mid = 1;
  private volatile Thread thread;
  private final Consumer<Map<Integer, Long>> sample;
  private final Consumer<String> state;
  private final Runnable failed;

  AirSession(
      Consumer<byte[]> writer,
      Consumer<Map<Integer, Long>> sample,
      Consumer<String> state,
      Runnable failed) {
    this.sample = sample;
    this.state = state;
    this.failed = failed;
    gl =
        new Gattlink(
            writer,
            ip -> {
              try {
                ipInput++;
                byte[] payload=AirWire.stripIpUdp(ip);
                if(payload.length>0)lastType=payload[0]&255;
                if (!input.offer(payload)) abort();
              } catch (IllegalArgumentException e) {
                abort();
              }
            },
            ready::countDown,
            e -> abort());
  }

  void start(byte[] identity, byte[] key) {
    gl.reset();
    thread =
        new Thread(
            () -> {
              try {
                if (!ready.await(8, TimeUnit.SECONDS))
                  throw new IOException("Gattlink handshake timeout");
                state.accept("authenticating");
                DTLSTransport dtls = AirDtls.accept(this, identity, key);
                state.accept("connected");
                long last = System.currentTimeMillis();
                CoapPoll polling = new CoapPoll();
                byte[] rx = new byte[4096];
                while (!closed) {
                  long now = System.currentTimeMillis();
                  byte[] get = polling.poll(now);
                  if (get != null) dtls.send(get, 0, get.length);
                  int n = dtls.receive(rx, 0, rx.length, 200);
                  if (n > 0) {
                    AirWire.Coap c;
                    try {
                      c = AirWire.parseCoap(Arrays.copyOf(rx, n));
                    } catch (IllegalArgumentException bad) {
                      continue;
                    }
                    if (c.code >= 64 && c.type == 0) {
                      byte[] ack = AirWire.coap(0, c.mid, new byte[0], "", new byte[0], 2);
                      dtls.send(ack, 0, ack.length);
                    }
                    if (c.code >= 1 && c.code <= 4) {
                      int code = 132; // 4.04 for unknown resources
                      if (c.code == 2
                          && (c.path.equals("events")
                              || c.path.equals("sync/request")
                              || c.path.equals("aspen/info")))
                        code = 68; // acknowledge receipt only; never sync-complete
                      byte[] reply =
                          AirWire.coap(code, c.mid, c.token, "", new byte[0], c.type == 0 ? 2 : 1);
                      dtls.send(reply, 0, reply.length);
                    } else if (polling.complete(c)) {
                      if (c.code != 69) throw new IOException("Live resource rejected");
                      Map<Integer, Long> fields = AirWire.protobuf(c.body);
                      if (!fields.isEmpty()) {
                        sample.accept(fields);
                        last = System.currentTimeMillis();
                      }
                    }
                  }
                  if (System.currentTimeMillis() - last > 20000)
                    throw new IOException("Live readings timed out");
                }
              } catch (InterruptedException ignored) {
                Thread.currentThread().interrupt();
              } catch (Exception e) {
                if (!closed) {
                  android.util.Log.w("AirspaceLink","Protocol failure: "+e.getClass().getSimpleName()+(e instanceof TlsFatalAlert ? " alert="+((TlsFatalAlert)e).getAlertDescription() : ""));
                  state.accept("protocol_error");
                }
              } finally {
                Arrays.fill(key, (byte) 0);
                if (!closed) {
                  closed = true;
                  failed.run();
                }
              }
            },
            "Airspace-DTLS");
    thread.start();
  }

  public int getReceiveLimit() {
    return 1400;
  }

  public int getSendLimit() {
    return 1400;
  }

  public int receive(byte[] b, int off, int len, int wait) throws IOException {
    try {
      byte[] p = input.poll(Math.max(1, wait), TimeUnit.MILLISECONDS);
      if (closed) throw new IOException("closed");
      if (p == null) return -1;
      dtlsInput++;
      if (p.length > len) throw new IOException("Datagram too large");
      System.arraycopy(p, 0, b, off, p.length);
      return p.length;
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      throw new IOException("closed");
    }
  }

  public void send(byte[] b, int off, int len) throws IOException {
    if (closed) throw new IOException("closed");
    dtlsOutput++;
    gl.send(AirWire.ipUdp(Arrays.copyOfRange(b, off, off + len), ipId++ & 65535, true));
  }

  org.json.JSONObject diagnostics(){
    org.json.JSONObject o=new org.json.JSONObject();
    try{o.put("ip_input",ipInput).put("dtls_input",dtlsInput).put("dtls_output",dtlsOutput).put("last_record_type",lastType).put("thread",thread==null?"none":thread.getState().toString());}catch(org.json.JSONException ignored){}
    return o;
  }
  private void abort() {
    if (closed) return;
    close();
    failed.run();
  }

  public void close() {
    closed = true;
    input.offer(new byte[0]);
    Thread t = thread;
    if (t != null) t.interrupt();
  }
}
