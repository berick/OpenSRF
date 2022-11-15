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

    if (sgroups == NULL) {
        fprintf(stderr, "No 'service_groups' defined");
        return 0;
    }

    return 1;
}


