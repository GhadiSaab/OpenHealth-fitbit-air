package app.airspace.air.protocol;
import java.util.ArrayList;
import java.util.List;
/** Header metadata only: never identities, randoms, keys or application payloads. */
public final class DtlsTrace {
 private DtlsTrace(){}
 private static int u16(byte[] b,int p){return ((b[p]&255)<<8)|(b[p+1]&255);}
 private static int u24(byte[] b,int p){return ((b[p]&255)<<16)|((b[p+1]&255)<<8)|(b[p+2]&255);}
 public static String describe(byte[] b){
  List<String> out=new ArrayList<>();int p=0;
  while(p<b.length && out.size()<12){
   if(b.length-p<13)return "malformed";
   int type=b[p]&255,epoch=u16(b,p+3),len=u16(b,p+11),end=p+13+len;
   if(end>b.length)return "malformed";
   long recordSeq=0;for(int q=p+5;q<p+11;q++)recordSeq=(recordSeq<<8)|(b[q]&255L);
   String summary="r"+type+":v"+Integer.toHexString(u16(b,p+1))+":e"+epoch+":s"+recordSeq+":n"+len;
   if(epoch==0&&type==22){int h=p+13;int messages=0;while(h<end && messages++<12){
    if(end-h<12)return "malformed";
    int ht=b[h]&255,total=u24(b,h+1),seq=u16(b,h+4),off=u24(b,h+6),frag=u24(b,h+9);
    if(frag>end-h-12 || off>total || frag>total-off)return "malformed";
    summary+=" h"+ht+"#"+seq+"["+off+"+"+frag+"/"+total+"]";
    if(ht==2 && off==0 && frag==total && frag>=38){int body=h+12;int sessionLen=b[body+34]&255;int cipher=body+35+sessionLen;if(cipher+3<=h+12+frag)summary+=" cipher="+Integer.toHexString(u16(b,cipher));}
    h+=12+frag;
   }}
   out.add(summary);p=end;
  }return out.isEmpty()?"empty":String.join(";",out);
 }
}
