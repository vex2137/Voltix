package com.voltix.monitor;

// No command, path, setting or other caller-controlled operation is accepted.
interface IReadOnlyGpuService {
    void destroy() = 16777114;
    String readSnapshot() = 1;
}
