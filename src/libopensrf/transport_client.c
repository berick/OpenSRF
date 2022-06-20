#include <opensrf/transport_client.h>

static int handle_redis_error(redisReply* reply, char* command, ...);

transport_client* client_init(const char* server, int port, const char* unix_path) {

	if(server == NULL) return NULL;

	/* build and clear the client object */
	transport_client* client = safe_malloc( sizeof( transport_client) );

	/* start with an empty message queue */
	client->bus = NULL;
	client->stream_name = NULL;

    client->max_queue_size = 1000; // TODO pull from config
    client->port = port;
    client->host = server ? strdup(server) : NULL;
    client->unix_path = unix_path ? strdup(unix_path) : NULL;
	client->error = 0;

	return client;
}

int client_connect_with_stream_name(transport_client* client, 
	const char* username, const char* password) {

    osrfLogDebug(OSRF_LOG_MARK, "Transport client connecting with bus "
        "stream=%s; host=%s; port=%d; unix_path=%s", 
        client->stream_name, 
        client->host, 
        client->port, 
        client->unix_path
    );

    // TODO use redisConnectWithTimeout so we can verify connection.
    if (client->host && client->port) {
        client->bus = redisConnect(client->host, client->port);
    } else if (client->unix_path) {
        client->bus = redisConnectUnix(client->unix_path);
    }

    if (client->bus == NULL) {
        osrfLogError(OSRF_LOG_MARK, "Could not connect to Redis instance");
        return 0;
    }

    osrfLogDebug(OSRF_LOG_MARK, "Connected to Redis instance OK");

    osrfLogDebug(OSRF_LOG_MARK, "Sending AUTH with username=%s", username);

    redisReply *reply = redisCommand(client->bus, "AUTH %s %s", username, password);

    if (handle_redis_error(reply, "AUTH %s %s", username, password)) { return 0; }

    osrfLogDebug(OSRF_LOG_MARK, "Redis AUTH succeeded");

    freeReplyObject(reply);

    // Create our stream + consumer group.
    // This will produce an error when the group already exists, which
    // will happen with service-level groups.  Skip error checking.
    reply = redisCommand(
        client->bus, 
        "XGROUP CREATE %s %s $ mkstream", 
        client->stream_name, 
        client->stream_name
    );

    freeReplyObject(reply);

    return 1;
}

int client_connect_as_service(transport_client* client,
	const char* appname, const char* username, const char* password) {
	if (client == NULL || appname == NULL) { return 0; }
    growing_buffer *buf = buffer_init(32);
    buffer_fadd(buf, "service:%s", appname);

    client->stream_name = buffer_release(buf);

    return client_connect_with_stream_name(client, username, password);
}

int client_connect(transport_client* client,
	const char* appname, const char* username, const char* password) {
	if (client == NULL || appname == NULL) { return 0; }

    char junk[256];
	snprintf(junk, sizeof(junk), 
        "%f.%d%ld", get_timestamp_millis(), (int) time(NULL), (long) getpid());

    char* md5 = md5sum(junk);

    growing_buffer *buf = buffer_init(32);
    buffer_add(buf, "client:");

    if (strcmp("client", appname) == 0) {
        // Standalone client
        buffer_add_n(buf, md5, 12);
    } else {
        // Service client client:servicename:junk
        buffer_fadd(buf, "%s:", appname);
        buffer_add_n(buf, md5, 12);
    }

	client->stream_name = buffer_release(buf);

    free(md5);

    return client_connect_with_stream_name(client, username, password);
}

int client_disconnect(transport_client* client) {
	if (client == NULL || client->bus == NULL) { return 0; }

    if (strncmp(client->stream_name, "client:", 7) == 0) {
        // Delete our stream on disconnect if we are a client.
        // No point in letting it linger.
        redisReply *reply = 
            redisCommand(client->bus, "DEL %s", client->stream_name);

        freeReplyObject(reply);
    }

    redisFree(client->bus);
    client->bus = NULL;
    return 1;
}

int client_connected( const transport_client* client ) {
	return (client != NULL && client->bus != NULL);
}

int client_send_message(transport_client* client, transport_message* msg) {
	if (client == NULL || client->error) { return -1; }

	if (msg->sender) { free(msg->sender); }
	msg->sender = strdup(client->stream_name);

    message_prepare_json(msg);

    osrfLogInternal(OSRF_LOG_MARK, 
        "client_send_message() to=%s %s", msg->recipient, msg->msg_json);

    redisReply *reply = redisCommand(client->bus,
        "XADD %s MAXLEN ~ %d * message %s",
        msg->recipient, 
        client->max_queue_size,
        msg->msg_json
    );

    if (handle_redis_error(reply, 
        "XADD %s MAXLEN ~ %d * message %s",
        msg->recipient, 
        client->max_queue_size,
        msg->msg_json)
    ) { return -1; }

    osrfLogInternal(OSRF_LOG_MARK, "client_send_message() send completed");

    freeReplyObject(reply);
    
    return 0;
}

// Returns the reply on success, NULL on error
// On error, the reply is freed.
static int handle_redis_error(redisReply *reply, char* command, ...) {

    if (reply != NULL && reply->type != REDIS_REPLY_ERROR) {
        return 0;
    }

    VA_LIST_TO_STRING(command);
    char* err = reply == NULL ? "" : reply->str;
    osrfLogError(OSRF_LOG_MARK, "REDIS Error [%s] %s", err, VA_BUF);
    freeReplyObject(reply);

    return 1;
}

/*
 * Returns at most one allocated char* pulled from the bus or NULL 
 * if the pop times out or is interrupted.
 *
 * The string will be valid JSON string, a partial JSON string, or
 * a message terminator chararcter.
 */
char* recv_one_chunk(transport_client* client, int timeout) {
	if (client == NULL || client->bus == NULL) { return NULL; }

    redisReply *reply, *tmp;
    char* json = NULL;
    char* msg_id = NULL;

    if (timeout != 0) {

        if (timeout == -1) {
            // Redis timeout 0 means block indefinitely
            timeout = 0;
        } else {
            // Milliseconds
            timeout *= 1000;
        }

        reply = redisCommand(client->bus, 
            "XREADGROUP GROUP %s %s BLOCK %d COUNT 1 STREAMS %s >",
            client->stream_name,
            client->stream_name,
            timeout,
            client->stream_name
        );

    } else {

        reply = redisCommand(client->bus, 
            "XREADGROUP GROUP %s %s COUNT 1 STREAMS %s >",
            client->stream_name,
            client->stream_name,
            client->stream_name
        );
    }

    // Timeout or error
    if (handle_redis_error(
        reply,
        "XREADGROUP GROUP %s %s %s COUNT 1 STREAMS %s >",
        client->stream_name,
        client->stream_name,
        "BLOCK X",
        client->stream_name
    )) { return NULL; }

    // Unpack the XREADGROUP response, which is a nest of arrays.
    // These arrays are mostly 1 and 2-element lists, since we are 
    // only reading one item on a single stream.
    if (reply->type == REDIS_REPLY_ARRAY && reply->elements > 0) {
        tmp = reply->element[0];

        if (tmp->type == REDIS_REPLY_ARRAY && tmp->elements > 1) {
            tmp = tmp->element[1];

            if (tmp->type == REDIS_REPLY_ARRAY && tmp->elements > 0) {
                tmp = tmp->element[0];

                if (tmp->type == REDIS_REPLY_ARRAY && tmp->elements > 1) {
                    redisReply *r1 = tmp->element[0];
                    redisReply *r2 = tmp->element[1];

                    if (r1->type == REDIS_REPLY_STRING) {
                        msg_id = strdup(r1->str);
                    }

                    if (r2->type == REDIS_REPLY_ARRAY && r2->elements > 1) {
                        // r2->element[0] is the message name, which we
                        // currently don't use for anything.

                        r2 = r2->element[1];

                        if (r2->type == REDIS_REPLY_STRING) {
                            json = strdup(r2->str);
                        }
                    }
                }
            }
        }
    }

    freeReplyObject(reply); // XREADGROUP

    if (msg_id == NULL) {
        // Read timed out. 'json' will also be NULL.
        return NULL;
    }

    reply = redisCommand(client->bus, "XACK %s %s %s", 
        client->stream_name, client->stream_name, msg_id);

    if (handle_redis_error(
        reply,
        "XACK %s %s %s",
        client->stream_name, 
        client->stream_name, msg_id
    )) { return NULL; }

    freeReplyObject(reply); // XACK

    free(msg_id);

    osrfLogInternal(OSRF_LOG_MARK, "recv_one_chunk() read json: %s", json);

    return json;
}

/// Returns at most one JSON value pulled from the bus or NULL if
/// the list pop times out or the pop is interrupted by a signal.
jsonObject* recv_one_value(transport_client* client, int timeout) {

    char* json = recv_one_chunk(client, timeout);

    if (json == NULL) {
        // recv() timed out.
        return NULL;
    }

    jsonObject* obj = jsonParse(json);

    if (obj == NULL) {
        osrfLogWarning(OSRF_LOG_MARK, "Error parsing JSON: %s", json);
    }

    free(json);

    return obj;
}

/**
 * Returns at most one jsonObject returned from the data bus.
 *
 * Keeps trying until a value is returned or the timeout is exceeded.
 */
jsonObject* recv_json_value(transport_client* client, int timeout) {

    if (timeout == 0) {
        return recv_one_value(client, 0);

    } else if (timeout < 0) {
        // Keep trying until we have a result.

        while (1) {
            jsonObject* obj = recv_one_value(client, timeout);
            if (obj != NULL) { return obj; }
        }
    }

    time_t seconds = (time_t) timeout;

    while (seconds > 0) {

        time_t now = time(NULL);
        jsonObject* obj = recv_one_value(client, timeout);

        if (obj == NULL) {
            seconds -= now;
        } else {
            return obj;
        }
    }

    return NULL;
}

transport_message* client_recv(transport_client* client, int timeout) {
	if (client == NULL || client->bus == NULL) { return NULL; }

    // TODO no need for intermediate to/from JSON.  Create transport
    // message directly from received JSON object.
    jsonObject* obj = recv_json_value(client, timeout);

    if (obj == NULL) { return NULL; } // Receive timed out.

    char* json = jsonObjectToJSON(obj);

	transport_message* msg = new_message_from_json(json);

    free(json);

    osrfLogInternal(OSRF_LOG_MARK, 
        "client_recv() read response for thread %s", msg->thread);

	return msg;
}

/**
	@brief Free a transport_client, along with all resources it owns.
	@param client Pointer to the transport_client to be freed.
	@return 1 if successful, or 0 if not.  The only error condition is if @a client is NULL.
*/
int client_free( transport_client* client ) {
	if (client == NULL) { return 0; }
	return client_discard( client );
}

/**
	@brief Free a transport_client's resources, but without disconnecting.
	@param client Pointer to the transport_client to be freed.
	@return 1 if successful, or 0 if not.  The only error condition is if @a client is NULL.

	A child process may call this in order to free the resources associated with the parent's
	transport_client, but without disconnecting from Jabber, since disconnecting would
	disconnect the parent as well.
 */
int client_discard( transport_client* client ) {

	if (client == NULL) { return 0; }

	if (client->host != NULL) { free(client->host); }
	if (client->unix_path != NULL) { free(client->unix_path); }
	if (client->stream_name != NULL) { free(client->stream_name); }

	free(client);

	return 1;
}

