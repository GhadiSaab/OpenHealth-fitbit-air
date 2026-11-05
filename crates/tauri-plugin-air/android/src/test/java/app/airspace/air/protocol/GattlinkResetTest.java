package app.airspace.air.protocol;
import java.util.*;
/** Asymmetric peer fixture implementing the upstream reset contract, not a second Java endpoint. */
public final class GattlinkResetTest {
 static final byte[] COMPLETE={(byte)0x81,0,0,32,32};
 static void check(boolean value,String message){if(!value)throw new AssertionError(message);}
 public static void main(String[] args){
  List<byte[]> sent=new ArrayList<>();int[] ready={0},failed={0};
  Gattlink g=new Gattlink(sent::add,x->{},()->ready[0]++,m->failed[0]++);
  g.reset();check(sent.size()==1&&(sent.get(0)[0]&255)==128,"missing reset request");
  g.notification(COMPLETE);
  check(sent.size()==2&&(sent.get(1)[0]&255)==129,"peer never receives final RESET_COMPLETE");
  check(ready[0]==1,"ready callback missing");
  g.notification(COMPLETE);check(sent.size()==2&&ready[0]==1,"duplicate complete causes echo loop");
  g.notification(new byte[]{(byte)0x80});check(failed[0]==1,"old DTLS context survives remote reset");
  int before=sent.size();try{g.send(new byte[]{1,2});}catch(IllegalStateException expected){}check(sent.size()==before,"data leaked after reset invalidated session");
  List<byte[]> retry=new ArrayList<>();Gattlink r=new Gattlink(retry::add,x->{},()->{},m->{});
  r.reset();r.tick(System.currentTimeMillis()+1500);check(retry.size()==2&&(retry.get(1)[0]&255)==128,"lost control request not retransmitted");
  List<byte[]> remote=new ArrayList<>();int[] remoteReady={0};Gattlink other=new Gattlink(remote::add,x->{},()->remoteReady[0]++,m->{});
  other.notification(new byte[]{(byte)0x80});check(remote.size()==1&&(remote.get(0)[0]&255)==129&&remoteReady[0]==0,"remote reset made ready too soon");
  other.notification(COMPLETE);check(remoteReady[0]==1&&remote.size()==1,"remote completion bounced another acknowledgement");
  System.out.println("Upstream-compatible Gattlink reset tests passed");
 }
}
