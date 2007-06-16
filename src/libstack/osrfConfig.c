/* defines the currently used bootstrap config file */
#include "osrfConfig.h"

static osrfConfig* osrfConfigDefault = NULL;


void osrfConfigSetDefaultConfig(osrfConfig* cfg) {
	if(cfg) {
		if( osrfConfigDefault )
			osrfConfigFree( osrfConfigDefault );
		osrfConfigDefault = cfg;
	}
}

void osrfConfigFree(osrfConfig* cfg) {
	if(cfg) {
		jsonObjectFree(cfg->config);
		free(cfg->configContext);
		free(cfg);
	}	
}


int osrfConfigHasDefaultConfig() {
	return ( osrfConfigDefault != NULL );
}


void osrfConfigCleanup() { 
	osrfConfigFree(osrfConfigDefault);
	osrfConfigDefault = NULL;
}


void osrfConfigReplaceConfig(osrfConfig* cfg, const jsonObject* obj) {
	if(!cfg || !obj) return;
	jsonObjectFree(cfg->config);
	cfg->config = jsonObjectClone(obj);	
}

osrfConfig* osrfConfigInit(char* configFile, char* configContext) {
	if(!configFile) return NULL;

	// Load XML from the configuration file
	
	xmlDocPtr doc = xmlParseFile(configFile);
	if(!doc) {
		fprintf( stderr, "osrfConfigInit: Unable to parse XML config file %s\n", configFile);
		osrfLogWarning( OSRF_LOG_MARK, "Unable to parse XML config file %s", configFile);
		return NULL;
	}

	// Translate it into a jsonObject
	
	jsonObject* json_config = xmlDocToJSON(doc);
	xmlFreeDoc(doc);

	if(!json_config ) {
		fprintf( stderr, "osrfConfigInit: xmlDocToJSON failed for config %s\n", configFile);
		osrfLogWarning( OSRF_LOG_MARK, "xmlDocToJSON failed for config %s", configFile);
		return NULL;
	}	

	// Build an osrfConfig and return it by pointer
	
	osrfConfig* cfg = safe_malloc(sizeof(osrfConfig));

	if(configContext) cfg->configContext = strdup(configContext);
	else cfg->configContext = NULL;

	cfg->config = json_config;
	
	return cfg;
}

char* osrfConfigGetValue(osrfConfig* cfg, char* path, ...) {

	if(!path) return NULL;
	if(!cfg) cfg = osrfConfigDefault;
	if(!cfg) { osrfLogWarning( OSRF_LOG_MARK, "No Config object in osrfConfigGetValue()"); return NULL; }

	VA_LIST_TO_STRING(path);

	jsonObject* obj;
	char* val = NULL;

	if(cfg->configContext) {
		obj = jsonObjectFindPath( cfg->config, "//%s%s", cfg->configContext, VA_BUF);
		if(obj) val = jsonObjectToSimpleString(jsonObjectGetIndex(obj, 0));

	} else {
		obj = jsonObjectFindPath( cfg->config, VA_BUF);
		if(obj) val = jsonObjectToSimpleString(obj);
	}

	jsonObjectFree(obj);
	return val;
}


int osrfConfigGetValueList(osrfConfig* cfg, osrfStringArray* arr, char* path, ...) {

	if(!arr || !path) return 0;
	if(!cfg) cfg = osrfConfigDefault;
	if(!cfg) { osrfLogWarning( OSRF_LOG_MARK, "No Config object!"); return -1;}

	VA_LIST_TO_STRING(path);

	jsonObject* obj;
	if(cfg->configContext) {
		obj = jsonObjectFindPath( cfg->config, "//%s%s", cfg->configContext, VA_BUF);
	} else {
		obj = jsonObjectFindPath( cfg->config, VA_BUF);
	}

	int count = 0;

	if(obj && obj->type == JSON_ARRAY ) {

		int i;
		for( i = 0; i < obj->size; i++ ) {

			char* val = jsonObjectToSimpleString(jsonObjectGetIndex(obj, i));
			if(val) {
				count++;
				osrfStringArrayAdd(arr, val);
				free(val);
			}
		}
	}

	jsonObjectFree(obj);
	return count;
}

