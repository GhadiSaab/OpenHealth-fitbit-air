package app.airspace.air.protocol;

import java.util.*;

public final class ProtocolTest {
  static void check(boolean x, String m) {
    if (!x) throw new AssertionError(m);
  }

  static void rejects(Runnable f) {
    try {
      f.run();
      throw new AssertionError("accepted malformed input");
    } catch (IllegalArgumentException expected) {
    }
  }

  public static void main(String[] args) throws Exception {
    byte[] payload = {8, 1, 16, 25, 24, (byte) 0x8d, (byte) 0x95, 1, 32, 38, 56, 75, 64, 2};
    Map<Integer, Long> r = AirWire.protobuf(payload);
    check(r.get(2) == 25 && r.get(7) == 75 && r.get(3) == 19085, "protobuf fields");
    rejects(() -> AirWire.protobuf(new byte[] {8, (byte) 0x80}));
    rejects(() -> AirWire.protobuf(new byte[] {0}));
    byte[] coap = AirWire.coap(1, 123, new byte[] {1, 2, 3, 4}, "liveactivity", new byte[0], 0);
    AirWire.Coap c = AirWire.parseCoap(coap);
    check(c.path.equals("liveactivity") && c.mid == 123 && c.code == 1, "CoAP request");
    rejects(() -> AirWire.parseCoap(new byte[] {0x4f, 1, 0, 0}));
    byte[] ip = AirWire.ipUdp(new byte[] {1, 2, 3}, 7, false);
    check(Arrays.equals(AirWire.stripIpUdp(ip), new byte[] {1, 2, 3}), "IP/UDP");
    ip[2] = 0;
    ip[3] = 1;
    rejects(() -> AirWire.stripIpUdp(ip));
    List<byte[]> out = new ArrayList<>(), got = new ArrayList<>();
    Gattlink g =
        new Gattlink(
            out::add,
            got::add,
            () -> {},
            m -> {
              throw new AssertionError(m);
            });
    g.notification(new byte[] {(byte) 0x81, 0, 0, 0x20, 0x20});
    byte[] packet = AirWire.ipUdp(new byte[] {5, 6}, 8, false);
    byte[] first = new byte[11];
    first[0] = 0;
    System.arraycopy(packet, 0, first, 1, 10);
    g.notification(first);
    g.notification(first);
    byte[] second = new byte[packet.length - 10 + 1];
    second[0] = 1;
    System.arraycopy(packet, 10, second, 1, packet.length - 10);
    g.notification(second);
    check(got.size() == 1, "duplicate fragment delivered twice");
    g.notification(new byte[] {3, 1});
    check(out.get(out.size() - 1)[0] == 0x41, "gap re-ACK");
    List<byte[]> tx = new ArrayList<>();
    Gattlink t = new Gattlink(tx::add, x -> {}, () -> {}, m -> {});
    t.notification(new byte[] {(byte) 0x81});
    t.send(packet);
    int before = tx.size();
    t.tick(System.currentTimeMillis() + 1000);
    check(tx.size() > before, "retransmission missing");
    for (int i = 0; i < 40; i++) {
      t.notification(new byte[] {(byte) (0x40 | (i & 31))});
      t.send(new byte[] {1});
    }
    check(tx.stream().allMatch(x -> x.length <= 20), "oversized ATT fragment");
    CoapPoll poll = new CoapPoll();
    byte[] request = poll.poll(0);
    check(poll.poll(1500) == null, "replaced outstanding CoAP token");
    check(Arrays.equals(request, poll.poll(3000)), "retransmission changed the CoAP request");
    AirWire.Coap req = AirWire.parseCoap(request);
    AirWire.Coap response =
        AirWire.parseCoap(AirWire.coap(69, req.mid, req.token, "", new byte[] {8, 1}, 2));
    check(poll.complete(response), "late response discarded");
    check(poll.poll(4000) != null, "polling did not resume");
    System.out.println("Protocol tests passed");
  }
}
