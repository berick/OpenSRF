#include <opensrf/transport_client.h>

static int handle_redis_error(redisReply* reply);

transport_client* client_init(const char* server, int port, const char* unix_path) {

	if(server == NULL) return NULL;

	/* build and clear the client object */
	transport_client* client = safe_malloc( sizeof( transport_client) );

	/* start with an empty message queue */
	client->bus = NULL;
	client->bus_id = NULL;

    client->port = port;
    client->host = server ? strdup(server) : NULL;
    client->unix_path = unix_path ? strdup(unix_path) : NULL;
	client->error = 0;

	return client;
}

int client_connect_with_bus_id(transport_client* client, 
	const char* username, const char* password) {

    osrfLogDebug(OSRF_LOG_MARK, 
        "Transport client connecting with bus id: %s; host=%s; port=%d; unix_path=%s", 
        client->bus_id, client->host, client->port, client->unix_path);

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

    osrfLogInfo(OSRF_LOG_MARK, "Connected to Redis instance OK");

    redisReply *reply = 
        redisCommand(client->bus, "AUTH %s %s", username, password);

    osrfLogInfo(OSRF_LOG_MARK, "Sending AUTH with username=%s", username);

    // reply is free'd on error
    if (handle_redis_error(reply)) { return 0; }

    osrfLogDebug(OSRF_LOG_MARK, "Redis AUTH succeeded");

    freeReplyObject(reply);

    return 1;
}

int client_connect_as_service(transport_client* client,
	const char* appname, const char* username, const char* password) {
	if (client == NULL || appname == NULL) { return 0; }
    growing_buffer *buf = buffer_init(32);
    buffer_fadd(buf, "service:%s", appname);
    client->bus_id = buffer_release(buf);
    return client_connect_with_bus_id(client, username, password);
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

	client->bus_id = buffer_release(buf);

    free(md5);

    return client_connect_with_bus_id(client, username, password);
}

int client_disconnect(transport_client* client) {
	if (client == NULL || client->bus == NULL) { return 0; }
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
	msg->sender = strdup(client->bus_id);

    message_prepare_json(msg);

    osrfLogInternal(OSRF_LOG_MARK, 
        "client_send_message() to=%s %s", msg->recipient, msg->msg_json);

    redisReply *reply = 
        redisCommand(client->bus, "RPUSH %s %s", msg->recipient, msg->msg_json);

    if (handle_redis_error(reply)) { return -1; }

    osrfLogInternal(OSRF_LOG_MARK, "client_send_message() send completed");

    freeReplyObject(reply);
    
    return 0;
}

// Returns true if if the reply was NULL or an error.
// If the reply is an error, the reply is FREEd.
static int handle_redis_error(redisReply* reply) {

    if (reply == NULL || reply->type == REDIS_REPLY_ERROR) {
        char* err = reply == NULL ? "" : reply->str;
        osrfLogError(OSRF_LOG_MARK, "Error in redisCommand(): %s", err);
        freeReplyObject(reply);
        return 1;
    }

    return 0;
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

    size_t len = 0;
    char command_buf[256];

    if (timeout == 0) { // Non-blocking list pop

        len = snprintf(command_buf, 256, "LPOP %s", client->bus_id);

    } else {
        
        if (timeout < 0) { // Block indefinitely

            len = snprintf(command_buf, 256, "BLPOP %s 0", client->bus_id);

        } else { // Block up to timeout seconds

            len = snprintf(command_buf, 256, "BLPOP %s %d", client->bus_id, timeout);
        }
    }

    command_buf[len] = '\0';

    osrfLogInternal(OSRF_LOG_MARK, 
        "recv_one_chunk() sending command: %s", command_buf);

    redisReply* reply = redisCommand(client->bus, command_buf);
    if (handle_redis_error(reply)) { return NULL; }

    char* json = NULL;
    if (reply->type == REDIS_REPLY_STRING) { // LPOP
        json = strdup(reply->str);

    } else if (reply->type == REDIS_REPLY_ARRAY) { // BLPOP

        // BLPOP returns [list_name, popped_value]
        if (reply->elements == 2 && reply->element[1]->str != NULL) {
            json = strdup(reply->element[1]->str); 
        } else {
            osrfLogInternal(OSRF_LOG_MARK, 
                "No response returned within timeout: %d", timeout);
        }
    }

    freeReplyObject(reply);

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
	if (client->bus_id != NULL) { free(client->bus_id); }

	free(client);

	return 1;
}

