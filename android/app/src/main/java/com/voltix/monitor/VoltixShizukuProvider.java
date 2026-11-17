package com.voltix.monitor;

import rikka.shizuku.ShizukuProvider;

/** Official provider, with automatic root/Sui integration explicitly disabled. */
public final class VoltixShizukuProvider extends ShizukuProvider {
    @Override public boolean onCreate() {
        ShizukuProvider.disableAutomaticSuiInitialization();
        return super.onCreate();
    }
}
