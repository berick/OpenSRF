#include <opensrf/transport_connection.h>

transport_con* transport_con_new(char* domain, int port, char* username, char* password) {
    if (domain == NULL) { return NULL; }

    transport_con* con = safe_malloc(sizeof(transport_con));

    con->bus = NULL;
    con->address = NULL;
    con->domain = strdup(domain);
    con->port = port;
    con->max_queue = 1000; // TODO pull from config

    if (username) { con->username = strdup(username); }
    if (password) { con->password = strdup(password); }
}

void transport_con_free(transport_con* con) {

   if (con == NULL) { return; } 
   if (con->bus != NULL) { free(con->bus); }
   if (con->address != NULL) { free(con->address); }
   if (con->domain != NULL) { free(con->domain); }
   if (con->username != NULL) { free(con->username); }
   if (con->password != NULL) { free(con->password); }

   free(con);
}

int transport_con_connected(transport_con* con) {
    return con->bus != NULL; // TODO is there a redis check?
} 

void transport_con_set_address(transport_con* con, char* service) {

    char hostname[1024];
    hostname[1023] = '\0';
    gethostname(hostname, 1023);

    growing_buffer *buf = buffer_init(64);
    buffer_fadd(buf, "opensrf:client:%s:%s:", con->domain, hostname);

    if (service != NULL) {
        buffer_fadd(buf, "%s:", service);
    }

    buffer_fadd(buf, "%ld", (long) getpid());

    char junk[256];
    snprintf(junk, sizeof(junk), 
        "%f%d", get_timestamp_millis(), (int) time(NULL));

    char* md5 = md5sum(junk);

    buffer_add(buf, ":");
    buffer_add_n(buf, md5, 8);

    con->address = buffer_release(buf);
}

int transport_con_connect(transport_con* con) {
    if (con == NULL) { return 0; }

    osrfLogDebug(OSRF_LOG_MARK, "Transport con connecting with bus "
        "domain=%s; address=%s; port=%d; username=%s", 
        con->domain,
        con->address,
        con->port, 
        con->username
    );

    con->bus = redisConnect(con->domain, con->port);

    if (con->bus == NULL) {
        osrfLogError(OSRF_LOG_MARK, "Could not connect to Redis instance");
        return 0;
    }

    osrfLogDebug(OSRF_LOG_MARK, "Connected to Redis instance OK");

    redisReply *reply = 
        redisCommand(con->bus, "AUTH %s %s", con->username, con->password);

    if (handle_redis_error(reply, "AUTH %s %s", con->username, con->password)) { 
        return 0; 
    }

    freeReplyObject(reply);

    reply = redisCommand(
        con->bus, 
        "XGROUP CREATE %s %s $ mkstream", 
        con->address,
        con->address,
        "$",
        "mkstream"
    );

    if (handle_redis_error(reply, 
        "XGROUP CREATE %s %s $ mkstream", 
        con->address,
        con->address,
        "$",
        "mkstream"
    )) { return 0; }

    freeReplyObject(reply);
}

int transport_con_disconnect(transport_con* con) {
    if (con == NULL || con->bus == NULL) { return 0; }

    redisReply *reply = redisCommand(con->bus, "DEL %s", con->address);

    if (!handle_redis_error(reply, "DEL %s", con->address)) {
        freeReplyObject(reply);
    }

    redisFree(con->bus);
    con->bus = NULL;

    return 1;

}

transport_con_msg* transport_con_send(transport_con* con, char* msg_json, char* stream) {
}

transport_con_msg* transport_con_recv(transport_con* con, int timeout, char* stream) {
}

void transport_con_flush_socket(transport_con* con) {
}

// Returns the reply on success, NULL on error
// On error, the reply is freed.
int handle_redis_error(redisReply *reply, char* command, ...) {

    if (reply != NULL && reply->type != REDIS_REPLY_ERROR) {
        return 0;
    }

    VA_LIST_TO_STRING(command);
    char* err = reply == NULL ? "" : reply->str;
    osrfLogError(OSRF_LOG_MARK, "REDIS Error [%s] %s", err, VA_BUF);
    freeReplyObject(reply);

    return 1;
}
