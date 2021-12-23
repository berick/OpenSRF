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

int client_connect(transport_client* client, const char* bus_name) {

	if (client == NULL || bus_name == NULL) { return 0; }

	// Create and store a Jabber ID
	if (client->bus_id) { free(client->bus_id); }

    char junk[256];
	snprintf(junk, sizeof(junk), 
        "%f.%d%ld", get_timestamp_millis(), (int) time(NULL), (long) getpid());

    char* md5 = md5sum(junk);

    size_t len = 14 + strlen(bus_name);
    char bus_id[len];
    snprintf(bus_id, len, "%s-%s", bus_name, md5);

	client->bus_id = strdup(bus_id);
    free(md5);

    // TODO
    client->port = 6379;

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
    
    } else {
        osrfLogInfo(OSRF_LOG_MARK, "Connected to Redis instance OK");
        return 1;
    }
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

    // LOOP
    // OSRF_MSG_BUS_CHUNK_SIZE
    int offset = 0;
    int msg_len = strlen(msg->msg_json);

    while (offset < msg_len) {

        char chunk[OSRF_MSG_BUS_CHUNK_SIZE + 1];
        chunk[OSRF_MSG_BUS_CHUNK_SIZE] = '\0';
        strncpy(chunk, msg->msg_json + offset, OSRF_MSG_BUS_CHUNK_SIZE);

        offset += OSRF_MSG_BUS_CHUNK_SIZE;

        osrfLogInternal(OSRF_LOG_MARK, "Sending to: %s => %s", msg->recipient, chunk);

        redisReply *reply;
        if (offset < msg_len) {
            reply = redisCommand(client->bus, "RPUSH %s %s", msg->recipient, chunk);
        } else {
            // Final chunk. Append ETX / End Of Text character
            reply = redisCommand(client->bus, "RPUSH %s %s\x03", msg->recipient, chunk);
        }

        if (handle_redis_error(reply)) { return -1; }

        freeReplyObject(reply);
    }
    
    return 1;
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
char* recv_one_chunk(transport_client* client, char* sent_to, int timeout) {
	if (client == NULL || client->bus == NULL) { return NULL; }

    redisReply* reply = NULL;

    if (timeout == 0) { // Non-blocking list pop

        reply = redisCommand(client->bus, "LPOP %s", sent_to);
        if (handle_redis_error(reply)) { return NULL; }

    } else {
        
        if (timeout < 0) { // Block indefinitely

            reply = redisCommand(client->bus, "BLPOP %s 0", sent_to);
            if (handle_redis_error(reply)) { return NULL; }

        } else { // Block up to timeout seconds

            reply = redisCommand(client->bus, "BLPOP %s %d", sent_to, timeout);
            if (handle_redis_error(reply)) { return NULL; }
        }
    }

    char* json = NULL;
    if (reply->type == REDIS_REPLY_STRING) { // LPOP
        json = strdup(reply->str);

    } else if (reply->type == REDIS_REPLY_ARRAY) { // BLPOP

        // BLPOP returns [list_name, popped_value]
        if (reply->elements == 2) {
            json = reply->element[1]->str; 
        } else {
            osrfLogInternal(OSRF_LOG_MARK, 
                "No response returned within timeout: %d", timeout);
        }
    }

    freeReplyObject(reply);

    return json;
}

/// Returns at most one JSON value pulled from the bus or nULL if
/// the list pop times out or the pop is interrupted by a signal.
jsonObject* recv_one_value(transport_client* client, char* sent_to, int timeout) {

    size_t len = 0;
    growing_buffer *gbuf = buffer_init(OSRF_MSG_BUS_CHUNK_SIZE);

    while (1) {
        char* chunk = recv_one_chunk(client, sent_to, timeout);

        if (chunk == NULL) {
            // Receive timed out or interrupted
            buffer_free(gbuf);
            return NULL;
        }

        len = buffer_add(gbuf, chunk);
        free(chunk);

        if (strcmp(gbuf->buf + len - 1, END_OF_TEXT_CHAR) == 0) {
            // Each JSON string will be terminated by the end-of-text
            // character and it will always be the last character in
            // any bus response.
            break;
        }
    }

    // Replace the end-of-text char with an end-of string for JSON parsing.
    gbuf->buf[len - 1] = '\0';

    jsonObject* obj = jsonParse(gbuf->buf);

    if (obj == NULL) {
        osrfLogWarning(OSRF_LOG_MARK, "Error parsing JSON: %s", gbuf->buf);
    }

    buffer_free(gbuf);

    return obj;
}

/**
 * Returns at most one jsonObject returned from the data bus.
 *
 * Keeps trying until a value is returned or the timeout is exceeded.
 */
jsonObject* recv_json_value(transport_client* client, char* sent_to, int timeout) {

    if (timeout == 0) {
        return recv_one_value(client, sent_to, 0);

    } else if (timeout < 0) {
        // Keep trying until we have a result.

        while (1) {
            jsonObject* obj = recv_one_value(client, sent_to, timeout);
            if (obj != NULL) { return obj; }
        }
    }

    time_t seconds = (time_t) timeout;

    while (seconds > 0) {

        time_t now = time(NULL);
        jsonObject* obj = recv_one_value(client, sent_to, timeout);

        if (obj == NULL) {
            seconds -= now;
        } else {
            return obj;
        }
    }

    return NULL;
}

transport_message* client_recv(transport_client* client, char* sent_to, int timeout) {
	if (client == NULL || client->bus == NULL) { return NULL; }

    jsonObject* obj = recv_one_value(client, sent_to, timeout);

    if (obj == NULL) { return NULL; } // Receive timed out.

    char* json = jsonObjectToJSON(obj);

	transport_message* msg = new_message_from_json(json);

    free(json);

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

