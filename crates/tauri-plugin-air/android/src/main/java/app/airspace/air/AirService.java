package app.airspace.air;

import android.app.*;
import android.bluetooth.*;
import android.bluetooth.le.*;
import android.content.*;
import android.content.pm.ServiceInfo;
import android.os.*;
import android.util.Base64;
import java.util.*;
import org.json.*;

/** User-started foreground recorder. The WebView never owns the radio or spool. */
public final class AirService extends Service {
  public static volatile boolean active = false;
  static volatile AirService current;
  static JSONObject transportInfo(){AirService s=current;return s!=null&&s.link!=null?s.link.diagnostics():new JSONObject();}
  public static final java.util.concurrent.atomic.AtomicLong commandEpoch =
      new java.util.concurrent.atomic.AtomicLong();
  private final Handler h = new Handler(Looper.getMainLooper());
  private static final UUID SVC = UUID.fromString("abbaff00-e56a-484c-b832-8b17cf6cbfe8"),
      TX = UUID.fromString("abbaff01-e56a-484c-b832-8b17cf6cbfe8"),
      RX = UUID.fromString("abbaff02-e56a-484c-b832-8b17cf6cbfe8"),
      CCCD = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb"),
      CACHE_READ = UUID.fromString("ac2f0145-8182-4be5-91e0-2992e6b40ebb"),
      CACHE_WRITE = UUID.fromString("ac2f2845-8182-4be5-91e0-2992e6b40ebb");
  private AirStore store;
  private JSONObject status, credentials;
  private BluetoothGatt gatt;
  private BluetoothLeScanner scanner;
  private AirSession link;
  private BluetoothGattCharacteristic tx;
  private final ArrayDeque<Runnable> operations = new ArrayDeque<>();
  private boolean busy = false, linked = false, attemptHasReading = false;
  private int attempt = 0, generation = 0, segment = 0;
  private int negotiatedMtu=23;
  private boolean mtuPending=false;
  private String session;
  private long lastReceived = 0, pendingGap = 0;
  private PowerManager.WakeLock wake;
  private final Runnable opTimeout = () -> {
    if(mtuPending){mtuPending=false;done();}else retry("Bluetooth operation timed out");
  };
  private final Runnable timer =
      new Runnable() {
        public void run() {
          if (!active) return;
          if (link != null) link.gl.tick(System.currentTimeMillis());
          if (wake != null && !wake.isHeld()) wake.acquire(600000L);
          if (attemptHasReading
              && System.currentTimeMillis() - lastReceived > 10000
              && status.optBoolean("recording")) state("connected", "Waiting for fresh readings");
          h.postDelayed(this, 100);
        }
      };

  public IBinder onBind(Intent i) {
    return null;
  }

  public void onCreate() {
    super.onCreate();
    current=this;
    store = new AirStore(this);
    status = store.status();
    NotificationManager nm = getSystemService(NotificationManager.class);
    nm.createNotificationChannel(
        new NotificationChannel(
            "air_recording", "Tracker recording", NotificationManager.IMPORTANCE_LOW));
    wake =
        ((PowerManager) getSystemService(POWER_SERVICE))
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "Airspace:Recording");
  }

  public int onStartCommand(Intent intent, int flags, int id) {
    if (intent != null && "STOP".equals(intent.getAction())) {
      stopRecording();
      return START_NOT_STICKY;
    }
    if (active) return START_NOT_STICKY;
    if (intent == null || intent.getLongExtra("epoch", -1) != commandEpoch.get()) {
      stopSelf();
      return START_NOT_STICKY;
    }
    try {
      if (Build.VERSION.SDK_INT >= 29)
        startForeground(
            4201,
            notification("Connecting to your Air"),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE);
      else startForeground(4201, notification("Connecting to your Air"));
      credentials = AirSecrets.load(this);
      if (credentials == null) throw new IllegalStateException("Import your Air credentials first");
      active = true;
      session = UUID.randomUUID().toString();
      status =
          new JSONObject()
              .put("state", "connecting")
              .put("recording", false)
              .put("session", session)
              .put("samples", 0)
              .put("started_at", System.currentTimeMillis())
              .put("gaps", new JSONArray());
      store.status(status);
      h.post(timer);
      connect();
    } catch (Exception e) {
      state("error", "Could not start recording. Check permissions and device credentials.");
      stopSelf();
    }
    return START_NOT_STICKY;
  }

  private Notification notification(String text) {
    Intent launch = getPackageManager().getLaunchIntentForPackage(getPackageName());
    PendingIntent open =
        PendingIntent.getActivity(
            this, 0, launch, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    PendingIntent stop =
        PendingIntent.getService(
            this,
            1,
            new Intent(this, AirService.class).setAction("STOP"),
            PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    return new Notification.Builder(this, "air_recording")
        .setContentTitle("Airspace")
        .setContentText(text)
        .setSmallIcon(android.R.drawable.stat_sys_data_bluetooth)
        .setOngoing(true)
        .setContentIntent(open)
        .addAction(new Notification.Action.Builder(null, "Stop", stop).build())
        .build();
  }

  private void state(String value, String message) {
    try {
      status
          .put("state", value)
          .put("message", message)
          .put(
              "recording",
              active
                  && linked
                  && attemptHasReading
                  && System.currentTimeMillis() - lastReceived <= 10000
                  && "connected".equals(value))
          .put("updated_at", System.currentTimeMillis());
      store.status(status);
      getSystemService(NotificationManager.class).notify(4201, notification(message));
    } catch (Exception ignored) {
    }
  }

  private void connect() {
    if (!active) return;
    final int gen = generation;
    try {
      BluetoothAdapter a = getSystemService(BluetoothManager.class).getAdapter();
      if (a == null || !a.isEnabled()) {
        state("bluetooth_off", "Bluetooth is off. Turn it on, then start recording again.");
        stopSelf();
        return;
      }
      String address = credentials.optString("address", "");
      if (!address.isEmpty()) {
        connectDevice(a.getRemoteDevice(address));
        return;
      }
      state("scanning", "Looking for your Air");
      scanner = a.getBluetoothLeScanner();
      scanner.startScan(
          Collections.singletonList(
              new ScanFilter.Builder().setServiceUuid(new ParcelUuid(SVC)).build()),
          new ScanSettings.Builder().setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY).build(),
          scan);
      h.postDelayed(
          () -> {
            if (active && generation == gen && gatt == null)
              retry(
                  "Air was not found. Keep it nearby and release its connection in Google Health.");
          },
          12000);
    } catch (SecurityException e) {
      state("permission_denied", "Nearby devices permission is required");
      stopSelf();
    } catch (Exception e) {
      retry("Could not connect to your Air");
    }
  }

  private final ScanCallback scan =
      new ScanCallback() {
        public void onScanResult(int type, ScanResult r) {
          h.post(() -> {
            if (!active || gatt != null) return;
            stopScan();
            connectDevice(r.getDevice());
          });
        }

        public void onScanFailed(int error) {
          h.post(() -> retry("Bluetooth scan failed"));
        }
      };

  private void stopScan() {
    try {
      if (scanner != null) scanner.stopScan(scan);
    } catch (SecurityException ignored) {
    }
    scanner = null;
  }

  private void connectDevice(BluetoothDevice d) {
    if (!active) return;
    final int gen = generation;
    state("connecting", "Connecting to your Air");
    gatt = d.connectGatt(this, false, callback, BluetoothDevice.TRANSPORT_LE);
    h.postDelayed(
        () -> {
          if (active && generation == gen && !linked && gatt != null)
            retry("Air authentication timed out");
        },
        25000);
  }

  private void enqueue(Runnable r) {
    operations.add(r);
    pump();
  }

  private void pump() {
    if (busy || operations.isEmpty() || gatt == null) return;
    busy = true;
    h.postDelayed(opTimeout, 5000);
    try {
      operations.removeFirst().run();
    } catch (Exception e) {
      retry("Bluetooth operation failed");
    }
  }

  private void done() {
    h.removeCallbacks(opTimeout);
    busy = false;
    pump();
  }

  private BluetoothGattCharacteristic find(UUID id) {
    for (BluetoothGattService s : gatt.getServices()) {
      BluetoothGattCharacteristic c = s.getCharacteristic(id);
      if (c != null) return c;
    }
    return null;
  }

  private void configure() {
    BluetoothGattService s = gatt.getService(SVC);
    if (s == null) {
      retry("Air protocol service not found");
      return;
    }
    tx = s.getCharacteristic(TX);
    BluetoothGattCharacteristic rx = s.getCharacteristic(RX);
    if (tx == null || rx == null) {
      retry("Air protocol characteristics missing");
      return;
    }
    enqueue(() -> {mtuPending=true;if(!gatt.requestMtu(247)){mtuPending=false;done();}});
    BluetoothGattCharacteristic cr = find(CACHE_READ), cw = find(CACHE_WRITE);
    if (cr != null)
      enqueue(() -> {
        if (!gatt.readCharacteristic(cr)) retry("Cache read failed");
      });
    if (cw != null)
      enqueue(() -> {
        cw.setWriteType(BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT);
        cw.setValue(new byte[] {0});
        if (!gatt.writeCharacteristic(cw)) retry("Cache confirmation failed");
      });
    enqueue(() -> {
      if (!gatt.setCharacteristicNotification(rx, true)) throw new IllegalStateException();
      BluetoothGattDescriptor desc = rx.getDescriptor(CCCD);
      if (desc == null) throw new IllegalStateException();
      desc.setValue(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE);
      if (!gatt.writeDescriptor(desc)) throw new IllegalStateException();
    });
  }

  private void beginProtocol() {
    if (link != null || !active) return;
    final int gen = generation;
    segment++;
    state("gattlink", "Starting the secure Air link");
    link =
        new AirSession(
            bytes ->
                h.post(() -> {
                  if (active && generation == gen && gatt != null) {
                    if (operations.size() > 512) {
                      retry("Bluetooth output overflow");
                      return;
                    }
                    enqueue(() -> {
                      tx.setWriteType(BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE);
                      tx.setValue(bytes);
                      if (!gatt.writeCharacteristic(tx)) retry("Tracker write failed");
                    });
                  }
                }),
            fields ->
                h.post(() -> {
                  if (generation == gen) reading(fields);
                }),
            s ->
                h.post(() -> {
                  if (!active || generation != gen) return;
                  if ("connected".equals(s)) {
                    linked = true;
                    state(s, "Secure link ready; waiting for readings");
                  } else
                    state(
                        s,
                        "authenticating".equals(s)
                            ? "Authenticating your Air"
                            : "Secure link failed");
                }),
            () ->
                h.post(() -> {
                  if (generation == gen) retry("Secure link disconnected");
                }));
    try {
      link.gl.setMtu(negotiatedMtu);
      link.start(
          credentials.getString("identity").getBytes("US-ASCII"),
          Base64.decode(credentials.getString("key"), 2));
    } catch (Exception e) {
      retry("Device credentials could not be used");
    }
  }

  private final BluetoothGattCallback callback =
      new BluetoothGattCallback() {
        public void onConnectionStateChange(BluetoothGatt g, int statusCode, int next) {
          h.post(() -> {
            if (g != gatt) return;
            if (statusCode == 0 && next == BluetoothProfile.STATE_CONNECTED) {
              state("discovering", "Discovering Air services");
              if (!g.discoverServices()) retry("Service discovery failed");
            } else if (next == BluetoothProfile.STATE_DISCONNECTED) retry("Air disconnected");
          });
        }

        public void onMtuChanged(BluetoothGatt g,int mtu,int code){
          h.post(() -> {
            if(g!=gatt)return;
            if(code==0 && mtu>=23 && mtu<=517){negotiatedMtu=mtu;if(link!=null)link.gl.setMtu(mtu);}
            if(mtuPending){mtuPending=false;done();}
          });
        }
        public void onServicesDiscovered(BluetoothGatt g, int code) {
          h.post(() -> {
            if (g != gatt) return;
            if (code == 0) configure();
            else retry("Service discovery failed");
          });
        }

        public void onCharacteristicRead(BluetoothGatt g, BluetoothGattCharacteristic c, int code) {
          h.post(() -> {
            if (g == gatt) {
              if (code == 0) done();
              else retry("Cache read rejected");
            }
          });
        }

        public void onCharacteristicWrite(
            BluetoothGatt g, BluetoothGattCharacteristic c, int code) {
          h.post(() -> {
            if (g == gatt) {
              if (code == 0) done();
              else retry("Tracker write rejected");
            }
          });
        }

        public void onDescriptorWrite(BluetoothGatt g, BluetoothGattDescriptor d, int code) {
          h.post(() -> {
            if (g != gatt) return;
            if (code == 0) {
              done();
              beginProtocol();
            } else retry("Notifications rejected");
          });
        }

        public void onCharacteristicChanged(
            BluetoothGatt g, BluetoothGattCharacteristic c, byte[] value) {
          byte[] copy = value.clone();
          h.post(() -> {
            if (g == gatt && link != null) link.gl.notification(copy);
          });
        }

        public void onCharacteristicChanged(BluetoothGatt g, BluetoothGattCharacteristic c) {
          if (Build.VERSION.SDK_INT < 33) onCharacteristicChanged(g, c, c.getValue());
        }
      };

  private void reading(Map<Integer, Long> fields) {
    if (!active || !linked) return;
    try {
      long now = System.currentTimeMillis();
      JSONObject f = new JSONObject();
      for (Map.Entry<Integer, Long> e : fields.entrySet())
        f.put(e.getKey().toString(), e.getValue());
      if (lastReceived > 0 && now - lastReceived > 10000 && pendingGap == 0) {
        pendingGap = lastReceived;
        segment++;
      }
      attemptHasReading = true;
      JSONObject raw =
          new JSONObject()
              .put("session", session)
              .put("segment", segment)
              .put("received_at", now)
              .put("utc_offset_min", TimeZone.getDefault().getOffset(now) / 60000)
              .put("fields", f);
      status
          .put("last_reading", raw)
          .put("last_received", now)
          .put("recording", true)
          .put("samples", status.optLong("samples") + 1)
          .put("state", "connected")
          .put("message", "Recording live readings");
      if (pendingGap > 0)
        status
            .getJSONArray("gaps")
            .put(
                new JSONObject()
                    .put("start", pendingGap)
                    .put("end", now)
                    .put("reason", "connection_lost"));
      pendingGap = 0;
      store.add(raw, status);
      lastReceived = now;
      attempt = 0;
      getSystemService(NotificationManager.class)
          .notify(4201, notification("Recording live readings"));
    } catch (Exception e) {
      state("storage_error", "Recording stopped because readings could not be saved");
      stopSelf();
    }
  }

  private void cleanupLink() {
    if (attemptHasReading && pendingGap == 0) pendingGap = lastReceived;
    attemptHasReading = false;
    generation++;
    try{if(link!=null)status.put("last_transport",link.diagnostics());}catch(org.json.JSONException ignored){}
    stopScan();
    h.removeCallbacks(opTimeout);
    busy = false;
    mtuPending=false;negotiatedMtu=23;
    operations.clear();
    BluetoothGatt old = gatt;
    gatt = null;
    AirSession l = link;
    link = null;
    linked = false;
    if (l != null) l.close();
    if (old != null) {
      try {
        old.disconnect();
        old.close();
      } catch (SecurityException ignored) {
      }
    }
  }

  private void retry(String message) {
    if (!active) return;
    cleanupLink();
    if (++attempt > 6) {
      state("error", message + ". Open Airspace and try again.");
      stopSelf();
      return;
    }
    state("reconnecting", message + ". Reconnecting…");
    h.postDelayed(
        () -> {
          if (active && gatt == null && scanner == null) connect();
        },
        Math.min(30000, 1000L << Math.min(attempt, 5)));
  }

  private void stopRecording() {
    commandEpoch.incrementAndGet();
    active = false;
    cleanupLink();
    h.removeCallbacksAndMessages(null);
    state("disconnected", "Recording stopped");
    stopSelf();
  }

  public void onDestroy() {
    current=null;
    active = false;
    cleanupLink();
    h.removeCallbacksAndMessages(null);
    try {
      status.put("recording", false);
      if ("connected".equals(status.optString("state")))
        status.put("state", "disconnected").put("message", "Recording interrupted");
      store.status(status);
    } catch (Exception ignored) {
    }
    if (wake != null && wake.isHeld()) wake.release();
    stopForeground(STOP_FOREGROUND_REMOVE);
    store.close();
    super.onDestroy();
  }
}
