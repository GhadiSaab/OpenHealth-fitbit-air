package app.airspace.air.protocol;

import java.io.IOException;
import java.util.Arrays;

/** A single outstanding CON request, preserving its token and MID on retry. */
public final class CoapPoll {
  private byte[] request, token;
  private int mid = 1, retries = 0;
  private long due = 0, sent = 0;

  public byte[] poll(long now) throws IOException {
    if (request == null) {
      if (now < due) return null;
      token = new byte[] {(byte) (mid >> 8), (byte) mid, 0x44, 0x40};
      request = AirWire.coap(1, mid++, token, "liveactivity", new byte[0], 0);
      retries = 0;
      sent = now;
      return request.clone();
    }
    if (now - sent < 3000) return null;
    if (++retries > 3) throw new IOException("Live request timed out");
    sent = now;
    return request.clone();
  }

  public boolean complete(AirWire.Coap response) {
    if (request == null || response.code < 64 || !Arrays.equals(token, response.token))
      return false;
    request = null;
    token = null;
    due = sent + 1000;
    return true;
  }
}
