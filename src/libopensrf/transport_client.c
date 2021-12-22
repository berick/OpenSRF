#include <opensrf/transport_client.h>

static void client_message_handler( void* client, transport_message* msg );

transport_client* client_init(const char* server, int port, const char* unix_path) {

	if(server == NULL) return NULL;

	/* build and clear the client object */
	transport_client* client = safe_malloc( sizeof( transport_client) );

	/* start with an empty message queue */
	client->msg_q_head = NULL;
	client->msg_q_tail = NULL;
	client->bus = NULL;
	client->bus_id = NULL;

    client->port = port;
    client->host = server ? strdup(server) : NULL;
    client->unix_path = unix_path ? strdup(unix_path) : NULL;

	client->session->message_callback = client_message_handler;
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

    const char* md5 = md5sum(junk);

    size_t len = 14 + strlen(bus_name);
    char bus_id[len];
    snprintf(bus_id, len, "%s-%s", bus_name, md5);

	client->bus_id = strdup(bus_id);
    free(md5);

    osrfLogDebug(OSRF_LOG_MARK, 
        "Transport client connecting with bus id: %s", client->bus_id);

    if (client->host && client->port) {
        client->bus = redisConnect(client->host, client->port);
    } else if (client->unix_path) {
        client->bus = redisConnectUnix(client->unix_path);
    }

    return client->bus == NULL ? 0 : 1;
}

int client_disconnect(transport_client* client) {
	if (client == NULL || client->bus == NULL) { return 0; }
    redisDisconnect(client->bus);
    client->bus = NULL;
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
    // OSRF_MSG_CHUNK_SIZE
    int offset = 0;
    int msg_len = strlen(msg->msg_json);

    while (offset < msg_len) {

        char* chunk[OSRF_MSG_CHUNK_SIZE + 1];
        snprintf(chunk, OSRF_MSG_CHUNK_SIZE + 1, msg->msg_json + offset);

        offset += OSRF_MSG_CHUNK_SIZE;

        redisReply *reply;
        if (offset < msg_len) {
            reply = redisCommand(client->bus, "RPUSH %s %s", msg->recipient, chunk);
        } else {
            // Final chunk. Append ETX / End Of Text character
            reply = redisCommand(client->bus, "RPUSH %s %s\x03", msg->recipient, chunk);
        }

        if (handle_redis_error(reply)) { return -1; }

        if (!reply || reply->type == REDIS_REPLY_ERROR) {
            char* err = reply == NULL ? "" : reply->str;
            osrfLogError(OSRF_LOG_MARK, "Error in client_send_message(): %", err);
            return -1;
        }

        freeReplyObject(reply);
    }
    
    return 1;
}

// Returns true if if the reply was NULL or an error.
// If the reply is an error, the reply is FREEd.
int handle_redis_error(redisReply* reply) {

    if (reply == NULL || reply->type == REDIS_REPLY_ERROR) {
        char* err = reply == NULL ? "" : reply->str;
        osrfLogError(OSRF_LOG_MARK, "Error in redisCommand(): %", err);
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

        reply = redisCommand("LPOP %s", sent_to);
        if (handle_redis_error(reply)) { return NULL; }

    } else {
        
        if (timeout < 0) { // Block indefinitely

            reply = redisCommand("BLPOP %s 0", sent_to);
            if (handle_redis_error(reply)) { return NULL; }

        } else { // Block up to timeout seconds

            reply = redisCommand("BLPOP %s %d", sent_to, timeout);
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
    growing_buffer *gbuf = buffer_init(OSRF_MSG_CHUNK_SIZE);

    while (1) {
        char* chunk = recv_one_chunk(client, sent_to, timeout);

        if (chunk == NULL) {
            // Receive timed out or interrupted
            buffer_free(buf);
            return NULL;
        }

        len = buffer_add(gbuf, chunk);
        free(chunk);

        if (gbuf[len - 1] == END_OF_TEXT_CHAR) {
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
        osrfLogWarn(OSRF_LOG_MARK, "Error parsing JSON: %s", gbuf->buf);
    }

    buffer_free(buf);

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

transport_message* client_recv( transport_client* client, int timeout ) {
	if (client == NULL || client->bus == NULL) { return NULL; }

	int error = 0;  /* boolean */

	if( NULL == client->msg_q_head ) {

		// No message available on the queue?  Try to get a fresh one.

		// When we call session_wait(), it reads a socket for new messages.  When it finds
		// one, it enqueues it by calling the callback function client_message_handler(),
		// which we installed in the transport_session when we created the transport_client.

		// Since a single call to session_wait() may not result in the receipt of a complete
		// message. we call it repeatedly until we get either a message or an error.

		// Alternatively, a single call to session_wait() may result in the receipt of
		// multiple messages.  That's why we have to enqueue them.

		// The timeout applies to the receipt of a complete message.  For a sufficiently
		// short timeout, a sufficiently long message, and a sufficiently slow connection,
		// we could timeout on the first message even though we're still receiving data.
		
		// Likewise we could time out while still receiving the second or subsequent message,
		// return the first message, and resume receiving messages later.

		if( timeout == -1 ) {  /* wait potentially forever for data to arrive */

			int x;
			do {
				if( (x = session_wait( client->session, -1 )) ) {
					osrfLogDebug(OSRF_LOG_MARK, "session_wait returned failure code %d\n", x);
					error = 1;
					break;
				}
			} while( client->msg_q_head == NULL );

		} else {    /* loop up to 'timeout' seconds waiting for data to arrive  */

			/* This loop assumes that a time_t is denominated in seconds -- not */
			/* guaranteed by Standard C, but a fair bet for Linux or UNIX       */

			time_t start = time(NULL);
			time_t remaining = (time_t) timeout;

			int wait_ret;
			do {
				if( (wait_ret = session_wait( client->session, (int) remaining)) ) {
					error = 1;
					osrfLogDebug(OSRF_LOG_MARK,
						"session_wait returned failure code %d: setting error=1\n", wait_ret);
					break;
				}

				remaining -= time(NULL) - start;
			} while( NULL == client->msg_q_head && remaining > 0 );
		}
	}

	transport_message* msg = NULL;

	if( !error && client->msg_q_head != NULL ) {
		/* got message(s); dequeue the oldest one */
		msg = client->msg_q_head;
		client->msg_q_head = msg->next;
		msg->next = NULL;  /* shouldn't be necessary; nullify for good hygiene */
		if( NULL == client->msg_q_head )
			client->msg_q_tail = NULL;
	}

	return msg;
}

/**
	@brief Enqueue a newly received transport_message.
	@param client A pointer to a transport_client, cast to a void pointer.
	@param msg A new transport message.

	Add a newly arrived input message to the tail of the queue.

	This is a callback function.  The transport_session parses the XML coming in through a
	socket, accumulating various bits and pieces.  When it sees the end of a message stanza,
	it packages the bits and pieces into a transport_message that it passes to this function,
	which enqueues the message for processing.
*/
static void client_message_handler( void* client, transport_message* msg ){

	if(client == NULL) return;
	if(msg == NULL) return;

	transport_client* cli = (transport_client*) client;

	/* add the new message to the tail of the queue */
	if( NULL == cli->msg_q_head )
		cli->msg_q_tail = cli->msg_q_head = msg;
	else {
		cli->msg_q_tail->next = msg;
		cli->msg_q_tail = msg;
	}
	msg->next = NULL;
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
	if(client == NULL)
		return 0;
	
	transport_message* current = client->msg_q_head;
	transport_message* next;

	/* deallocate the list of messages */
	while( current != NULL ) {
		next = current->next;
		message_free( current );
		current = next;
	}

	free(client->host);
	free(client->xmpp_id);
	free( client );
	return 1;
}

