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
  private enum ResetState { IDLE, SELF, REMOTE, READY, FAILED }
  private ResetState resetState=ResetState.IDLE;
  private long controlSent=0,resetRequests=0,resetCompletes=0;
  private int controlAttempts=0,transmitWindow=4;
  private void clearFraming(){tx=expected=retries=0;rx.reset();pending.clear();flight.clear();}
  private void sendResetComplete(){controlSent=System.currentTimeMillis();transmit(new byte[]{(byte)0x81,0,0,32,4});}

  private int fragmentBodyLimit=19;
  public synchronized void setMtu(int mtu){
    if(mtu<23 || mtu>517)throw new IllegalArgumentException("Invalid ATT MTU");
    fragmentBodyLimit=mtu-4;
  }
  private long receivedPackets, sentPackets, ackPackets, matchedAcks, retransmittedPackets;
  private int lastAck=-1;
  private void transmit(byte[] p){sentPackets++;write.accept(p);}
  public synchronized Map<String,Long> diagnostics(){
    Map<String,Long> m=new LinkedHashMap<>();
    m.put("reset_requests",resetRequests);m.put("reset_completes",resetCompletes);m.put("fragment_body_limit",(long)fragmentBodyLimit);m.put("rx_packets",receivedPackets);m.put("tx_packets",sentPackets);
    m.put("ack_packets",ackPackets);m.put("matched_acks",matchedAcks);
    m.put("last_ack",(long)lastAck);m.put("unacked",(long)flight.size());
    m.put("queued",(long)pending.size());m.put("retransmitted",retransmittedPackets);
    m.put("tx_next",(long)tx);m.put("rx_next",(long)expected);return m;
  }

  public Gattlink(Consumer<byte[]> w, Consumer<byte[]> d, Runnable r, Consumer<String> f) {
    write = w;
    datagram = d;
    ready = r;
    failure = f;
  }

  public synchronized void reset() {
    active=false;clearFraming();resetState=ResetState.SELF;controlAttempts=0;
    controlSent=System.currentTimeMillis();transmit(new byte[]{(byte)0x80});
  }

  public synchronized void notification(byte[] b) {
    if (b.length == 0) return;
    receivedPackets++;
    int h = b[0] & 255;
    if ((h & 128) != 0) {
      int cmd = h & 127;
      if (cmd == 0) {
        resetRequests++;
        if(active){active=false;resetState=ResetState.FAILED;clearFraming();failure.accept("Gattlink peer reset; discard DTLS session");return;}
        active=false;clearFraming();resetState=ResetState.REMOTE;controlAttempts=0;sendResetComplete();
      } else if (cmd == 1) {
        resetCompletes++;
        if(resetState!=ResetState.SELF && resetState!=ResetState.REMOTE)return;
        if(b.length<5 || (b[1]&255)!=0 || (b[3]&255)<1 || (b[3]&255)>32 || (b[4]&255)<1 || (b[4]&255)>32)return;
        transmitWindow=Math.min(4,b[3]&255);
        if(resetState==ResetState.SELF)sendResetComplete();
        tx=expected=retries=0;rx.reset();flight.clear();
        resetState=ResetState.READY;active=true;ready.run();flush();
      }
      return;
    }
    if(!active)return;
    int off = 0;
    if ((h & 64) != 0) {
      int ack = h & 31;
      ackPackets++; lastAck=ack;
      if (flight.containsKey(ack)) {
        matchedAcks++;
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
      transmit(new byte[] {(byte) (0x40 | ((expected + 31) & 31))});
      return;
    }
    expected = (expected + 1) & 31;
    transmit(new byte[] {(byte) (0x40 | seq)});
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
    if(resetState==ResetState.FAILED)throw new IllegalStateException("Gattlink session was reset");
    if (d.length > 1500) throw new IllegalArgumentException("Oversized datagram");
    for (int i = 0; i < d.length; i += fragmentBodyLimit) {
      if (pending.size() >= 512) throw new IllegalStateException("Gattlink output overflow");
      pending.add(Arrays.copyOfRange(d, i, Math.min(i + fragmentBodyLimit, d.length)));
    }
    flush();
  }

  private void flush() {
    while (active && flight.size() < transmitWindow && !pending.isEmpty()) {
      byte[] body = pending.removeFirst(), p = new byte[body.length + 1];
      p[0] = (byte) tx;
      System.arraycopy(body, 0, p, 1, body.length);
      flight.put(tx, p);
      tx = (tx + 1) & 31;
      transmit(p);
      sent = System.currentTimeMillis();
    }
  }

  public synchronized void tick(long now) {
    if(!active && (resetState==ResetState.SELF || resetState==ResetState.REMOTE) && now-controlSent>=1000){
      if(++controlAttempts>7){resetState=ResetState.FAILED;failure.accept("Gattlink control reset timed out");return;}
      clearFraming();resetState=ResetState.SELF;controlSent=now;transmit(new byte[]{(byte)0x80});return;
    }
    if (!flight.isEmpty() && now - sent >= 600) {
      if (++retries > 20) {
        flight.clear();
        failure.accept("Gattlink acknowledgement timeout");
        return;
      }
      for (byte[] p : flight.values()) { retransmittedPackets++; transmit(p); }
      sent = now;
    }
  }
}
