#include <stdio.h>
#include <stdint.h>
#include "../include/surrealdb_ffi.h"

int main(void) {
    int32_t rc = surreal_init_runtime();
    if (rc != 0) {
        fprintf(stderr, "failed to init runtime: %d\n", rc);
        return 1;
    }

    SurHandle* h = surreal_connect("ws://127.0.0.1:8000", "test", "test", "root", "root");
    if (!h) {
        fprintf(stderr, "connect failed\n");
        return 2;
    }

    rc = surreal_publish(h, "events", "{\"ok\":true}");
    if (rc != 0) {
        fprintf(stderr, "publish failed: %d\n", rc);
    } else {
        printf("publish ok\n");
    }

    surreal_close(h);
    return rc;
}

