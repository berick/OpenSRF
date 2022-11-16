#include <opensrf/osrf_conf.h>

static osrfConf* _osrfConfDefault = NULL;
static int setHostInfo(osrfConf*);
static int addServiceGroups(osrfConf*);
static int addConnectionTypes(osrfConf*);

osrfConf* osrfConfInit(const char* filename, const char* connection_type) {

    struct fy_document *fyd = fy_document_build_from_file(NULL, filename);

    if (fyd == NULL) {
        fprintf(stderr, "Cannot load/parse YAML file: %s", filename);
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

    if (!setHostInfo(conf)) { return NULL; }
    if (!addServiceGroups(conf)) { return NULL; }
    if (!addConnectionTypes(conf)) { return NULL; }

    _osrfConfDefault = conf;

    return conf;
}


// This does not guarantee values will be set, since the caller
// has the option to manually apply values after we have init'ed.
static int setHostInfo(osrfConf* conf) {

    char text[256 + 1];
    int count = fy_document_scanf(conf->source, "/hostname %256s", text);
    if (count > 0) {
        conf->hostname = strdup(text);
    } else {
        conf->hostname = getHostName();
    }

    count = fy_document_scanf(conf->source, "/domain %256s", text);
    if (count > 0) {
        conf->domain = strdup(text);
    } else {
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

static int addConnectionTypess(osrfConf* conf) {

    struct fy_node *node = fy_document_root(conf->source);
    struct fy_node *connections = fy_node_by_path(node, "connections", -1, 0);

    if (connections == NULL || !fy_node_is_mapping(connections)) {
        fprintf(stderr, "Invalid 'service_groups' setting");
        return 0;
    }

    void *iter = NULL;
    struct fy_node_pair *node_pair = NULL;

    while ((node_pair = fy_node_mapping_iterate(connections, &iter)) != NULL) {
        const char* key = fy_node_get_scalar0(fy_node_pair_key(node_pair));
        struct fy_node* value = fy_node_pair_value(node_pair);

        if (!fy_node_is_mapping(value)) {
            fprintf(stderr, "Invalid connections");
            return 0;
        }

        // TODO 
    }
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

int osrfConfSetPrimaryConnection(osrfConf* conf, const char* domain, const char* connection_type) {
    // TODO
    return 1;
}

int osrfConfHasDefaultConfig() {
    return _osrfConfDefault != NULL;
}

osrfConf* osrfConfDefault() {
    return _osrfConfDefault;
}


