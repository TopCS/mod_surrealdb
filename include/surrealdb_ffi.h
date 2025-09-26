#ifndef SURREALDB_FFI_H
#define SURREALDB_FFI_H

#include <stdint.h>

// Forward-declared opaque handle to a SurrealDB connection context.
typedef struct SurHandle SurHandle;
typedef void (*surreal_command_cb)(const char* topic, const char* json, void* user_data);
typedef void (*surreal_log_cb)(const char* msg, void* user_data);

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
// Returns last global error code (e.g., from connect failures).
int32_t surreal_last_error_global(void);

// Returns 1 if the FFI library is built in stub mode, 0 if using the real client.
int32_t surreal_is_stub(void);

// Registers a logger callback that the FFI may use to emit informational
// messages (e.g., subscribe row counts). Passing NULL disables logging.
int32_t surreal_set_logger(surreal_log_cb cb, void* user_data);

// Copies the last error text for the handle into the provided buffer.
// Returns the number of bytes written (excluding NUL), or a negative error code on failure.
// If no error text is available, writes an empty string.
int32_t surreal_last_error_text(SurHandle* handle, char* buf, uint32_t len);

// Subscribes to incoming commands/messages on a topic (stubbed).
int32_t surreal_subscribe(SurHandle* handle,
                          const char* topic,
                          surreal_command_cb cb,
                          void* user_data);

// Unsubscribes from a topic (stubbed).
int32_t surreal_unsubscribe(SurHandle* handle, const char* topic);

// Testing helper: trigger a callback invocation (stub only).
int32_t surreal_debug_emit(SurHandle* handle, const char* topic, const char* json);

// Updates a record by id with a JSON object (merge/content semantics defined by implementation).
// Returns 0 on success.
int32_t surreal_update(SurHandle* handle,
                       const char* table,
                       const char* id,
                       const char* json_patch);

// Reads rows from a table, writing JSON array into out_json (NUL-terminated).
// Limit of 0 uses a sensible default; implementation may cap maximum.
// Returns 0 on success.
int32_t surreal_select(SurHandle* handle,
                       const char* table,
                       uint32_t limit,
                       char* out_json,
                       uint32_t out_len);

// Gets a single record by id from a table, writing JSON (or null) into out_json.
// Returns 0 on success.
int32_t surreal_get(SurHandle* handle,
                    const char* table,
                    const char* id,
                    char* out_json,
                    uint32_t out_len);

#endif // SURREALDB_FFI_H
