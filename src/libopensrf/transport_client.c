#include <opensrf/transport_client.h>

transport_client* client_init(const char* domain, 
    int port, const char* username, const char* password) {

	transport_client* client = safe_malloc(sizeof(transport_client));
    client->primary_domain = strdup(domain);
    client->connections = osrfNewHash();

    // These 2 only get values if this client works for a service.
	client->service = NULL;
	client->service_address = NULL;

    client->username = username ? strdup(username) : NULL;
    client->password = password ? strdup(password) : NULL;

    client->port = port;
    client->primary_connection = NULL;

	client->error = 0;

	return client;
}

static transport_con* client_connect_common(
    transport_client* client, const char* domain) {

    transport_con* con = transport_con_new(domain);

    osrfHashSet(client->connections, (char*) domain, (void*) con);

    return con;
}


static transport_con* get_transport_con(transport_client* client, const char* domain) {

    transport_con* con = (transport_con*) osrfHashGet(client->connections, (char*) domain);

    if (con != NULL) { return con; }

    // If we don't have the a connection for the requested domain,
    // it means we're setting up a connection to a remote domain.

    con = client_connect_common(client, domain);

    transport_con_set_address(con, NULL);

    // Connections to remote domains assume the same connection
    // attributes apply.
    transport_con_connect(con, client->port, client->username, client->password);

    return con;
}

int client_connect_as_service(transport_client* client, const char* service) {

    growing_buffer* buf = buffer_init(32);

    buffer_fadd(buf, "opensrf:service:%s", service);

    client->service_address = buffer_release(buf);
    client->service = strdup(service);

    transport_con* con = client_connect_common(client, client->primary_domain);

    transport_con_set_address(con, service);

    client->primary_connection = con;

    return transport_con_connect(
        con, client->port, client->username, client->password);
}

int client_connect(transport_client* client) {

    transport_con* con = client_connect_common(client, client->primary_domain);

    transport_con_set_address(con, NULL);

    client->primary_connection = con;

    return transport_con_connect(
        con, client->port, client->username, client->password);
}

// Disconnect all connections and remove them from the connections hash.
int client_disconnect(transport_client* client) {

    osrfHashIterator* iter = osrfNewHashIterator(client->connections);

    while (1) {
        transport_con* con = (transport_con*) osrfHashIteratorNext(iter);

        if (con == NULL) { break; }

        transport_con_disconnect(con);
        transport_con_free(con);
    }

    return 1;
}

int client_connected( const transport_client* client ) {
	return (client != NULL && client->primary_connection != NULL);
}

static char* get_domain_from_address(const char* address) {

    char* addr_copy = strdup(address);
    strtok(addr_copy, ":"); // "opensrf:"
    strtok(NULL, ":"); // "client:"
    char* domain = strtok(NULL, ":");

    if (domain) {
        // About to free addr_copy...
        domain = strdup(domain);
    } else {
        osrfLogError(OSRF_LOG_MARK, "No domain parsed from address: %s", address);
    }

    free(addr_copy);

    return domain;
}

int client_send_message(transport_client* client, transport_message* msg) {
	if (client == NULL || client->error) { return -1; }

    char* domain = get_domain_from_address(msg->recipient);

    if (!domain) { return -1; }

    transport_con* con = get_transport_con(client, domain);

    if (!con) {
        osrfLogError(
            OSRF_LOG_MARK, "Error creating connection for domain: %s", domain);
    }
    
	if (msg->sender) { free(msg->sender); }
	msg->sender = strdup(con->address);

    message_prepare_json(msg);

    osrfLogInternal(OSRF_LOG_MARK, 
        "client_send_message() to=%s %s", msg->recipient, msg->msg_json);

    return transport_con_send(con, msg->msg_json, msg->recipient);

    osrfLogInternal(OSRF_LOG_MARK, "client_send_message() send completed");
    
    return 0;
}

transport_message* client_recv_stream(transport_client* client, int timeout, const char* stream) {

	if (client == NULL || client->primary_connection == NULL) { return NULL; }

    if (stream == NULL) { stream = client->primary_connection->address; }

    transport_con_msg* con_msg = 
        transport_con_recv(client->primary_connection, timeout, stream);

    if (con_msg == NULL) { return NULL; } // Receive timed out.

	transport_message* msg = new_message_from_json(con_msg->msg_json);

    transport_con_msg_free(con_msg);

    osrfLogInternal(OSRF_LOG_MARK, 
        "client_recv() read response for thread %s", msg->thread);

	return msg;
}

transport_message* client_recv(transport_client* client, int timeout) {

    return client_recv_stream(client, timeout, client->primary_connection->address);
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

	if (client->primary_domain) { free(client->primary_domain); }
	if (client->service) { free(client->service); }
	if (client->service_address) { free(client->service_address); }
    if (client->username) { free(client->username); }
    if (client->password) { free(client->password); }

    // Avoid freeing primary_connection since it's cleared when the
    // connections hash is cleaned up in disconnect.

    osrfHashFree(client->connections);

	free(client);

	return 1;
}

