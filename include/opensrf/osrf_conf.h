
#ifndef OSRF_CONF_H
#define OSRF_CONF_H

#include <libfyaml.h>
#include <opensrf/utils.h>
#include <opensrf/log.h>
#include <opensrf/osrf_list.h>
#include <opensrf/osrf_hash.h>
#include <opensrf/string_array.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    Public,
    Private
} osrfBusNodeType;

typedef struct {
    char* username;
    char* password;
} osrfBusCredentials;

typedef struct {
    char* name;
    int port;
    osrfStringArray* allowed_services;
} osrfBusNode;

typedef struct {
    char* name;
    osrfBusNode* private_node;
    osrfBusNode* public_node;
} osrfBusDomain;

typedef struct {
    int log_level;
    char* log_file;
    char* syslog_facility;
    char* activity_log_facility;
    char* log_tag;
} osrfLogOptions;

typedef struct {
    osrfBusNodeType node_type;
    osrfBusCredentials* credentials;
    osrfLogOptions* logging;

} osrfBusConnectionType;

typedef struct {
    int port;
    char* domain_name;
    char* node_name;
    osrfBusConnectionType* connection_type;
} osrfBusConnection;

typedef struct {

    // Our runtime hostname.
    // For services, this determines which services we host and their settings.
    char* hostname;

    // Our primary domain.
    char* domain;

    // Hash of connection name to connection type.
    osrfHash* connection_types;

    // Hash of name to osrfBusCredentials
    osrfHash* credentials;

    // List of osrfBusDomain's
    osrfList* domains;

    // Hash of group name to osrfStringArray of service names.
    osrfHash* service_groups;

    // Which api name prefixes to obfuscate in INFO logs.
    osrfStringArray* log_protect;

    // Default logging options.
    osrfLogOptions* log_defaults;

    // Connection configus our OpenSRF clients will use by default.
    osrfBusConnection* primary_connection;

    // YAML source document
    struct fy_document* source;
} osrfConf;

osrfConf* osrfConfInit(const char* filename, const char* connection_type);

int osrfConfHasDefaultConfig();

osrfConf* osrfConfDefault();

void osrfConfSetHostName(osrfConf* conf, const char* name);
void osrfConfSetDomainName(osrfConf* conf, const char* name);

int osrfConfSetPrimaryConnection(osrfConf* conf, const char* domain, const char* connection_type);

#ifdef __cplusplus
}
#endif

#endif

