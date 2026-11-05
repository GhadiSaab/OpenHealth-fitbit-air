package app.airspace.air.protocol;
import java.io.*;import java.util.*;import java.util.concurrent.*;import java.security.SecureRandom;
import org.bouncycastle.tls.*;import org.bouncycastle.tls.crypto.impl.bc.BcTlsCrypto;
/** Full synthetic DTLS-over-Gattlink test, including sequence wrap and dropped ATT frames. */
public final class GattlinkDtlsTest {
 static final class Link implements DatagramTransport {
  final BlockingQueue<byte[]> incoming=new LinkedBlockingQueue<>();final Gattlink gl;final boolean host;
  final ScheduledExecutorService wire;Link peer;int id=1,frames;final int dropEvery;volatile boolean closed;
  Link(boolean host,ScheduledExecutorService wire,int dropEvery,int mtu){this.host=host;this.wire=wire;this.dropEvery=dropEvery;
   gl=new Gattlink(p->{if(p.length>mtu-3)throw new AssertionError("ATT frame exceeds MTU");byte[] copy=p.clone();if(dropEvery>0&&++frames%dropEvery==0)return;wire.execute(()->peer.gl.notification(copy));},ip->{if(AirWire.checksum(ip,0,20)!=0)throw new AssertionError("IP checksum");incoming.offer(Arrays.copyOfRange(ip,28,ip.length));},()->{},m->{throw new AssertionError(m);});gl.setMtu(mtu);
  }
  public int getReceiveLimit(){return 1400;}public int getSendLimit(){return 1400;}
  public void send(byte[] b,int o,int n)throws IOException{if(closed)throw new IOException("closed");gl.send(AirWire.ipUdp(Arrays.copyOfRange(b,o,o+n),id++,host));}
  public int receive(byte[] b,int o,int n,int wait)throws IOException{try{byte[] x=incoming.poll(Math.max(1,wait),TimeUnit.MILLISECONDS);if(closed)throw new IOException("closed");if(x==null)return -1;if(x.length>n)throw new IOException("overflow");System.arraycopy(x,0,b,o,x.length);return x.length;}catch(InterruptedException e){throw new IOException("closed");}}
  public void close(){closed=true;incoming.offer(new byte[0]);}
 }
 public static void main(String[] args)throws Exception{
  for(int mtu:new int[]{23,247}){
  for(int drop:new int[]{0,17}){
   ScheduledExecutorService wire=Executors.newSingleThreadScheduledExecutor();ExecutorService serverThread=Executors.newSingleThreadExecutor();
   Link server=new Link(true,wire,drop,mtu),client=new Link(false,wire,drop,mtu);server.peer=client;client.peer=server;
   byte[] id="MD-00000001-00000002".getBytes("US-ASCII"),key=new byte[16];Arrays.fill(key,(byte)7);
   try{
    wire.scheduleAtFixedRate(()->{long now=System.currentTimeMillis();server.gl.tick(now);client.gl.tick(now);},100,100,TimeUnit.MILLISECONDS);
    server.gl.reset();client.gl.reset();
    Future<Boolean> result=serverThread.submit(()->{DTLSTransport s=AirDtls.accept(server,id,key);byte[] buf=new byte[1024];for(int i=0;i<32;i++){int n=s.receive(buf,0,buf.length,10000);if(n!=120||buf[0]!=(byte)i)throw new AssertionError("corrupt DTLS request");s.send(buf,0,n);}return true;});
    PSKTlsClient c=new PSKTlsClient(new BcTlsCrypto(new SecureRandom()),id,key){protected ProtocolVersion[] getSupportedVersions(){return new ProtocolVersion[]{ProtocolVersion.DTLSv12};}protected int[] getSupportedCipherSuites(){return new int[]{CipherSuite.TLS_PSK_WITH_AES_128_CCM_8};}public int getHandshakeTimeoutMillis(){return 10000;}};
    DTLSTransport dtls=new DTLSClientProtocol().connect(c,client);byte[] buf=new byte[1024];
    for(int i=0;i<32;i++){byte[] request=new byte[120];Arrays.fill(request,(byte)i);dtls.send(request,0,request.length);int n=dtls.receive(buf,0,buf.length,10000);if(n!=request.length||!Arrays.equals(request,Arrays.copyOf(buf,n)))throw new AssertionError("DTLS echo corrupted");}
    if(!result.get(5,TimeUnit.SECONDS))throw new AssertionError("server failed");System.out.println("DTLS over Gattlink passed: MTU="+mtu+", dropped every "+drop+"th ATT frame, 32 echoed messages");
   }finally{server.close();client.close();wire.shutdownNow();serverThread.shutdownNow();}
  }}
 }
}
