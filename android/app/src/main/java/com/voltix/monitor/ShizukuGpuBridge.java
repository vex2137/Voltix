package com.voltix.monitor;

import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.RemoteException;
import android.os.SystemClock;
import org.json.JSONObject;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import rikka.shizuku.Shizuku;

/** Optional shell-UID UserService client. Sensor IPC never runs on the UI thread. */
public final class ShizukuGpuBridge {
    private static final String MANAGER="moe.shizuku.privileged.api";
    private static final int REQUEST=804;
    private final Context context;
    private final Handler main=new Handler(Looper.getMainLooper());
    private final ExecutorService worker=Executors.newSingleThreadExecutor(r -> { Thread t=new Thread(r,"voltix-shizuku-gpu");t.setDaemon(true);return t; });
    private final SharedPreferences preferences;
    private final GpuSession gate;
    private final Shizuku.UserServiceArgs args;
    private IReadOnlyGpuService service;
    private JSONObject sample;
    private boolean installed, running, binding, inFlight, closed;
    private int uid=-1;
    private long nextPoll, startedRead, startedBind;

    private final Shizuku.OnBinderReceivedListener received=() -> reconcile();
    private final Shizuku.OnBinderDeadListener died=this::binderDied;
    private void binderDied() { running=false;uid=-1;gate.authorized=false;gate.authorization="not_requested";invalidate("disconnected"); }
    private final Shizuku.OnRequestPermissionResultListener permission=this::permissionResult;
    private void permissionResult(int code,int result) {
        if (closed || code!=REQUEST) return;
        gate.permission(result==PackageManager.PERMISSION_GRANTED);sample=null;
        if (gate.authorized) reconcile(); else detach();
    }
    private ServiceConnection connection;
    private ServiceConnection connectionFor(long epoch) { return new ServiceConnection() {
        @Override public void onServiceConnected(ComponentName name,IBinder binder) { main.post(() -> connected(binder)); }
        private void connected(IBinder binder) {
            if (closed || !gate.acceptsConnection(epoch) || uid!=2000) return;
            if (!binder.isBinderAlive()) return;
            service=IReadOnlyGpuService.Stub.asInterface(binder);binding=false;sample=null;
            if (service==null) { invalidate("disconnected");return; }
            gate.connect(); nextPoll=0; poll();
        }
        @Override public void onServiceDisconnected(ComponentName name) { main.post(() -> {
            if (!closed && gate.acceptsConnection(epoch) && (service!=null || binding || gate.connected)) invalidate("disconnected");
        }); }
    }; }
    public ShizukuGpuBridge(Context context) {
        this.context=context.getApplicationContext();
        preferences=this.context.getSharedPreferences("advanced_gpu",Context.MODE_PRIVATE);
        gate=new GpuSession(preferences.getBoolean("shizuku_enabled",false));
        args=new Shizuku.UserServiceArgs(new ComponentName(this.context,ReadOnlyGpuService.class))
            .tag("voltix.readonly.mali").processNameSuffix("gpu_read").daemon(false).version(BuildConfig.VERSION_CODE);
        Shizuku.addBinderReceivedListenerSticky(received,main);
        Shizuku.addBinderDeadListener(died,main);
        Shizuku.addRequestPermissionResultListener(permission,main);
    }
    private void detach() {
        boolean hadService=service!=null || binding || gate.connected;
        ServiceConnection old=connection;
        service=null;sample=null;connection=null;binding=false;gate.invalidateConnection();
        if (hadService && old!=null) {
            try { if (Shizuku.pingBinder()) Shizuku.unbindUserService(args,old,true); } catch (RuntimeException ignored) { }
            // Also release the SDK's local callbacks; old queued callbacks are epoch-gated.
            try { if (Shizuku.pingBinder()) Shizuku.unbindUserService(args,old,false); } catch (RuntimeException ignored) { }
        }
    }
    private void invalidate(String reason) { detach();gate.invalidate(reason); }
    public void foreground(boolean foreground) {
        if (closed) return;
        detach();gate.foreground(foreground);
        if (foreground) reconcile();
    }
    private void reconcile() {
        if (closed) return;
        try { context.getPackageManager().getPackageInfo(MANAGER,0);installed=true; }
        catch (PackageManager.NameNotFoundException | RuntimeException e) { installed=false; }
        try {
            running=Shizuku.pingBinder();
            if (!gate.enabled) { invalidate("disabled");return; }
            if (!gate.foreground) { invalidate("paused");return; }
            if (!installed) { invalidate("not_installed");return; }
            if (!running) { uid=-1;gate.authorized=false;gate.authorization="not_requested";invalidate("not_running");return; }
            if (Shizuku.isPreV11() || Shizuku.getVersion()<12) { invalidate("old_service");return; }
            uid=Shizuku.getUid();
            if (uid!=2000) { invalidate("shell_required");return; }
            boolean granted=Shizuku.checkSelfPermission()==PackageManager.PERMISSION_GRANTED;
            gate.authorized=granted;
            if (!granted) {
                if (!gate.authorization.equals("denied")) gate.authorization="not_requested";
                invalidate(gate.authorization.equals("denied")?"permission_denied":"permission_required");return;
            }
            gate.authorization="granted";
            if (service==null && !binding) {
                binding=true;startedBind=SystemClock.elapsedRealtime();gate.state="connecting";
                connection=connectionFor(gate.beginConnection());
                Shizuku.bindUserService(args,connection);
                long attempt=startedBind;
                main.postDelayed(() -> { if (!closed && binding && startedBind==attempt && gate.enabled && gate.foreground) invalidate("disconnected"); },10000);
            }
        } catch (SecurityException e) { gate.permission(false);invalidate("permission_denied"); }
        catch (RuntimeException e) { invalidate("disconnected"); }
    }
    /** Fixed commands from bundled UI only: enable, disable, authorize, open official manager. */
    public void command(int command) {
        if (closed) return;
        if (command==1 || command==2) {
            detach();gate.enable(command==1);
            preferences.edit().putBoolean("shizuku_enabled",gate.enabled).apply();
            reconcile();
        } else if (command==3 && gate.enabled && gate.foreground) {
            reconcile();
            if (!running || uid!=2000) return;
            try {
                if (Shizuku.isPreV11() || Shizuku.getVersion()<12) return;
                if (Shizuku.checkSelfPermission()==PackageManager.PERMISSION_GRANTED) { reconcile();return; }
                if (Shizuku.shouldShowRequestPermissionRationale()) { gate.permission(false);return; }
                gate.authorization="requesting";gate.state="permission_required";
                Shizuku.requestPermission(REQUEST);
            } catch (RuntimeException e) { invalidate("disconnected"); }
        } else if (command==4) {
            try {
                Intent intent=context.getPackageManager().getLaunchIntentForPackage(MANAGER);
                if (intent==null) intent=new Intent(Intent.ACTION_VIEW,Uri.parse("https://shizuku.rikka.app/download/"));
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);context.startActivity(intent);
            } catch (RuntimeException ignored) { }
        }
    }
    public void poll() {
        if (closed || !gate.enabled || !gate.foreground) return;
        long now=SystemClock.elapsedRealtime();
        if (binding && now-startedBind>10000) { invalidate("disconnected");return; }
        if (inFlight) {
            if (now-startedRead>6000 && gate.connected) invalidate("read_timeout");
            return;
        }
        if (!gate.canPoll() || service==null || now<nextPoll) return;
        nextPoll=now+2000;startedRead=now;inFlight=true;
        long token=gate.token();IReadOnlyGpuService remote=service;
        main.postDelayed(() -> { if (!closed && inFlight && startedRead==now && gate.accepts(token)) invalidate("read_timeout"); },6000);
        worker.execute(() -> {
            if (!gate.accepts(token)) { main.post(() -> { inFlight=false; }); return; }
            JSONObject result=null;String error=null;
            try {
                String raw=remote.readSnapshot();
                if (raw==null || raw.length()>65536) throw new IllegalArgumentException();
                result=new JSONObject(raw);
                if (result.optInt("uid",-1)!=2000 || !result.has("records") || !result.has("sampled_elapsed_ms")) throw new IllegalArgumentException();
            } catch (SecurityException e) { error="permission_denied"; }
            catch (RemoteException e) { error="disconnected"; }
            catch (Exception e) { error="invalid_payload"; }
            final JSONObject completed=result;final String failure=error;
            main.post(() -> {
                inFlight=false;
                if (closed || !gate.accepts(token)) return;
                if (failure!=null) {
                    if (failure.equals("permission_denied")) gate.permission(false);
                    invalidate(failure);
                } else sample=completed;
            });
        });
    }
    public JSONObject snapshot() {
        JSONObject out=new JSONObject();
        try {
            out.put("enabled",gate.enabled).put("installed",installed).put("service_running",running)
                .put("authorization",gate.authorization).put("state",gate.state).put("uid",uid);
            if (gate.canPoll() && sample!=null && SystemClock.elapsedRealtime()-sample.optLong("sampled_elapsed_ms",0)<=6000) {
                out.put("sampled_elapsed_ms",sample.getLong("sampled_elapsed_ms")).put("records",sample.getJSONArray("records"));
            }
        } catch (Exception ignored) { }
        return out;
    }
    public void close() {
        if (closed) return;
        detach();gate.foreground(false);closed=true;worker.shutdownNow();
        Shizuku.removeBinderReceivedListener(received);Shizuku.removeBinderDeadListener(died);Shizuku.removeRequestPermissionResultListener(permission);
    }
}
