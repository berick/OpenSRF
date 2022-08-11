#ifndef TRANSPORT_CLIENT_H
#define TRANSPORT_CLIENT_H

/**
	@file transport_client.h
	@brief Header for implementation of transport_client.

	The transport_client routines provide an interface for sending and receiving Jabber
	messages, one at a time.
*/

#include <time.h>
#include <opensrf/transport_session.h>
#include <opensrf/transport_connection.h>
#include <opensrf/utils.h>
#include <opensrf/log.h>

#ifdef __cplusplus
extern "C" {
#endif

struct message_list_struct;

/**
	@brief A collection of members used for keeping track of transport_messages.

	Among other things, this struct includes a queue of incoming messages, and
	a Jabber ID for outgoing messages.
*/
struct transport_client_struct {
    char* primary_domain;
    char* service; // NULL if this is a standalone client.
    char* service_address; // NULL if this is a standalone client.
    osrfHash* connections;

    int port;
    char* username;
    char* password;
    transport_con* primary_connection;

	int error;                       /**< Boolean: true if an error has occurred */
};
typedef struct transport_client_struct transport_client;

transport_client* client_init(const char* server, 
    int port, const char* username, const char* password);

int client_connect_as_service(transport_client* client, const char* service);
int client_connect(transport_client* client); 

int client_disconnect( transport_client* client );

int client_free( transport_client* client );

int client_discard( transport_client* client );

int client_send_message( transport_client* client, transport_message* msg );

int client_connected( const transport_client* client );

transport_message* client_recv_stream(transport_client* client, int timeout, const char* stream);
transport_message* client_recv(transport_client* client, int timeout);
transport_message* client_recv_for_service(transport_client* client, int timeout);

int client_sock_fd( transport_client* client );

#ifdef __cplusplus
}
#endif

#endif
