#ifndef TRANSPORT_CLIENT_H
#define TRANSPORT_CLIENT_H

/**
	@file transport_client.h
	@brief Header for implementation of transport_client.

	The transport_client routines provide an interface for sending and receiving Jabber
	messages, one at a time.
*/

#include <time.h>
#include <hiredis.h>
#include <opensrf/transport_session.h>
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
	transport_message* msg_q_head;   /**< Head of message queue */
	transport_message* msg_q_tail;   /**< Tail of message queue */
    redisContext* bus;
    char* bus_id;
    int port;
    char* unix_path;
	int error;                       /**< Boolean: true if an error has occurred */
	char* host;                      /**< Domain name or IP address of the Jabber server */
	char* xmpp_id;                   /**< Jabber ID used for outgoing messages */
};
typedef struct transport_client_struct transport_client;

transport_client* client_init( const char* server, int port, const char* unix_path );

int client_connect_with_bus_id(transport_client* client); 
int client_connect_as_service(transport_client* client, const char* appname); 
int client_connect(transport_client* client, const char* appname); 

int client_disconnect( transport_client* client );

int client_free( transport_client* client );

int client_discard( transport_client* client );

int client_send_message( transport_client* client, transport_message* msg );

int client_connected( const transport_client* client );

transport_message* client_recv( transport_client* client, int timeout );

int client_sock_fd( transport_client* client );

#ifdef __cplusplus
}
#endif

#endif
