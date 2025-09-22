#ifndef SURREALDB_FFI_H
#define SURREALDB_FFI_H

#include <stdint.h>

// Forward-declared opaque handle to a SurrealDB connection context.
typedef struct SurHandle SurHandle;
typedef void (*surreal_command_cb)(const char* topic, const char* json, void* user_data);

// Initializes async runtime (no-op in stub). Returns 0 on success.
int32_t surreal_init_runtime(void);

// Connects to SurrealDB and returns a handle, or NULL on failure.
SurHandle* surreal_connect(const char* url,
                           const char* ns,
                           const char* db,
                           const char* user,
                           const char* pass);

// Token-based connect variant (e.g., Bearer token).
SurHandle* surreal_connect_with_token(const char* url,
                                      const char* ns,
                                      const char* db,
                                      const char* token);

// Publishes a JSON payload to a table/topic. Returns 0 on success.
int32_t surreal_publish(SurHandle* handle,
                        const char* table_or_topic,
                        const char* json_payload);

// Frees the handle and closes connections.
void surreal_close(SurHandle* handle);

// Returns last error code for the handle (implementation-defined).
int32_t surreal_last_error_code(SurHandle* handle);

// Subscribes to incoming commands/messages on a topic (stubbed).
int32_t surreal_subscribe(SurHandle* handle,
                          const char* topic,
                          surreal_command_cb cb,
                          void* user_data);

// Unsubscribes from a topic (stubbed).
int32_t surreal_unsubscribe(SurHandle* handle, const char* topic);

// Testing helper: trigger a callback invocation (stub only).
int32_t surreal_debug_emit(SurHandle* handle, const char* topic, const char* json);

#endif // SURREALDB_FFI_H
