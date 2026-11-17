package com.voltix.monitor;
import org.junit.Test;
import static org.junit.Assert.*;
public final class GpuSessionTest {
    private GpuSession ready() { GpuSession gate=new GpuSession(true);gate.foreground(true);gate.permission(true);gate.connect();return gate; }
    @Test public void ordinaryMonitoringDefaultsToNoPrivilegedPolling() {
        GpuSession gate=new GpuSession(false);gate.foreground(true);gate.permission(true);gate.connect();assertFalse(gate.canPoll());
    }
    @Test public void denialRejectsLateReadAndRequiresAuthorization() {
        GpuSession gate=ready();long token=gate.token();gate.permission(false);assertFalse(gate.canPoll());assertFalse(gate.accepts(token));assertEquals("permission_denied",gate.state);
    }
    @Test public void disconnectionInvalidatesReadEvenAfterReconnect() {
        GpuSession gate=ready();long token=gate.token();gate.invalidate("disconnected");assertFalse(gate.canPoll());gate.connect();assertFalse(gate.accepts(token));assertTrue(gate.canPoll());
    }
    @Test public void pauseAndDisableRejectInFlightSamples() {
        GpuSession gate=ready();long token=gate.token();gate.foreground(false);assertFalse(gate.accepts(token));assertFalse(gate.canPoll());gate.foreground(true);gate.connect();assertFalse(gate.accepts(token));gate.enable(false);assertFalse(gate.canPoll());
    }
    @Test public void oldServiceCallbackCannotDisconnectTheNewForegroundSession() {
        GpuSession gate=ready();long old=gate.beginConnection();gate.connect();assertTrue(gate.acceptsConnection(old));
        gate.foreground(false);gate.foreground(true);long current=gate.beginConnection();gate.connect();
        assertFalse(gate.acceptsConnection(old));assertTrue(gate.acceptsConnection(current));
        gate.invalidateConnection();assertFalse(gate.acceptsConnection(current));
    }
}
