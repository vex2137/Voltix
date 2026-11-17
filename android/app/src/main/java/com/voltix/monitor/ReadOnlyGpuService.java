package com.voltix.monitor;

import android.os.SystemClock;
import android.system.ErrnoException;
import android.system.Os;
import android.system.OsConstants;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;
import java.io.FileDescriptor;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.AccessDeniedException;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Shizuku UserService, not an Android framework Service. Fixed read-only sysfs ABI. */
public final class ReadOnlyGpuService extends IReadOnlyGpuService.Stub {
    private static final int MAX_RECORDS = 48, MAX_BYTES = 1024;
    private final Map<String, Probe> probes = new LinkedHashMap<>();
    private final List<JSONObject> discovery = new ArrayList<>();
    private final Map<String,JSONObject> negative = new LinkedHashMap<>();
    private long discoveredAt = -30000, lastRead = -2000;
    private String cached;
    public ReadOnlyGpuService() {}

    private static final class Probe {
        final String kind, path, label;
        Probe(String kind, String path, String label) { this.kind=kind; this.path=path; this.label=label; }
    }
    private static JSONObject record(Probe p, String status, String text) throws JSONException {
        return new JSONObject().put("kind",p.kind).put("path",p.path).put("label",p.label).put("status",status).put("text",text);
    }
    private static JSONObject read(Probe p) throws JSONException {
        FileDescriptor fd = null;
        try {
            // All paths originate in this class. Never write/open a device, ioctl, or spawn a shell.
            fd = Os.open(p.path, OsConstants.O_RDONLY | OsConstants.O_CLOEXEC | OsConstants.O_NONBLOCK, 0);
            byte[] bytes = new byte[MAX_BYTES+1]; int count=0;
            while (count < bytes.length) {
                int n = Os.read(fd,bytes,count,bytes.length-count);
                if (n <= 0) break;
                count += n;
            }
            if (count > MAX_BYTES) return record(p,"too_large","");
            String text=new String(bytes,0,count,StandardCharsets.UTF_8).trim();
            if (p.kind.equals("identity")) text=text.replace("\u0000","").trim();
            return record(p,"readable",text);
        } catch (java.io.InterruptedIOException e) {
            return record(p,"temporary","");
        } catch (ErrnoException e) {
            String status = e.errno == OsConstants.EACCES || e.errno == OsConstants.EPERM ? "read_denied" :
                (e.errno == OsConstants.ENOENT || e.errno == OsConstants.ENODEV ? "unsupported" :
                (e.errno == OsConstants.EAGAIN || e.errno == OsConstants.EINTR ? "temporary" : "read_failed"));
            return record(p,status,"errno="+e.errno);
        } finally { if (fd != null) try { Os.close(fd); } catch (ErrnoException ignored) {} }
    }
    private void add(String kind, String path, String label) {
        if (probes.size() < MAX_RECORDS-5) probes.putIfAbsent(kind+":"+path,new Probe(kind,path,label));
    }
    static boolean maliName(String name) {
        return name.matches("[a-zA-Z0-9._-]{1,80}") && name.contains("mali") &&
            !name.contains("bus") && !name.contains("mem") && !name.contains("cpu");
    }
    private void devfreq(String base) {
        add("cur_freq_hz",base+"/cur_freq","");
        add("min_freq_hz",base+"/min_freq","");
        add("max_freq_hz",base+"/max_freq","");
        add("freqs_hz",base+"/available_frequencies","");
        add("runtime_state",base+"/device/power/runtime_status","");
        add("mali_util_unknown",base+"/load","");
    }
    private interface Visitor { void visit(Path entry) throws JSONException; }
    private void entries(String root, Visitor visitor) throws JSONException {
        try (DirectoryStream<Path> entries = Files.newDirectoryStream(Paths.get(root))) {
            int count=0;
            for (Path entry : entries) { if (++count > 128) break; visitor.visit(entry); }
        } catch (IOException | SecurityException e) {
            String status=e instanceof AccessDeniedException || e instanceof SecurityException ? "read_denied" :
                (e instanceof java.nio.file.NoSuchFileException ? "unsupported" : "read_failed");
            if (discovery.size()<4) discovery.add(record(new Probe("discovery",root,""),status,""));
        }
    }
    private void discover() throws JSONException {
        probes.clear(); discovery.clear(); negative.clear();
        devfreq("/sys/class/devfreq/13000000.mali");
        devfreq("/sys/class/misc/mali0/device/devfreq/mali0");
        add("runtime_state","/sys/class/misc/mali0/device/power/runtime_status","");
        add("ged_load_v1","/sys/kernel/ged/hal/gpu_utilization","");
        add("ged_freq_unknown","/sys/kernel/ged/hal/current_freqency","");
        add("mali_util_unknown","/sys/class/misc/mali0/device/utilization","");
        add("mali_util_unknown","/sys/class/misc/mali0/device/utilisation","");
        entries("/sys/class/devfreq",entry -> { if (maliName(entry.getFileName().toString())) devfreq(entry.toString()); });
        entries("/sys/class/thermal",entry -> {
            if (!entry.getFileName().toString().matches("thermal_zone[0-9]{1,3}")) return;
            JSONObject type=read(new Probe("thermal_type",entry+"/type",""));
            if (!type.getString("status").equals("readable")) {
                if (type.getString("status").equals("read_denied") && discovery.size()<4)
                    discovery.add(record(new Probe("discovery",entry+"/type",""),"read_denied",""));
                return;
            }
            String label=type.getString("text").toLowerCase(java.util.Locale.ROOT);
            // Explicit GPU zone types only; never CPU/SoC/battery temperatures.
            if (label.matches("(gpu|gpuss|mali)([-_]?(thermal|[0-9]+))?")) add("gpu_temp_mc",entry+"/temp",label);
            else if ((label.contains("gpu") || label.contains("mali")) && discovery.size()<4)
                discovery.add(record(new Probe("thermal_candidate",entry+"/type",""),"readable",label));
        });
        discoveredAt=SystemClock.elapsedRealtime();
    }
    private JSONObject readConfigured(Probe p) throws JSONException {
        String key=p.kind+":"+p.path;
        JSONObject cached=negative.get(key);
        if (cached!=null) return cached;
        JSONObject result=read(p);
        String status=result.getString("status");
        if (status.equals("unsupported") || status.equals("read_denied")) negative.put(key,result);
        return result;
    }
    @Override public synchronized String readSnapshot() {
        try {
            long now=SystemClock.elapsedRealtime();
            if (cached!=null && now-lastRead<2000) return cached;
            JSONObject result=new JSONObject().put("uid",android.os.Process.myUid()).put("sampled_elapsed_ms",SystemClock.elapsedRealtime());
            JSONArray records=new JSONArray(); result.put("records",records);
            if (android.os.Process.myUid()!=2000) return result.toString();
            records.put(read(new Probe("identity","/proc/self/attr/current","")));
            if (SystemClock.elapsedRealtime()-discoveredAt>=30000) discover();
            for (JSONObject entry : discovery) records.put(entry);
            for (Probe p : probes.values()) { if (records.length()>=MAX_RECORDS) break; records.put(readConfigured(p)); }
            cached=result.toString();lastRead=now;return cached;
        } catch (JSONException | RuntimeException e) { return "{}"; }
    }
    @Override public void destroy() { System.exit(0); }
}
