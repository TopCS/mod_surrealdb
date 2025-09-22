#include <stdio.h>
#include <stdint.h>
#include "../include/surrealdb_ffi.h"

static void on_cmd(const char* topic, const char* json, void* user_data) {
    (void)user_data;
    printf("callback: topic=%s json=%s\n", topic ? topic : "(null)", json ? json : "(null)");
}

int main(void) {
    if (surreal_init_runtime() != 0) {
        fprintf(stderr, "runtime init failed\n");
        return 1;
    }

    SurHandle* h = surreal_connect("ws://127.0.0.1:8000", "test", "test", "root", "root");
    if (!h) {
        fprintf(stderr, "connect failed\n");
        return 2;
    }

    if (surreal_subscribe(h, "commands", on_cmd, NULL) != 0) {
        fprintf(stderr, "subscribe failed\n");
        surreal_close(h);
        return 3;
    }

    // In stub mode, emit a synthetic message through the debug hook
    surreal_debug_emit(h, "commands", "{\"do\":\"ping\"}");

    surreal_unsubscribe(h, "commands");
    surreal_close(h);
    return 0;
}

