package app.airspace.air;

import android.content.*;
import android.database.Cursor;
import android.database.sqlite.*;
import org.json.*;

/** Crash-durable spool. Native Rust acknowledges rows only after its transaction commits. */
final class AirStore extends SQLiteOpenHelper {
  AirStore(Context c) {
    super(c, "airspace-air-spool.sqlite", null, 1);
  }

  public void onCreate(SQLiteDatabase d) {
    d.execSQL("CREATE TABLE readings(seq INTEGER PRIMARY KEY AUTOINCREMENT,json TEXT NOT NULL)");
    d.execSQL("CREATE TABLE state(id INTEGER PRIMARY KEY,json TEXT NOT NULL)");
  }

  public void onUpgrade(SQLiteDatabase d, int old, int next) {
    throw new IllegalStateException("Unsupported spool migration");
  }

  synchronized JSONObject status() {
    try (Cursor c = getReadableDatabase().rawQuery("SELECT json FROM state WHERE id=1", null)) {
      if (c.moveToFirst()) return new JSONObject(c.getString(0));
      return new JSONObject()
          .put("state", "disconnected")
          .put("recording", false)
          .put("samples", 0);
    } catch (JSONException e) {
      throw new IllegalStateException(e);
    }
  }

  synchronized void status(JSONObject v) {
    ContentValues row = new ContentValues();
    row.put("id", 1);
    row.put("json", v.toString());
    getWritableDatabase().insertWithOnConflict("state", null, row, SQLiteDatabase.CONFLICT_REPLACE);
  }

  synchronized void add(JSONObject v, JSONObject state) {
    SQLiteDatabase d = getWritableDatabase();
    d.beginTransaction();
    try {
      try (Cursor c = d.rawQuery("SELECT COUNT(*) FROM readings", null)) {
        c.moveToFirst();
        if (c.getLong(0) >= 200000)
          throw new IllegalStateException(
              "Recording storage is full. Open Airspace to save pending readings.");
      }
      ContentValues row = new ContentValues();
      row.put("json", v.toString());
      if (d.insertOrThrow("readings", null, row) < 0)
        throw new IllegalStateException("Recording storage failed");
      status(state);
      d.setTransactionSuccessful();
    } finally {
      d.endTransaction();
    }
  }

  synchronized JSONArray drain() {
    JSONArray a = new JSONArray();
    try (Cursor c =
        getReadableDatabase()
            .rawQuery("SELECT seq,json FROM readings ORDER BY seq LIMIT 256", null)) {
      while (c.moveToNext()) {
        try {
          a.put(new JSONObject(c.getString(1)).put("seq", c.getLong(0)));
        } catch (JSONException e) {
          throw new IllegalStateException(e);
        }
      }
    }
    return a;
  }

  synchronized void ack(long seq) {
    getWritableDatabase().delete("readings", "seq <= ?", new String[] {Long.toString(seq)});
  }

  synchronized void clear() {
    SQLiteDatabase d = getWritableDatabase();
    d.beginTransaction();
    try {
      d.delete("readings", null, null);
      d.delete("state", null, null);
      d.setTransactionSuccessful();
    } finally {
      d.endTransaction();
    }
  }
}
