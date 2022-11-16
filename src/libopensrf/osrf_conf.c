#include <opensrf/osrf_conf.h>

static osrfConf* _osrfConfDefault = NULL;
static int setHostInfo(osrfConf*);
static int addServiceGroups(osrfConf*);
static int addCredentials(osrfConf*);
static int addDomains(osrfConf*);
static osrfBusNode* extractBusNode(osrfConf*, struct fy_node* node, char* name);
static int addLogDefaults(osrfConf*);
static int addConnectionTypes(osrfConf*);
static char* getString(struct fy_node*, const char* path);
static osrfLogOptions* buildLogOps(osrfConf*, struct fy_node*);

osrfConf* osrfConfInit(const char* filename, const char* connection_type) {

    struct fy_document *fyd = fy_document_build_from_file(NULL, filename);

    if (fyd == NULL) {
        fprintf(stderr, "Cannot load/parse yaml file: %s", filename);
        return NULL;
    }

    osrfConf* conf = (osrfConf*) safe_malloc(sizeof(osrfConf));

    conf->hostname = NULL;
    conf->domain = NULL;
    conf->connection_types = osrfNewHash();
    conf->credentials = osrfNewHash();
    conf->domains = osrfNewList();
    conf->service_groups = osrfNewHash();
    conf->log_protect = osrfNewStringArray(16);
    conf->log_defaults = NULL;
    conf->primary_connection = NULL;
    conf->source = fyd;

    if (!setHostInfo(conf)      ||
        !addLogDefaults(conf)   ||
        !addCredentials(conf)   ||
        !addServiceGroups(conf) ||
        !addDomains(conf)       ||
        !addConnectionTypes(conf)) {

        fprintf(stderr, "Cannot build config");
        return NULL;
    }

    _osrfConfDefault = conf;

    return conf;
}


static char* getString(struct fy_node* node, const char* path) {
    char buf[1024 + 1];

    char pathbuf[1024 + 1];
    snprintf(pathbuf, 1024, "%s %%1024s", path);

    int count = fy_node_scanf(node, pathbuf, buf);

    if (count == 0) {
        fprintf(stderr, "Invalid username");
        return NULL;
    }

    return strdup(buf);
}

static int addLogDefaults(osrfConf* conf) {

    struct fy_node *node = fy_document_root(conf->source);
    node = fy_node_by_path(node, "log_defaults", -1, 0);

    if (node == NULL) { // Optional
        return 1;
    }

    osrfLogOptions* ops = (osrfLogOptions*) safe_malloc(sizeof(osrfLogOptions));

    // NULL's are OK for values here.
    ops->log_file = getString(node, "/log_file");
    ops->syslog_facility = getString(node, "/syslog_facility");
    ops->activity_log_facility = getString(node, "/activity_log_facility");

    char* log_level_str = getString(node, "/log_level");
    if (log_level_str) {
        ops->log_level = atoi(log_level_str);
        free(log_level_str);
    }

    conf->log_defaults = ops;

    return 1;
}

static osrfLogOptions* buildLogOps(osrfConf* conf, struct fy_node* node) {
     osrfLogOptions* ops = (osrfLogOptions*) safe_malloc(sizeof(osrfLogOptions));

    ops->log_file = getString(node, "/log_file");
    ops->syslog_facility = getString(node, "/syslog_facility");
    ops->activity_log_facility = getString(node, "/activity_log_facility");

    char* log_level_str = getString(node, "/log_level");
    if (log_level_str) {
        ops->log_level = atoi(log_level_str);
        free(log_level_str);
    }

    // Do we have any defaults to apply?
    if (conf->log_defaults == NULL) { return ops; }

    if (ops->log_file == NULL && conf->log_defaults->log_file != NULL) {
        ops->log_file = strdup(conf->log_defaults->log_file);
    }
    if (ops->log_level == 0 && conf->log_defaults->log_level != 0) {
        ops->log_level = conf->log_defaults->log_level;
    }
    if (ops->syslog_facility == NULL && conf->log_defaults->syslog_facility != NULL) {
        ops->syslog_facility = strdup(conf->log_defaults->syslog_facility);
    }
    if (ops->activity_log_facility == NULL && conf->log_defaults->activity_log_facility != NULL) {
        ops->activity_log_facility = strdup(conf->log_defaults->activity_log_facility);
    }

    return ops;
}

static int addDomains(osrfConf* conf) {

    struct fy_node *node = fy_document_root(conf->source);
    struct fy_node *domain_list = fy_node_by_path(node, "domains", -1, 0);

    if (domain_list == NULL || !fy_node_is_sequence(domain_list)) {
        fprintf(stderr, "Invalid 'domains' setting");
        return 0;
    }

    void *iter = NULL;
    struct fy_node *domain_entry = NULL;

    while ((domain_entry = fy_node_sequence_iterate(domain_list, &iter)) != NULL) {
        osrfBusDomain* domain = (osrfBusDomain*) safe_malloc(sizeof(osrfBusDomain));

        domain->name = getString(domain_entry, "/name");
        domain->private_node = extractBusNode(conf, domain_entry, "private_node");
        domain->public_node = extractBusNode(conf, domain_entry, "public_node");

        if (domain->private_node == NULL || domain->public_node) {
            fprintf(stderr, "Cannot configure public/private nodes");
            return 0;
        }

        osrfListPush(conf->domains, domain);
    }

    return 1;
}

static osrfBusNode* extractBusNode(osrfConf* conf, struct fy_node* node, char* name) {
    struct fy_node* fy_bus_node = fy_node_by_path(node, name, -1, 0);
    osrfBusNode* bus_node = (osrfBusNode*) safe_malloc(sizeof(osrfBusNode));

    bus_node->name = getString(fy_bus_node, "/name");
    char* port_str = getString(fy_bus_node, "/port");
    char* svcgname = getString(fy_bus_node, "/allowed_services");

    if (bus_node->name == NULL || port_str == NULL) {
        fprintf(stderr, "Invalid bus node");
        return NULL;
    }

    bus_node->port = atoi(port_str);
    free(port_str);

    if (svcgname == NULL) { // Optional
        return bus_node;
    }

    osrfStringArray* services = 
        (osrfStringArray*) osrfHashGet(conf->service_groups, svcgname);

    if (services == NULL) {
        fprintf(stderr, "Invalid service group name %s", svcgname);
        free(svcgname);
        return NULL;
    }

    bus_node->allowed_services = services;
    free(svcgname);
    return bus_node;
 }


// This does not guarantee values will be set, since the caller
// has the option to manually apply values after we have init'ed.
static int setHostInfo(osrfConf* conf) {
    struct fy_node *node = fy_document_root(conf->source);

    conf->hostname = getString(node, "/hostname");
    if (conf->hostname == NULL) {
        conf->hostname = getHostName();
    }

    conf->domain = getString(node, "/domain");
    if (conf->domain == NULL) {
        conf->domain = getDomainName();
    }

    return 1;
}

void osrfConfSetHostName(osrfConf* conf, const char* name) {
    if (name == NULL) { 
        fprintf(stderr, "Attempt to set hostname to NULL");
        return; 
    }

    if (conf->hostname) {
        free(conf->hostname);
    }

    conf->hostname = strdup(name);
}

void osrfConfSetDomainName(osrfConf* conf, const char* name) {
    if (name == NULL) { 
        fprintf(stderr, "Attempt to set domain to NULL");
        return; 
    }

    if (conf->domain) {
        free(conf->domain);
    }

    conf->domain = strdup(name);
}

static int addConnectionTypes(osrfConf* conf) {

    struct fy_node *node = fy_document_root(conf->source);
    struct fy_node *connections = fy_node_by_path(node, "connections", -1, 0);

    if (connections == NULL || !fy_node_is_mapping(connections)) {
        fprintf(stderr, "Invalid 'service_groups' setting");
        return 0;
    }

    void *iter = NULL;
    struct fy_node_pair *node_pair = NULL;

    while ((node_pair = fy_node_mapping_iterate(connections, &iter)) != NULL) {
        const char* name = fy_node_get_scalar0(fy_node_pair_key(node_pair));
        struct fy_node* value = fy_node_pair_value(node_pair);

        if (!fy_node_is_mapping(value)) {
            fprintf(stderr, "Invalid connections");
            return 0;
        }

        osrfBusConnectionType* contype = 
            (osrfBusConnectionType*) safe_malloc(sizeof(osrfBusConnectionType));

        const char* node_type = getString(value, "/node_type");
        const char* credentials = getString(value, "/credentials");

        contype->node_type = (node_type == NULL || strcmp(node_type, "public")) ?
            Public : Private;

        osrfBusCredentials* creds = osrfHashGet(conf->credentials, credentials);
        if (creds == NULL) {
            fprintf(stderr, "Invalid credentials: %s", credentials);
            return 0;
        }

        contype->credentials = creds;
        contype->logging = buildLogOps(conf, value);

        osrfHashSet(conf->connection_types, contype, name);
    }

    return 1;
}

static int addCredentials(osrfConf* conf) {
    struct fy_node *node = fy_document_root(conf->source);
    struct fy_node *creds = fy_node_by_path(node, "credentials", -1, 0);

    if (creds == NULL || !fy_node_is_mapping(creds)) {
        fprintf(stderr, "Invalid 'credentials' setting");
        return 0;
    }

    void *iter = NULL;
    struct fy_node_pair *node_pair = NULL;

    while ((node_pair = fy_node_mapping_iterate(creds, &iter)) != NULL) {
        const char* key = fy_node_get_scalar0(fy_node_pair_key(node_pair));
        struct fy_node* value = fy_node_pair_value(node_pair);

        osrfBusCredentials* credentials = 
            (osrfBusCredentials*) safe_malloc(sizeof(osrfBusCredentials));

        credentials->username = getString(value, "/username");
        credentials->password = getString(value, "/password");

        if (credentials->username == NULL || credentials->password  == NULL) {
            fprintf(stderr, "Invalid credentials for %s", key);
            return 0;
        }

        osrfHashSet(conf->credentials, credentials, key);
    }

    return 1;
}

static int addServiceGroups(osrfConf* conf) {

    struct fy_node *node = fy_document_root(conf->source);
    struct fy_node *sgroups = fy_node_by_path(node, "service_groups", -1, 0);

    if (sgroups == NULL || !fy_node_is_mapping(sgroups)) {
        fprintf(stderr, "Invalid 'service_groups' setting");
        return 0;
    }

    void *iter = NULL;
    struct fy_node_pair *node_pair = NULL;

    while ((node_pair = fy_node_mapping_iterate(sgroups, &iter)) != NULL) {
        const char* key = fy_node_get_scalar0(fy_node_pair_key(node_pair));
        struct fy_node* value = fy_node_pair_value(node_pair);

        printf("Found service group: %s", key);

        if (!fy_node_is_sequence(value)) {
            fprintf(stderr, "Invalid service group list");
            return 0;
        }

        void *iter2 = NULL;
        struct fy_node *name_node;

        while ((name_node = fy_node_sequence_iterate(value, iter2)) != NULL) {
            if (!fy_node_is_scalar(name_node)) {
                fprintf(stderr, "Invalid service name config");
                return 0;
            }

            const char* name = fy_node_get_scalar0(name_node);

            printf("Found service name %s", name);
        }
    }

    return 1;
}

osrfBusConnection* osrfConfSetPrimaryConnection(osrfConf* conf, const char* domain, const char* connection_type) {
    // TODO
    return NULL;
}

int osrfConfHasDefaultConfig() {
    return _osrfConfDefault != NULL;
}

osrfConf* osrfConfDefault() {
    return _osrfConfDefault;
}

void osrfConfCleanup() {
    // TODO
}


