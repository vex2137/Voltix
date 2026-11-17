package com.voltix.monitor;

/** Pure lifecycle gate, also exercised by JVM tests. Accessed on the main handler. */
final class GpuSession {
    volatile boolean enabled, foreground, authorized, connected;
    String state = "disabled", authorization = "not_requested";
    private volatile long generation;
    private long connectionGeneration;

    GpuSession(boolean enabled) { this.enabled = enabled; }
    void invalidate(String reason) { generation++;invalidateConnection();state = reason; }
    void enable(boolean value) { enabled = value; invalidate(value ? "permission_required" : "disabled"); }
    void foreground(boolean value) { foreground = value; invalidate(enabled ? (value ? "permission_required" : "paused") : "disabled"); }
    void permission(boolean granted) {
        authorized = granted;
        authorization = granted ? "granted" : "denied";
        invalidate(granted ? "connecting" : "permission_denied");
    }
    long beginConnection() { return ++connectionGeneration; }
    void invalidateConnection() { connectionGeneration++;connected=false; }
    boolean acceptsConnection(long token) { return enabled && foreground && authorized && connectionGeneration==token; }
    void connect() { generation++; connected = true; state = "connected"; }
    boolean canPoll() { return enabled && foreground && authorized && connected; }
    long token() { return generation; }
    boolean accepts(long token) { return canPoll() && generation == token; }
}
