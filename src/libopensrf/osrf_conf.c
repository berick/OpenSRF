#include <opensrf/osrf_conf.h>

static osrfConf* osrfConfDefault = NULL;
static int add_service_groups(osrfConf*);

osrfConf* osrfConfInit(const char* filename, const char* connection_type) {

    struct fy_document *fyd = fy_document_build_from_file(NULL, filename);

    if (fyd == NULL) {
        fprintf(stderr, "Cannot load/parse YAML file: %s", filename);
        return NULL;
    }

    char hostname[1024 + 1];
    hostname[1024] = '\0';
    gethostname(hostname, 1024);

    osrfConf* conf = (osrfConf*) safe_malloc(sizeof(osrfConf));
    conf->hostname = strdup(hostname);
    conf->connections = osrfNewHash();
    conf->credentials = osrfNewHash();
    conf->domains = osrfNewList();
    conf->service_groups = osrfNewHash();
    conf->log_protect = osrfNewStringArray(16);
    conf->log_defaults = NULL;
    conf->primary_connection = NULL;
    conf->source = fyd;

    if (!add_service_groups(conf)) { return NULL; }


    osrfConfDefault = conf;

    return conf;
}

static int add_service_groups(osrfConf* conf) {

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


