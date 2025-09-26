/*
 * mod_surrealdb.c — FreeSWITCH module scaffolding for SurrealDB I/O via Rust FFI
 *
 * Build notes:
 * - Links against libfreeswitch via pkg-config
 * - Optionally links to libsurrealdb_ffi if available; otherwise, runs in no-op mode
 *
 * Configuration: read from surrealdb.conf.xml (autoload_configs)
 *   <param name="url" value="127.0.0.1:8000"/>
 *   <param name="namespace" value="test"/>
 *   <param name="database" value="test"/>
 *   <param name="username" value="root"/>
 *   <param name="password" value="root"/>
 *   <!-- Or use token instead of username/password -->
 *   <param name="token" value=""/>
 *   <param name="connect-on-load" value="true"/>
 */

#include <switch.h>
#include <string.h>
#include <stdlib.h>
#include <time.h>
#include <switch_json.h>

#ifdef HAVE_SURREALDB_FFI
#include "include/surrealdb_ffi.h"
static void surreal_ffi_log_cb(const char* msg, void* user_data)
{
    (void)user_data;
    if (!msg) return;
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "mod_surrealdb: %s\n", msg);
}
/* Allow running with older libsurrealdb_ffi without failing to load. */
extern int32_t surreal_last_error_text(SurHandle* handle, char* buf, uint32_t len) __attribute__((weak));
#endif

#define MOD_SURREALDB_NAME "mod_surrealdb"

SWITCH_MODULE_LOAD_FUNCTION(mod_surrealdb_load);
SWITCH_MODULE_SHUTDOWN_FUNCTION(mod_surrealdb_shutdown);
SWITCH_MODULE_DEFINITION(mod_surrealdb, mod_surrealdb_load, mod_surrealdb_shutdown, NULL);

/* API: surrealdb.publish <table_or_topic> <json> */
SWITCH_STANDARD_API(mod_surrealdb_publish_api);
/* API: surrealdb.select <table> [limit] */
SWITCH_STANDARD_API(mod_surrealdb_select_api);
/* API: surrealdb.get <table> <id> */
SWITCH_STANDARD_API(mod_surrealdb_get_api);
/* API: surrealdb.update <table> <id> <json> */
SWITCH_STANDARD_API(mod_surrealdb_update_api);

/* internal helpers */
static char *mod_surrealdb_ltrim(char *s);

typedef struct mod_surrealdb_cfg_s {
	char *url;
	char *ns;
	char *db;
	char *user;
	char *pass;
	char *token;
	switch_bool_t connect_on_load;
	char *command_table;
	switch_bool_t enable_commands;
	/* Event sink */
	switch_bool_t enable_events;
	char *event_table;
	char *event_filter;
	uint32_t send_queue_size;
	uint32_t circuit_breaker_ms;
    /* CDR sink */
    switch_bool_t enable_cdr;
    char *cdr_table;
} mod_surrealdb_cfg_t;

typedef struct mod_surrealdb_state_s {
#ifdef HAVE_SURREALDB_FFI
	SurHandle *handle;
#else
	int unused;
#endif
	mod_surrealdb_cfg_t cfg;
	/* Event sink state */
	switch_bool_t events_running;
	switch_queue_t *send_queue;
	switch_thread_t *event_thread;
	switch_time_t cb_reset_time;
	switch_event_node_t *event_nodes[SWITCH_EVENT_ALL];
	int event_nodes_count;
} mod_surrealdb_state_t;

static mod_surrealdb_state_t g_mod = { 0 };

static void mod_surrealdb_log_cfg(void)
{
	switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,
		"%s: cfg url=%s ns=%s db=%s user=%s token=%s connect_on_load=%s\n",
		MOD_SURREALDB_NAME,
		g_mod.cfg.url ? g_mod.cfg.url : "(unset)",
		g_mod.cfg.ns ? g_mod.cfg.ns : "(unset)",
		g_mod.cfg.db ? g_mod.cfg.db : "(unset)",
		g_mod.cfg.user ? "(set)" : "(unset)",
		g_mod.cfg.token ? "(set)" : "(unset)",
		g_mod.cfg.connect_on_load ? "true" : "false");
}

static void mod_surrealdb_read_config(switch_memory_pool_t *pool)
{
	switch_xml_t cfg, xml, settings, param;

	memset(&g_mod.cfg, 0, sizeof(g_mod.cfg));
	g_mod.cfg.connect_on_load = SWITCH_TRUE;
	g_mod.cfg.enable_commands = SWITCH_FALSE;
	g_mod.cfg.enable_events = SWITCH_FALSE;
	g_mod.cfg.send_queue_size = 1000;
	g_mod.cfg.circuit_breaker_ms = 10000;
    g_mod.cfg.enable_cdr = SWITCH_FALSE;

	if (!(xml = switch_xml_open_cfg("surrealdb.conf", &cfg, NULL))) {
		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "%s: Failed to open surrealdb.conf\n", MOD_SURREALDB_NAME);
		return;
	}

	if ((settings = switch_xml_child(cfg, "settings"))) {
		for (param = switch_xml_child(settings, "param"); param; param = param->next) {
			const char *name = switch_xml_attr_soft(param, "name");
			const char *value = switch_xml_attr_soft(param, "value");
			if (zstr(name)) continue;
			if (!zstr(value)) {
				if (!strcasecmp(name, "url")) {
					g_mod.cfg.url = switch_core_strdup(pool, value);
				} else if (!strcasecmp(name, "namespace")) {
					g_mod.cfg.ns = switch_core_strdup(pool, value);
				} else if (!strcasecmp(name, "database")) {
					g_mod.cfg.db = switch_core_strdup(pool, value);
				} else if (!strcasecmp(name, "username")) {
					g_mod.cfg.user = switch_core_strdup(pool, value);
				} else if (!strcasecmp(name, "password")) {
					g_mod.cfg.pass = switch_core_strdup(pool, value);
				} else if (!strcasecmp(name, "token")) {
					g_mod.cfg.token = switch_core_strdup(pool, value);
				} else if (!strcasecmp(name, "connect-on-load")) {
					g_mod.cfg.connect_on_load = switch_true(value);
				} else if (!strcasecmp(name, "command-table")) {
					g_mod.cfg.command_table = switch_core_strdup(pool, value);
				} else if (!strcasecmp(name, "enable-commands")) {
					g_mod.cfg.enable_commands = switch_true(value);
				} else if (!strcasecmp(name, "enable-events")) {
					g_mod.cfg.enable_events = switch_true(value);
				} else if (!strcasecmp(name, "event-table")) {
					g_mod.cfg.event_table = switch_core_strdup(pool, value);
				} else if (!strcasecmp(name, "event-filter")) {
					g_mod.cfg.event_filter = switch_core_strdup(pool, value);
				} else if (!strcasecmp(name, "send-queue-size")) {
					uint32_t v = (uint32_t)atoi(value);
					if (v) g_mod.cfg.send_queue_size = v;
				} else if (!strcasecmp(name, "circuit-breaker-ms")) {
					uint32_t v = (uint32_t)atoi(value);
					if (v) g_mod.cfg.circuit_breaker_ms = v;
				} else if (!strcasecmp(name, "enable-cdr")) {
					g_mod.cfg.enable_cdr = switch_true(value);
				} else if (!strcasecmp(name, "cdr-table")) {
					g_mod.cfg.cdr_table = switch_core_strdup(pool, value);
				}
			}
		}
	}

	switch_xml_free(xml);
}

#ifdef HAVE_SURREALDB_FFI
static void mod_surrealdb_try_connect(void)
{
	const char *url = g_mod.cfg.url;
	const char *ns = g_mod.cfg.ns;
	const char *db = g_mod.cfg.db;
	const char *user = g_mod.cfg.user;
	const char *pass = g_mod.cfg.pass;
	const char *token = g_mod.cfg.token;

	if (!url || !ns || !db || (!token && !(user && pass))) {
		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_WARNING,
			"%s: missing configuration; need url, namespace, database and (username+password) or token. Skipping connect.\n",
			MOD_SURREALDB_NAME);
		return;
	}

    if (token && *token) {
        g_mod.handle = surreal_connect_with_token(url, ns, db, token);
    } else {
        g_mod.handle = surreal_connect(url, ns, db, user, pass);
    }

    if (g_mod.handle) {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "%s: connected to SurrealDB.\n", MOD_SURREALDB_NAME);
    } else {
        int32_t gcode = surreal_last_error_global();
        const char *why = "unknown";
        switch (gcode) {
            case -100: why = "tokio runtime init failed"; break;
            case -101: why = "runtime not initialized"; break;
            case -102: why = "ws connect failed"; break;
            case -103: why = "signin failed"; break;
            case -104: why = "token authenticate failed"; break;
            case -105: why = "use_ns/use_db failed"; break;
        }
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "%s: SurrealDB connect failed (code %d: %s).\n", MOD_SURREALDB_NAME, (int)gcode, why);
    }
}
#endif

static const char *json_find_id(const char *json)
{
	const char *p = json;
	while ((p = strstr(p, "\"id\""))) {
		p += 4;
		while (*p && (*p == ' ' || *p == '\t' || *p == ':' )) p++;
		if (*p == '"') {
			const char *start = ++p;
			while (*p && *p != '"') p++;
			if (*p == '"') {
				static char idbuf[256];
				size_t len = (size_t)(p - start);
				if (len >= sizeof(idbuf)) len = sizeof(idbuf) - 1;
				memcpy(idbuf, start, len);
				idbuf[len] = '\0';
				return idbuf;
			}
		}
	}
	return NULL;
}

static void on_command_cb(const char* topic, const char* json, void* user_data)
{
    mod_surrealdb_state_t *st = (mod_surrealdb_state_t *)user_data;
    if (!st || !topic || !json) return;
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "%s: command received on %s: %s\n", MOD_SURREALDB_NAME, topic, json);

#ifdef HAVE_SURREALDB_FFI
    const char *id = NULL;
    int ok = 0;
    char result_buf[512] = {0};

    cJSON *root = cJSON_Parse(json);
    if (root) {
        cJSON *j_id = cJSON_GetObjectItem(root, "id");
        if (j_id && cJSON_IsString(j_id)) { id = j_id->valuestring; }
        cJSON *j_action = cJSON_GetObjectItem(root, "action");
        const char *action = j_action && cJSON_IsString(j_action) ? j_action->valuestring : NULL;
        if (!zstr(action)) {
            if (!strcasecmp(action, "api")) {
                cJSON *j_cmd = cJSON_GetObjectItem(root, "cmd");
                cJSON *j_args = cJSON_GetObjectItem(root, "args");
                const char *cmd = j_cmd && cJSON_IsString(j_cmd) ? j_cmd->valuestring : NULL;
                const char *args = j_args && cJSON_IsString(j_args) ? j_args->valuestring : NULL;
                if (!zstr(cmd)) {
                    switch_stream_handle_t stream = { 0 };
                    SWITCH_STANDARD_STREAM(stream);
                    switch_status_t s = switch_api_execute(cmd, args, NULL, &stream);
                    ok = (s == SWITCH_STATUS_SUCCESS);
                    if (stream.data) {
                        snprintf(result_buf, sizeof(result_buf), "%s", (char *)stream.data);
                        switch_safe_free(stream.data);
                    }
                } else {
                    snprintf(result_buf, sizeof(result_buf), "missing cmd");
                }
            } else if (!strcasecmp(action, "originate")) {
                cJSON *j_args = cJSON_GetObjectItem(root, "args");
                const char *args = j_args && cJSON_IsString(j_args) ? j_args->valuestring : NULL;
                if (!zstr(args)) {
                    switch_stream_handle_t stream = { 0 };
                    SWITCH_STANDARD_STREAM(stream);
                    switch_status_t s = switch_api_execute("originate", args, NULL, &stream);
                    ok = (s == SWITCH_STATUS_SUCCESS);
                    if (stream.data) {
                        snprintf(result_buf, sizeof(result_buf), "%s", (char *)stream.data);
                        switch_safe_free(stream.data);
                    }
                } else {
                    snprintf(result_buf, sizeof(result_buf), "missing args");
                }
            } else if (!strcasecmp(action, "hangup")) {
                /* Fields: uuid (required), cause (optional) */
                cJSON *j_uuid = cJSON_GetObjectItem(root, "uuid");
                cJSON *j_cause = cJSON_GetObjectItem(root, "cause");
                const char *uuid = j_uuid && cJSON_IsString(j_uuid) ? j_uuid->valuestring : NULL;
                const char *cause = j_cause && cJSON_IsString(j_cause) ? j_cause->valuestring : NULL;
                if (!zstr(uuid)) {
                    char args[256];
                    if (!zstr(cause)) {
                        snprintf(args, sizeof(args), "%s %s", uuid, cause);
                    } else {
                        snprintf(args, sizeof(args), "%s", uuid);
                    }
                    switch_stream_handle_t stream = { 0 };
                    SWITCH_STANDARD_STREAM(stream);
                    switch_status_t s = switch_api_execute("uuid_kill", args, NULL, &stream);
                    ok = (s == SWITCH_STATUS_SUCCESS);
                    if (stream.data) {
                        snprintf(result_buf, sizeof(result_buf), "%s", (char *)stream.data);
                        switch_safe_free(stream.data);
                    }
                } else {
                    snprintf(result_buf, sizeof(result_buf), "missing uuid");
                }
            } else if (!strcasecmp(action, "bridge")) {
                /* Fields: uuid_a, uuid_b */
                cJSON *j_a = cJSON_GetObjectItem(root, "uuid_a");
                cJSON *j_b = cJSON_GetObjectItem(root, "uuid_b");
                const char *uuid_a = j_a && cJSON_IsString(j_a) ? j_a->valuestring : NULL;
                const char *uuid_b = j_b && cJSON_IsString(j_b) ? j_b->valuestring : NULL;
                if (!zstr(uuid_a) && !zstr(uuid_b)) {
                    char args[256];
                    snprintf(args, sizeof(args), "%s %s", uuid_a, uuid_b);
                    switch_stream_handle_t stream = { 0 };
                    SWITCH_STANDARD_STREAM(stream);
                    switch_status_t s = switch_api_execute("uuid_bridge", args, NULL, &stream);
                    ok = (s == SWITCH_STATUS_SUCCESS);
                    if (stream.data) {
                        snprintf(result_buf, sizeof(result_buf), "%s", (char *)stream.data);
                        switch_safe_free(stream.data);
                    }
                } else {
                    snprintf(result_buf, sizeof(result_buf), "missing uuid_a/uuid_b");
                }
            } else if (!strcasecmp(action, "playback")) {
                /* Fields: uuid, file, legs?(aleg|bleg|both) */
                cJSON *j_uuid = cJSON_GetObjectItem(root, "uuid");
                cJSON *j_file = cJSON_GetObjectItem(root, "file");
                cJSON *j_legs = cJSON_GetObjectItem(root, "legs");
                const char *uuid = j_uuid && cJSON_IsString(j_uuid) ? j_uuid->valuestring : NULL;
                const char *file = j_file && cJSON_IsString(j_file) ? j_file->valuestring : NULL;
                const char *legs = j_legs && cJSON_IsString(j_legs) ? j_legs->valuestring : NULL;
                if (!zstr(uuid) && !zstr(file)) {
                    char args[512];
                    if (!zstr(legs)) {
                        snprintf(args, sizeof(args), "%s %s %s", uuid, file, legs);
                    } else {
                        snprintf(args, sizeof(args), "%s %s", uuid, file);
                    }
                    switch_stream_handle_t stream = { 0 };
                    SWITCH_STANDARD_STREAM(stream);
                    switch_status_t s = switch_api_execute("uuid_broadcast", args, NULL, &stream);
                    ok = (s == SWITCH_STATUS_SUCCESS);
                    if (stream.data) {
                        snprintf(result_buf, sizeof(result_buf), "%s", (char *)stream.data);
                        switch_safe_free(stream.data);
                    }
                } else {
                    snprintf(result_buf, sizeof(result_buf), "missing uuid/file");
                }
            } else {
                snprintf(result_buf, sizeof(result_buf), "unknown action: %s", action);
            }
        } else {
            snprintf(result_buf, sizeof(result_buf), "missing action");
        }
        cJSON_Delete(root);
    } else {
        snprintf(result_buf, sizeof(result_buf), "invalid json");
    }

    if (st->handle && id) {
        for (size_t i = 0; result_buf[i]; ++i) { if (result_buf[i] == '\n' || result_buf[i] == '\r') result_buf[i] = ' '; }
        char patch[768];
        snprintf(patch, sizeof(patch),
                 "{\"status\":\"%s\",\"processed_at\":%ld,\"result\":\"%s\"}",
                 ok ? "done" : "failed", (long)time(NULL), result_buf);
        if (surreal_update(st->handle, topic, id, patch) != 0) {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_WARNING, "%s: failed to ack command id=%s on %s\n", MOD_SURREALDB_NAME, id, topic);
        }
    }
#endif
}

#ifdef HAVE_SURREALDB_FFI
typedef struct mod_surrealdb_evtmsg_s {
	char *json;
	char *table;
} mod_surrealdb_evtmsg_t;

static void mod_surrealdb_evtmsg_destroy(mod_surrealdb_evtmsg_t **pmsg)
{
	mod_surrealdb_evtmsg_t *msg = pmsg && *pmsg ? *pmsg : NULL;
	if (!msg) return;
	if (msg->json) free(msg->json);
	if (msg->table) free(msg->table);
	free(msg);
	*pmsg = NULL;
}

static void mod_surrealdb_event_handler(switch_event_t *evt)
{
	mod_surrealdb_state_t *st = (mod_surrealdb_state_t *)evt->bind_user_data;
	if (!st || !st->events_running) return;
	switch_time_t now = switch_time_now();
	if (st->cb_reset_time && now < st->cb_reset_time) return;

	mod_surrealdb_evtmsg_t *msg = (mod_surrealdb_evtmsg_t *)malloc(sizeof(*msg));
	if (!msg) return;
	memset(msg, 0, sizeof(*msg));

	char *pjson = NULL;
	switch_event_serialize_json(evt, &pjson);
	if (!pjson) { free(msg); return; }
	msg->json = pjson;
	msg->table = g_mod.cfg.event_table ? strdup(g_mod.cfg.event_table) : strdup("fs_events");

	if (switch_queue_trypush(st->send_queue, msg) != SWITCH_STATUS_SUCCESS) {
		unsigned int qsz = switch_queue_size(st->send_queue);
		st->cb_reset_time = now + (switch_time_t)g_mod.cfg.circuit_breaker_ms * 1000;
		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,
			"%s: event queue full (cap %u, size %u). Dropping events for %.1fs\n",
			MOD_SURREALDB_NAME, g_mod.cfg.send_queue_size, qsz, g_mod.cfg.circuit_breaker_ms / 1000.0);
		mod_surrealdb_evtmsg_destroy(&msg);
	}
}

static void mod_surrealdb_cdr_handler(switch_event_t *evt)
{
	mod_surrealdb_state_t *st = (mod_surrealdb_state_t *)evt->bind_user_data;
	if (!st || !st->events_running) return;
	switch_time_t now = switch_time_now();
	if (st->cb_reset_time && now < st->cb_reset_time) return;

	/* Build a compact CDR JSON from key headers */
	cJSON *root = cJSON_CreateObject();
	if (!root) return;

	#define ADD_S(key, hdr) do { const char *v = switch_event_get_header(evt, hdr); if (v && *v) cJSON_AddStringToObject(root, key, v); } while (0)
	#define ADD_I(key, hdr) do { const char *v = switch_event_get_header(evt, hdr); if (v && *v) { long long n = atoll(v); cJSON_AddNumberToObject(root, key, (double)n); } } while (0)

	ADD_S("id", "Unique-ID");
	ADD_S("sip_call_id", "variable_sip_call_id");
	ADD_S("direction", "Call-Direction");
	ADD_S("caller_id_number", "Caller-Caller-ID-Number");
	ADD_S("destination_number", "Caller-Destination-Number");
	ADD_S("ani", "Caller-ANI");
	ADD_S("hangup_cause", "Hangup-Cause");
	ADD_I("start_epoch", "variable_start_epoch");
	ADD_I("answer_epoch", "variable_answer_epoch");
	ADD_I("end_epoch", "variable_end_epoch");
	ADD_I("duration", "variable_duration");
	ADD_I("billsec", "variable_billsec");

	/* Channel info */
	ADD_S("context", "variable_user_context");
	ADD_S("dialplan", "variable_dialplan");
	ADD_S("profile", "variable_sofia_profile_name");

	#undef ADD_S
	#undef ADD_I

	char *json = cJSON_PrintUnformatted(root);
	cJSON_Delete(root);
	if (!json) return;

	mod_surrealdb_evtmsg_t *msg = (mod_surrealdb_evtmsg_t *)malloc(sizeof(*msg));
	if (!msg) { free(json); return; }
	memset(msg, 0, sizeof(*msg));
	msg->json = json;
	msg->table = g_mod.cfg.cdr_table ? strdup(g_mod.cfg.cdr_table) : strdup("fs_cdr");

	if (switch_queue_trypush(st->send_queue, msg) != SWITCH_STATUS_SUCCESS) {
		unsigned int qsz = switch_queue_size(st->send_queue);
		st->cb_reset_time = now + (switch_time_t)g_mod.cfg.circuit_breaker_ms * 1000;
		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,
			"%s: CDR queue full (cap %u, size %u). Dropping for %.1fs\n",
			MOD_SURREALDB_NAME, g_mod.cfg.send_queue_size, qsz, g_mod.cfg.circuit_breaker_ms / 1000.0);
		mod_surrealdb_evtmsg_destroy(&msg);
	}
}

static void *SWITCH_THREAD_FUNC mod_surrealdb_event_thread(switch_thread_t *thread, void *obj)
{
	mod_surrealdb_state_t *st = (mod_surrealdb_state_t *)obj;
	(void)thread;
	while (st->events_running) {
		mod_surrealdb_evtmsg_t *msg = NULL;
		if (!st->events_running) break;
		if (switch_queue_pop_timeout(st->send_queue, (void **)&msg, 500000) != SWITCH_STATUS_SUCCESS) {
			continue; /* timeout, re-check events_running */
		}
		if (!msg) continue;
		const char *table = msg->table ? msg->table : (g_mod.cfg.event_table ? g_mod.cfg.event_table : "fs_events");
		int rc = surreal_publish(st->handle, table, msg->json);
		if (rc != 0) {
			char why[512] = {0};
			int32_t n = surreal_last_error_text(st->handle, why, sizeof(why));
			if (n < 0) why[0] = '\0';
			switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_WARNING,
				"%s: event publish failed rc=%d%s%s\n", MOD_SURREALDB_NAME, rc,
				zstr(why) ? "" : ": ", zstr(why) ? "" : why);
			/* brief backoff to avoid hot spin on persistent failure */
			switch_yield(200000);
		}
		mod_surrealdb_evtmsg_destroy(&msg);
	}
	return NULL;
}

static void mod_surrealdb_start_event_sink(switch_memory_pool_t *pool)
{
	if (!(g_mod.cfg.enable_events || g_mod.cfg.enable_cdr)) return;
	if (!g_mod.handle) {
		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_WARNING, "%s: enable-events is true but not connected; skipping\n", MOD_SURREALDB_NAME);
		return;
	}
	if (g_mod.cfg.enable_events && zstr(g_mod.cfg.event_table)) {
		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_WARNING, "%s: event-table not set; skipping event sink\n", MOD_SURREALDB_NAME);
		g_mod.cfg.enable_events = SWITCH_FALSE;
	}

	/* Create bounded queue */
	if (switch_queue_create(&g_mod.send_queue, g_mod.cfg.send_queue_size, pool) != SWITCH_STATUS_SUCCESS) {
		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "%s: failed to create event queue size %u\n", MOD_SURREALDB_NAME, g_mod.cfg.send_queue_size);
		return;
	}

	/* Start worker thread */
	switch_threadattr_t *thd_attr = NULL;
	switch_threadattr_create(&thd_attr, pool);
	switch_threadattr_stacksize_set(thd_attr, SWITCH_THREAD_STACKSIZE);
	g_mod.events_running = SWITCH_TRUE;
	if (switch_thread_create(&g_mod.event_thread, thd_attr, mod_surrealdb_event_thread, &g_mod, pool)) {
		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "%s: cannot create event sender thread\n", MOD_SURREALDB_NAME);
		g_mod.events_running = SWITCH_FALSE;
		return;
	}

	/* Bind events */
	if (g_mod.cfg.enable_events) {
		char *filter = g_mod.cfg.event_filter ? switch_core_strdup(pool, g_mod.cfg.event_filter) : NULL;
		char *argv[SWITCH_EVENT_ALL] = {0};
		int subs = 0;
		if (zstr(filter) || !strcasecmp(filter, "SWITCH_EVENT_ALL") || !strcasecmp(filter, "ALL") ) {
			if (switch_event_bind_removable(MOD_SURREALDB_NAME, SWITCH_EVENT_ALL, SWITCH_EVENT_SUBCLASS_ANY, mod_surrealdb_event_handler, &g_mod, &g_mod.event_nodes[g_mod.event_nodes_count]) == SWITCH_STATUS_SUCCESS) {
				g_mod.event_nodes_count++;
			} else {
				switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "%s: failed to bind SWITCH_EVENT_ALL\n", MOD_SURREALDB_NAME);
			}
		} else {
			subs = switch_separate_string(filter, ',', argv, (sizeof(argv)/sizeof(argv[0])));
			for (int i = 0; i < subs; i++) {
				char *spec = argv[i];
				if (!spec) continue;
				while (*spec == ' ' || *spec == '\t') spec++;
				char *subclass = SWITCH_EVENT_SUBCLASS_ANY;
				char *caret = strchr(spec, '^');
				if (caret) { *caret = '\0'; subclass = caret + 1; }
				switch_event_types_t id;
				if (switch_name_event(spec, &id) == SWITCH_STATUS_SUCCESS) {
					if (switch_event_bind_removable(MOD_SURREALDB_NAME, id, subclass, mod_surrealdb_event_handler, &g_mod, &g_mod.event_nodes[g_mod.event_nodes_count]) == SWITCH_STATUS_SUCCESS) {
						g_mod.event_nodes_count++;
					} else {
						switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "%s: cannot bind event %s\n", MOD_SURREALDB_NAME, spec);
					}
				} else {
					switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_WARNING, "%s: unrecognized event %s\n", MOD_SURREALDB_NAME, spec);
				}
			}
		}
		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "%s: event sink enabled -> table=%s, subs=%d, queue=%u\n",
			MOD_SURREALDB_NAME, g_mod.cfg.event_table, g_mod.event_nodes_count, g_mod.cfg.send_queue_size);
	}

	/* Bind CDR */
	if (g_mod.cfg.enable_cdr) {
		if (zstr(g_mod.cfg.cdr_table)) {
			g_mod.cfg.cdr_table = "fs_cdr";
		}
		if (switch_event_bind_removable(MOD_SURREALDB_NAME, SWITCH_EVENT_CHANNEL_HANGUP_COMPLETE, SWITCH_EVENT_SUBCLASS_ANY, mod_surrealdb_cdr_handler, &g_mod, &g_mod.event_nodes[g_mod.event_nodes_count]) == SWITCH_STATUS_SUCCESS) {
			g_mod.event_nodes_count++;
			switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "%s: CDR sink enabled -> table=%s\n", MOD_SURREALDB_NAME, g_mod.cfg.cdr_table);
		} else {
			switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "%s: failed to bind CDR handler\n", MOD_SURREALDB_NAME);
		}
	}
}

static void mod_surrealdb_stop_event_sink(void)
{
	if (!g_mod.events_running) return;
	g_mod.events_running = SWITCH_FALSE;
	/* Unbind events */
	for (int i = 0; i < g_mod.event_nodes_count; i++) {
		if (g_mod.event_nodes[i]) switch_event_unbind(&g_mod.event_nodes[i]);
		g_mod.event_nodes[i] = NULL;
	}
	g_mod.event_nodes_count = 0;
	/* Drain queue */
	if (g_mod.event_thread) {
		switch_status_t st = SWITCH_STATUS_SUCCESS;
		switch_thread_join(&st, g_mod.event_thread);
		g_mod.event_thread = NULL;
	}
	if (g_mod.send_queue) {
		void *item = NULL;
		while (switch_queue_trypop(g_mod.send_queue, &item) == SWITCH_STATUS_SUCCESS) {
			mod_surrealdb_evtmsg_t *msg = (mod_surrealdb_evtmsg_t *)item;
			mod_surrealdb_evtmsg_destroy(&msg);
		}
		g_mod.send_queue = NULL;
	}
    }
#endif /* HAVE_SURREALDB_FFI */

SWITCH_MODULE_LOAD_FUNCTION(mod_surrealdb_load)
{
	switch_api_interface_t *api_interface = NULL;

	switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "%s: loading...\n", MOD_SURREALDB_NAME);

	mod_surrealdb_read_config(pool);
	mod_surrealdb_log_cfg();

#ifdef HAVE_SURREALDB_FFI
	if (surreal_init_runtime() != 0) {
		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "%s: runtime init failed.\n", MOD_SURREALDB_NAME);
		return SWITCH_STATUS_FALSE;
	}
	if (g_mod.cfg.connect_on_load) {
		mod_surrealdb_try_connect();
	}


	/* Warn if FFI is stubbed while command/event features are enabled */
	if ((g_mod.cfg.enable_commands || g_mod.cfg.enable_events || g_mod.cfg.enable_cdr) && g_mod.handle) {
		int is_stub = surreal_is_stub();
		if (is_stub == 1) {
			switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_WARNING,
				"%s: surrealdb_ffi built in stub mode — subscribe/poll will not read from SurrealDB. Rebuild FFI with --no-default-features --features real.\n",
				MOD_SURREALDB_NAME);
		}
	}

#ifdef HAVE_SURREALDB_FFI
    /* Wire FFI logger into FreeSWITCH logs */
    surreal_set_logger(surreal_ffi_log_cb, NULL);

    if (g_mod.cfg.enable_commands && g_mod.cfg.command_table && g_mod.handle) {
		if (surreal_subscribe(g_mod.handle, g_mod.cfg.command_table, on_command_cb, &g_mod) == 0) {
			switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "%s: subscribed to command table %s\n", MOD_SURREALDB_NAME, g_mod.cfg.command_table);
		} else {
			switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_WARNING, "%s: failed to subscribe to command table %s\n", MOD_SURREALDB_NAME, g_mod.cfg.command_table);
		}
	}
#endif

#ifdef HAVE_SURREALDB_FFI
	/* Start event sink after connect so publish works */
	mod_surrealdb_start_event_sink(pool);
#endif
#else
	switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "%s: built without SurrealDB FFI; running in no-op mode.\n", MOD_SURREALDB_NAME);

#endif

	*module_interface = switch_loadable_module_create_module_interface(pool, modname);
	SWITCH_ADD_API(api_interface, "surrealdb.publish", "Publish JSON to SurrealDB", mod_surrealdb_publish_api, "<table_or_topic> <json>");
	SWITCH_ADD_API(api_interface, "surrealdb.select", "Select rows from a table", mod_surrealdb_select_api, "<table> [limit]");
	SWITCH_ADD_API(api_interface, "surrealdb.get", "Get a single row by id", mod_surrealdb_get_api, "<table> <id>");
	SWITCH_ADD_API(api_interface, "surrealdb.update", "Update a record by id with JSON patch", mod_surrealdb_update_api, "<table> <id> <json>");

	return SWITCH_STATUS_SUCCESS;
}

SWITCH_MODULE_SHUTDOWN_FUNCTION(mod_surrealdb_shutdown)
{
	switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "%s: shutdown...\n", MOD_SURREALDB_NAME);
#ifdef HAVE_SURREALDB_FFI
	mod_surrealdb_stop_event_sink();
	if (g_mod.handle) {
		surreal_close(g_mod.handle);
		g_mod.handle = NULL;
	}
#endif
	return SWITCH_STATUS_SUCCESS;
}

SWITCH_STANDARD_API(mod_surrealdb_update_api)
{
	char *args_copy = NULL;
	char *table = NULL;
	char *id = NULL;
	char *json = NULL;

	if (zstr(cmd)) {
		stream->write_function(stream, "-ERR Usage: surrealdb.update <table> <id> <json>\n");
		return SWITCH_STATUS_SUCCESS;
	}

	args_copy = strdup(cmd);
	if (!args_copy) {
		stream->write_function(stream, "-ERR memory error\n");
		return SWITCH_STATUS_SUCCESS;
	}

	char *sp1 = strchr(args_copy, ' ');
	if (!sp1) {
		free(args_copy);
		stream->write_function(stream, "-ERR Usage: surrealdb.update <table> <id> <json>\n");
		return SWITCH_STATUS_SUCCESS;
	}
	*sp1 = '\0';
	table = args_copy;
	char *rest = mod_surrealdb_ltrim(sp1 + 1);
	char *sp2 = strchr(rest, ' ');
	if (!sp2) {
		free(args_copy);
		stream->write_function(stream, "-ERR Usage: surrealdb.update <table> <id> <json>\n");
		return SWITCH_STATUS_SUCCESS;
	}
	*sp2 = '\0';
	id = rest;
	json = mod_surrealdb_ltrim(sp2 + 1);

	if (zstr(table) || zstr(id) || zstr(json)) {
		free(args_copy);
		stream->write_function(stream, "-ERR missing args\n");
		return SWITCH_STATUS_SUCCESS;
	}

#ifdef HAVE_SURREALDB_FFI
	if (!g_mod.handle) {
		free(args_copy);
		stream->write_function(stream, "-ERR not connected\n");
		return SWITCH_STATUS_SUCCESS;
	}
	int rc = surreal_update(g_mod.handle, table, id, json);
	if (rc == 0) {
		stream->write_function(stream, "+OK updated\n");
	} else {
		char errtxt[512] = {0};
		int32_t n = surreal_last_error_text(g_mod.handle, errtxt, sizeof(errtxt));
		if (n < 0) errtxt[0] = '\0';
		if (!zstr(errtxt)) {
			stream->write_function(stream, "-ERR update failed (%d: %s)\n", rc, errtxt);
		} else {
			stream->write_function(stream, "-ERR update failed (%d)\n", rc);
		}
	}
#else
	stream->write_function(stream, "-ERR built without FFI (no-op)\n");
#endif

	free(args_copy);
	return SWITCH_STATUS_SUCCESS;
}

SWITCH_STANDARD_API(mod_surrealdb_select_api)
{
	char *args_copy = NULL;
	char *table = NULL;
	char *limit_s = NULL;
	uint32_t limit = 100;

	if (zstr(cmd)) {
		stream->write_function(stream, "-ERR Usage: surrealdb.select <table> [limit]\n");
		return SWITCH_STATUS_SUCCESS;
	}

	args_copy = strdup(cmd);
	if (!args_copy) {
		stream->write_function(stream, "-ERR memory error\n");
		return SWITCH_STATUS_SUCCESS;
	}

	table = args_copy;
	limit_s = strchr(args_copy, ' ');
	if (limit_s) {
		*limit_s++ = '\0';
		limit_s = mod_surrealdb_ltrim(limit_s);
		if (!zstr(limit_s)) {
			unsigned long l = strtoul(limit_s, NULL, 10);
			if (l > 0 && l < 1000000UL) limit = (uint32_t)l;
		}
	}

	if (zstr(table)) {
		free(args_copy);
		stream->write_function(stream, "-ERR missing table\n");
		return SWITCH_STATUS_SUCCESS;
	}

#ifdef HAVE_SURREALDB_FFI
	if (!g_mod.handle) {
		free(args_copy);
		stream->write_function(stream, "-ERR not connected\n");
		return SWITCH_STATUS_SUCCESS;
	}
	/* Allocate output buffer */
	uint32_t out_len = 65536; /* 64KB cap */
	char *out = (char *)malloc(out_len);
	if (!out) {
		free(args_copy);
		stream->write_function(stream, "-ERR memory error\n");
		return SWITCH_STATUS_SUCCESS;
	}
	int rc = surreal_select(g_mod.handle, table, limit, out, out_len);
	if (rc == 0) {
		stream->write_function(stream, "%s\n", out);
	} else {
		char errtxt[512] = {0};
		int32_t n = surreal_last_error_text(g_mod.handle, errtxt, sizeof(errtxt));
		if (n < 0) errtxt[0] = '\0';
		if (!zstr(errtxt)) {
			stream->write_function(stream, "-ERR select failed (%d: %s)\n", rc, errtxt);
		} else {
			stream->write_function(stream, "-ERR select failed (%d)\n", rc);
		}
	}
	free(out);
#else
	stream->write_function(stream, "-ERR built without FFI (no-op)\n");
#endif

	free(args_copy);
	return SWITCH_STATUS_SUCCESS;
}

SWITCH_STANDARD_API(mod_surrealdb_get_api)
{
	char *args_copy = NULL;
	char *table = NULL;
	char *id = NULL;

	if (zstr(cmd)) {
		stream->write_function(stream, "-ERR Usage: surrealdb.get <table> <id>\n");
		return SWITCH_STATUS_SUCCESS;
	}

	args_copy = strdup(cmd);
	if (!args_copy) {
		stream->write_function(stream, "-ERR memory error\n");
		return SWITCH_STATUS_SUCCESS;
	}

	char *sp = strchr(args_copy, ' ');
	if (!sp) {
		free(args_copy);
		stream->write_function(stream, "-ERR Usage: surrealdb.get <table> <id>\n");
		return SWITCH_STATUS_SUCCESS;
	}
	*sp = '\0';
	table = args_copy;
	id = mod_surrealdb_ltrim(sp + 1);

	if (zstr(table) || zstr(id)) {
		free(args_copy);
		stream->write_function(stream, "-ERR missing args\n");
		return SWITCH_STATUS_SUCCESS;
	}

#ifdef HAVE_SURREALDB_FFI
	if (!g_mod.handle) {
		free(args_copy);
		stream->write_function(stream, "-ERR not connected\n");
		return SWITCH_STATUS_SUCCESS;
	}
	uint32_t out_len = 65536;
	char *out = (char *)malloc(out_len);
	if (!out) {
		free(args_copy);
		stream->write_function(stream, "-ERR memory error\n");
		return SWITCH_STATUS_SUCCESS;
	}
	int rc = surreal_get(g_mod.handle, table, id, out, out_len);
	if (rc == 0) {
		stream->write_function(stream, "%s\n", out);
	} else {
		char errtxt[512] = {0};
		int32_t n = surreal_last_error_text(g_mod.handle, errtxt, sizeof(errtxt));
		if (n < 0) errtxt[0] = '\0';
		if (!zstr(errtxt)) {
			stream->write_function(stream, "-ERR get failed (%d: %s)\n", rc, errtxt);
		} else {
			stream->write_function(stream, "-ERR get failed (%d)\n", rc);
		}
	}
	free(out);
#else
	stream->write_function(stream, "-ERR built without FFI (no-op)\n");
#endif

	free(args_copy);
	return SWITCH_STATUS_SUCCESS;
}

static char *mod_surrealdb_ltrim(char *s)
{
	while (s && *s && (*s == ' ' || *s == '\t')) s++;
	return s;
}

SWITCH_STANDARD_API(mod_surrealdb_publish_api)
{
	char *args_copy = NULL;
	char *table = NULL;
	char *json = NULL;

	if (zstr(cmd)) {
		stream->write_function(stream, "-ERR Usage: surrealdb.publish <table_or_topic> <json>\n");
		return SWITCH_STATUS_SUCCESS;
	}

	args_copy = strdup(cmd);
	if (!args_copy) {
		stream->write_function(stream, "-ERR memory error\n");
		return SWITCH_STATUS_SUCCESS;
	}

	char *sp = strchr(args_copy, ' ');
	if (!sp) {
		free(args_copy);
		stream->write_function(stream, "-ERR Usage: surrealdb.publish <table_or_topic> <json>\n");
		return SWITCH_STATUS_SUCCESS;
	}
	*sp = '\0';
	table = args_copy;
	json = mod_surrealdb_ltrim(sp + 1);

	if (zstr(table) || zstr(json)) {
		free(args_copy);
		stream->write_function(stream, "-ERR missing args\n");
		return SWITCH_STATUS_SUCCESS;
	}

#ifdef HAVE_SURREALDB_FFI
	if (!g_mod.handle) {
		free(args_copy);
		stream->write_function(stream, "-ERR not connected\n");
		return SWITCH_STATUS_SUCCESS;
	}
    int rc = surreal_publish(g_mod.handle, table, json);
    if (rc == 0) {
        stream->write_function(stream, "+OK published\n");
    } else {
        int32_t hcode = surreal_last_error_code(g_mod.handle);
        const char *why = "unknown";
        switch (rc) {
            case -1: why = "invalid handle"; break;
            case -2: why = "reconnect failed"; break;
            case -3: why = "invalid table/topic"; break;
            case -4: why = "invalid json ptr"; break;
            case -5: why = "json parse failed"; break;
            case -6: why = "insert failed"; break;
        }
        char errtxt[512] = {0};
        int32_t n = surreal_last_error_text(g_mod.handle, errtxt, sizeof(errtxt));
        if (n < 0) errtxt[0] = '\0';
        if (!zstr(errtxt)) {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_WARNING, "%s: publish failed table=%s rc=%d last=%d (%s): %s\n", MOD_SURREALDB_NAME, table, rc, (int)hcode, why, errtxt);
            stream->write_function(stream, "-ERR publish failed (%d: %s: %s)\n", rc, why, errtxt);
        } else {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_WARNING, "%s: publish failed table=%s rc=%d last=%d (%s)\n", MOD_SURREALDB_NAME, table, rc, (int)hcode, why);
            stream->write_function(stream, "-ERR publish failed (%d: %s)\n", rc, why);
        }
    }
#else
	stream->write_function(stream, "-ERR built without FFI (no-op)\n");
#endif

	free(args_copy);
	return SWITCH_STATUS_SUCCESS;
}
