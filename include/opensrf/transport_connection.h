#ifndef TRANSPORT_CONNECTION_H
#define TRANSPORT_CONNECTION_H

/**
	@file transport_client.h
	@brief Header for implementation of transport_client.

	The transport_client routines provide an interface for sending and receiving Jabber
	messages, one at a time.
*/

#include <hiredis.h>
#include <opensrf/utils.h>
#include <opensrf/log.h>

#ifdef __cplusplus
extern "C" {
#endif

struct transport_con_struct {
    char* address;
    char* domain;
    int port;
    char* username;
    char* password;
    int max_queue;
    redisContext* bus;
};
typedef struct transport_con_struct transport_con;

struct transport_con_msg_struct {
    char* msg_id;
    char* msg_json;
};
typedef struct transport_con_msg_struct transport_con_msg;

transport_con* transport_con_new(char* domain, int port, char* username, char* password);

void transport_con_free(transport_con* con);

int transport_con_connected(transport_con* con);

void transport_con_set_address(transport_con* con, char* service);

int transport_con_connect(transport_con* con);

int transport_con_disconnect(transport_con* con);

int transport_con_send(transport_con* con, char* msg_json, char* stream);

transport_con_msg*  transport_con_recv(transport_con* con, int timeout, char* stream);

void transport_con_flush_socket(transport_con* con);

int handle_redis_error(redisReply *reply, char* command, ...);


#ifdef __cplusplus
}
#endif

#endif
