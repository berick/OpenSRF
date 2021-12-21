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

        if (!reply || reply->type == REDIS_REPLY_ERROR) {
            char* err = reply == NULL ? "" : reply->str;
            osrfLogError(OSRF_LOG_MARK, "Error in client_send_message(): %", err);
            return -1;
        }
    }
    
    return 1;
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
	if(client == NULL)
		return 0;
	session_free( client->session );
	client->session = NULL;
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

int client_sock_fd( transport_client* client )
{
	if( !client )
		return 0;
	else
		return client->session->sock_id;
}
