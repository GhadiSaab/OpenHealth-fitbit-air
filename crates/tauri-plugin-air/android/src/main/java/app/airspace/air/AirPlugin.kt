package app.airspace.air

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.util.Base64
import androidx.activity.result.ActivityResult
import androidx.core.content.ContextCompat
import app.tauri.PermissionState
import app.tauri.annotation.*
import app.tauri.plugin.*
import org.json.JSONObject

@InvokeArg class AckArgs { var seq: Long = 0 }
@TauriPlugin(permissions = [
 Permission(alias = "nearby", strings = ["android.permission.BLUETOOTH_SCAN", "android.permission.BLUETOOTH_CONNECT"]),
 Permission(alias = "location", strings = ["android.permission.ACCESS_FINE_LOCATION"]),
 Permission(alias = "notifications", strings = ["android.permission.POST_NOTIFICATIONS"])
])
class AirPlugin(private val activity: Activity): Plugin(activity) {
 private val context get() = activity.applicationContext
 @Command fun status(invoke: Invoke) {
  try {
   val s = AirStore(context).use { it.status() }
   val secret = AirSecrets.load(context)
   s.put("configured", secret != null)
   s.put("timezone", java.util.TimeZone.getDefault().id)
   s.put("transport",AirService.transportInfo())
   if(secret != null) s.put("expires_at", secret.optLong("expires_at"))
   if(!AirService.active) {
    s.put("recording", false)
    if(s.optString("state") in listOf("connected", "connecting", "authenticating", "scanning", "gattlink", "reconnecting"))
     s.put("state", "disconnected").put("message", "Recording is stopped. Start it to reconnect.")
   }
   invoke.resolve(JSObject(s.toString()))
  } catch(e: Exception) { invoke.reject("Could not read native recording state") }
 }
 @Command fun importCredentials(invoke: Invoke) {
  if(AirService.active){ invoke.reject("Stop recording before replacing device credentials"); return }
  startActivityForResult(invoke,Intent(Intent.ACTION_OPEN_DOCUMENT).apply { addCategory(Intent.CATEGORY_OPENABLE); type="*/*" },"credentialsPicked")
 }
 @ActivityCallback fun credentialsPicked(invoke: Invoke,result: ActivityResult) {
  if(result.resultCode != Activity.RESULT_OK) { invoke.reject("Credential import cancelled"); return }
  try {
   val uri=result.data?.data ?: error("No document")
   require(uri.scheme=="content")
   val raw=context.contentResolver.openInputStream(uri)?.use { input ->
    val out=java.io.ByteArrayOutputStream(); val buf=ByteArray(4096)
    while(true){ val n=input.read(buf); if(n<0)break; if(out.size()+n>32768)error("Oversized document");out.write(buf,0,n) }
    out.toString("UTF-8")
   } ?: error("Unreadable document")
   val doc=JSONObject(raw)
   require(doc.getString("schema")=="airspace.air.credentials" && doc.getInt("version")==1)
   val id=doc.getString("identity"); require(id.matches(Regex("MD-[0-9A-F]{8}-[0-9A-F]{8}")))
   require(Base64.decode(doc.getString("key"),Base64.NO_WRAP).size==16)
   val address=doc.optString("address",""); require(address.isEmpty() || android.bluetooth.BluetoothAdapter.checkBluetoothAddress(address))
   val safe=JSONObject().put("identity",id).put("key",doc.getString("key")).put("address",address).put("expires_at",doc.optLong("expires_at"))
   AirSecrets.save(context,safe)
   invoke.resolve(JSObject().put("configured",true))
  } catch(e: Exception) { invoke.reject("Not a supported Air credential document; no credentials were changed") }
 }
 @Command fun start(invoke: Invoke) {
  val alias=if(Build.VERSION.SDK_INT>=31)"nearby" else "location"
  if(getPermissionState(alias)!=PermissionState.GRANTED){requestPermissionForAlias(alias,invoke,"afterNearby");return}
  afterNearby(invoke)
 }
 @PermissionCallback fun afterNearby(invoke: Invoke){
  val alias=if(Build.VERSION.SDK_INT>=31)"nearby" else "location"
  if(getPermissionState(alias)!=PermissionState.GRANTED){invoke.reject("Nearby devices permission was denied");return}
  if(Build.VERSION.SDK_INT>=33 && getPermissionState("notifications")!=PermissionState.GRANTED){requestPermissionForAlias("notifications",invoke,"afterNotification");return}
  afterNotification(invoke)
 }
 @PermissionCallback fun afterNotification(invoke: Invoke){
  try{require(AirSecrets.load(context)!=null);ContextCompat.startForegroundService(context,Intent(context,AirService::class.java).putExtra("epoch",AirService.commandEpoch.incrementAndGet()));invoke.resolve(JSObject().put("requested",true))}
  catch(e:Exception){invoke.reject("Could not start recording; import credentials and check permissions")}
 }
 @Command fun stop(invoke: Invoke){
  AirService.commandEpoch.incrementAndGet()
  context.stopService(Intent(context,AirService::class.java))
  val handler=android.os.Handler(android.os.Looper.getMainLooper()); val deadline=System.currentTimeMillis()+5000
  val wait=object:Runnable { override fun run(){ if(!AirService.active) invoke.resolve() else if(System.currentTimeMillis()>deadline)invoke.reject("Recorder has not stopped yet") else handler.postDelayed(this,50) } };handler.post(wait)
 }
 @Command fun clearPending(invoke: Invoke){if(AirService.active){invoke.reject("Recorder must be stopped first");return};AirStore(context).use{it.clear()};invoke.resolve()}
 @Command fun drain(invoke: Invoke){try{val rows=AirStore(context).use { it.drain() };invoke.resolve(JSObject().put("rows",rows))}catch(e:Exception){invoke.reject("Could not read pending samples")}}
 @Command fun ack(invoke: Invoke){try{val seq=invoke.parseArgs(AckArgs::class.java).seq;require(seq>=0);AirStore(context).use{it.ack(seq)};invoke.resolve()}catch(e:Exception){invoke.reject("Could not acknowledge committed samples")}}
 @Command fun forget(invoke: Invoke){if(AirService.active){invoke.reject("Stop recording before removing the device");return};AirSecrets.delete(context);invoke.resolve()}
}
