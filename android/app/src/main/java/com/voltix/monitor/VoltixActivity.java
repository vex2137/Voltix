package com.voltix.monitor;

import android.app.ActivityManager;
import android.app.NativeActivity;
import android.content.*;
import android.content.pm.PackageManager;
import android.hardware.Sensor;
import android.hardware.SensorManager;
import android.net.*;
import android.os.*;
import android.provider.Settings;
import android.util.DisplayMetrics;
import org.json.*;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;

/** Foreground-only platform bridge. Optional Shizuku is isolated from ordinary monitoring. */
public final class VoltixActivity extends NativeActivity {
    // Register the JNI library with ART as well as NativeActivity's native loader.
    static { System.loadLibrary("voltix"); }
    private static native void nativeSnapshot(String json);
    private static native int nativeCommand();
    private static native int nativeGpuCommand();
    private static native int nativePollingInterval();
    private static native String nativeExportJson();
    private final Handler handler = new Handler(Looper.getMainLooper());
    private Intent batteryIntent;
    private boolean registered;
    private boolean resumed;
    private String exportData = "";
    private volatile String exportStatus = "";
    private volatile String actionStatus = "";
    private JSONArray sensors = new JSONArray();
    private ShizukuGpuBridge advancedGpu;
    private final BroadcastReceiver batteryReceiver = new BroadcastReceiver() {
        @Override public void onReceive(Context context, Intent intent) { batteryIntent = intent; publish(resumed); }
    };
    private static Object valid(int value) { return value == Integer.MIN_VALUE ? JSONObject.NULL : value; }
    private void enumerateSensors() {
        sensors = new JSONArray();
        SensorManager manager = (SensorManager)getSystemService(SENSOR_SERVICE);
        if (manager != null) for (Sensor sensor : manager.getSensorList(Sensor.TYPE_ALL)) {
            if (sensors.length() >= 128) break;
            sensors.put(sensor.getName() + " · " + sensor.getVendor() + " · " + sensor.getStringType());
        }
    }
    private final Runnable tick = new Runnable() {
        @Override public void run() {
            if (!resumed) return;
            if (advancedGpu != null) {
                for (int i=0;i<4;i++) { int action=nativeGpuCommand();if(action==0)break;advancedGpu.command(action); }
                advancedGpu.poll();
            }
            publish(true);
            int command = nativeCommand();
            try {
                if (command == 1) { actionStatus=""; startActivity(new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:" + getPackageName()))); }
                if (command == 2) {
                    exportData = nativeExportJson(); exportStatus = "cc_export_choose";
                    Intent save = new Intent(Intent.ACTION_CREATE_DOCUMENT).setType("application/json").addCategory(Intent.CATEGORY_OPENABLE).putExtra(Intent.EXTRA_TITLE,"Voltix-telemetry.json");
                    startActivityForResult(save, 44);
                }
            } catch (RuntimeException error) { if(command==1) actionStatus="cc_app_settings_failed"; else exportStatus = "cc_export_failed"; }
            handler.postDelayed(this, Math.max(1000, Math.min(5000, nativePollingInterval())));
        }
    };
    @Override protected void onResume() {
        super.onResume(); resumed = true;
        try {
            if (advancedGpu == null) advancedGpu = new ShizukuGpuBridge(this);
            advancedGpu.foreground(true);
        } catch (RuntimeException | LinkageError ignored) { advancedGpu = null; }
        try { enumerateSensors(); } catch (RuntimeException ignored) { sensors = new JSONArray(); }
        IntentFilter filter = new IntentFilter(Intent.ACTION_BATTERY_CHANGED);
        try { batteryIntent = Build.VERSION.SDK_INT >= 33 ? registerReceiver(batteryReceiver, filter, Context.RECEIVER_NOT_EXPORTED) : registerReceiver(batteryReceiver, filter);
        registered = true; } catch (RuntimeException ignored) { batteryIntent = null; registered = false; }
        handler.removeCallbacks(tick); handler.post(tick);
    }
    @Override protected void onPause() {
        resumed = false; handler.removeCallbacks(tick);
        if (advancedGpu != null) advancedGpu.foreground(false);
        if (registered) { unregisterReceiver(batteryReceiver); registered = false; }
        publish(false);
        super.onPause();
    }
    @Override protected void onDestroy() {
        if (advancedGpu != null) advancedGpu.close();
        super.onDestroy();
    }
    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != 44) return;
        if (result != RESULT_OK || data == null || data.getData() == null) { exportStatus = "cc_export_cancelled"; return; }
        final Uri destination=data.getData(); final String json=exportData; exportData="";
        new Thread(() -> {
            try (OutputStream stream = getContentResolver().openOutputStream(destination)) {
                if (stream == null) throw new java.io.IOException("No output stream");
                stream.write(json.getBytes(StandardCharsets.UTF_8));stream.flush();exportStatus="cc_export_saved";
            } catch (Exception error) {exportStatus="cc_export_failed";}
        },"voltix-export").start();
    }

    private void publish(boolean foreground) {
        try {
            JSONObject root = new JSONObject(); root.put("foreground",foreground); root.put("elapsed_ms",SystemClock.elapsedRealtime()); root.put("network","TrafficStats"); root.put("sensors",sensors); root.put("export_status",exportStatus);root.put("action_status",actionStatus);
            if (advancedGpu != null) root.put("shizuku",advancedGpu.snapshot());
            final boolean granted=checkSelfPermission("android.permission.ACCESS_NETWORK_STATE")==PackageManager.PERMISSION_GRANTED;root.put("network_permission",granted);
            if (batteryIntent != null) {
                JSONObject b = new JSONObject(); int level = batteryIntent.getIntExtra(BatteryManager.EXTRA_LEVEL,-1), scale = batteryIntent.getIntExtra(BatteryManager.EXTRA_SCALE,-1);
                b.put("percent", level >= 0 && scale > 0 && level <= scale ? 100.0*level/scale : JSONObject.NULL);
                b.put("status", batteryIntent.getIntExtra(BatteryManager.EXTRA_STATUS,1)); b.put("source", batteryIntent.getIntExtra(BatteryManager.EXTRA_PLUGGED,0));
                int temp=batteryIntent.getIntExtra(BatteryManager.EXTRA_TEMPERATURE,Integer.MIN_VALUE), voltage=batteryIntent.getIntExtra(BatteryManager.EXTRA_VOLTAGE,-1);
                b.put("temperature_c", temp == Integer.MIN_VALUE ? JSONObject.NULL : temp/10.0); b.put("voltage_v",voltage > 0 ? voltage/1000.0 : JSONObject.NULL);
                BatteryManager manager = (BatteryManager)getSystemService(BATTERY_SERVICE);
                int current = manager == null ? Integer.MIN_VALUE : manager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CURRENT_NOW);
                b.put("current_a", current == Integer.MIN_VALUE ? JSONObject.NULL : current/1000000.0);
                b.put("power_w", current == Integer.MIN_VALUE || voltage <= 0 ? JSONObject.NULL : (current/1000000.0)*(voltage/1000.0));
                b.put("technology", batteryIntent.getStringExtra(BatteryManager.EXTRA_TECHNOLOGY)); b.put("health", batteryIntent.getIntExtra(BatteryManager.EXTRA_HEALTH,1)); root.put("battery",b);
            }
            try { ActivityManager am=(ActivityManager)getSystemService(ACTIVITY_SERVICE);
            if (am != null) { ActivityManager.MemoryInfo m=new ActivityManager.MemoryInfo();am.getMemoryInfo(m);root.put("memory_total",m.totalMem);root.put("memory_available",m.availMem);root.put("memory_low",m.lowMemory); }
            } catch (RuntimeException ignored) { }
            try { DisplayMetrics metrics = new DisplayMetrics(); getWindowManager().getDefaultDisplay().getRealMetrics(metrics);
            root.put("display",metrics.widthPixels+" × "+metrics.heightPixels+" px · "+metrics.densityDpi+" dpi · "+getWindowManager().getDefaultDisplay().getRefreshRate()+" Hz");
            } catch (RuntimeException ignored) { }
            try { long rx=TrafficStats.getTotalRxBytes(),tx=TrafficStats.getTotalTxBytes();root.put("received",rx>=0?rx:JSONObject.NULL);root.put("transmitted",tx>=0?tx:JSONObject.NULL);

            if (granted) { ConnectivityManager cm=(ConnectivityManager)getSystemService(CONNECTIVITY_SERVICE); LinkProperties link=cm==null?null:cm.getLinkProperties(cm.getActiveNetwork());root.put("network",link==null?"TrafficStats":link.getInterfaceName()); }
            } catch (RuntimeException ignored) { }
            nativeSnapshot(root.toString());
        } catch (Exception error) { /* Retain last valid sample; Rust expires stale bridge data. */ }
    }
}
