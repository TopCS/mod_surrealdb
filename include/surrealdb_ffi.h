#ifndef SURREALDB_FFI_H
#define SURREALDB_FFI_H

#include <stdint.h>

// Forward-declared opaque handle to a SurrealDB connection context.
typedef struct SurHandle SurHandle;

// Initializes async runtime (no-op in stub). Returns 0 on success.
int32_t surreal_init_runtime(void);

// Connects to SurrealDB and returns a handle, or NULL on failure.
SurHandle* surreal_connect(const char* url,
                           const char* ns,
                           const char* db,
                           const char* user,
                           const char* pass);

// Publishes a JSON payload to a table/topic. Returns 0 on success.
int32_t surreal_publish(SurHandle* handle,
                        const char* table_or_topic,
                        const char* json_payload);

// Frees the handle and closes connections.
void surreal_close(SurHandle* handle);

// Returns last error code for the handle (implementation-defined).
int32_t surreal_last_error_code(SurHandle* handle);

#endif // SURREALDB_FFI_H

