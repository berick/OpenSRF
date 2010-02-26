#ifndef OSRF_APPLICATION_H
#define OSRF_APPLICATION_H

/**
	@file osrf_application.h
	@brief Routines to load and manage shared object libraries.
*/

#include <opensrf/utils.h>
#include <opensrf/log.h>
#include <opensrf/osrf_app_session.h>
#include <opensrf/osrf_hash.h>

#include <opensrf/osrf_json.h>
#include <stdio.h>
#include <dlfcn.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
	All OpenSRF methods take the signature
	int methodName( osrfMethodContext* );
	If a negative number is returned, it means an unknown error occured and an exception
	will be returned to the client automatically.
	If a positive number is returned, it means that libopensrf should send a 'Request Complete'
	message following any messages sent by the method.
	If 0 is returned, it tells libopensrf that the method completed successfully and
	there is no need to send any further data to the client.
*/

/** This macro verifies that methods receive the correct parameters */
#define _OSRF_METHOD_VERIFY_CONTEXT(d) \
	if(!d) return -1; \
	if(!d->session) { \
		 osrfLogError( OSRF_LOG_MARK, "Session is NULL in app request" ); \
		 return -1; \
	} \
	if(!d->method) { \
		osrfLogError( OSRF_LOG_MARK, "Method is NULL in app request" ); \
		return -1; \
	} \
	if(d->method->argc) { \
		if(!d->params) { \
			osrfLogError( OSRF_LOG_MARK, "Params is NULL in app request %s", d->method->name ); \
			return -1; \
		} \
		if( d->params->type != JSON_ARRAY ) { \
			osrfLogError( OSRF_LOG_MARK, "'params' is not a JSON array for method %s", \
				d->method->name); \
			return -1; } \
	} \
	if( !d->method->name ) { \
		osrfLogError( OSRF_LOG_MARK, "Method name is NULL"); return -1; \
	}

#ifdef OSRF_LOG_PARAMS
#define OSRF_METHOD_VERIFY_CONTEXT(d) \
	_OSRF_METHOD_VERIFY_CONTEXT(d); \
	char* __j = jsonObjectToJSON(d->params); \
	if(__j) { \
		osrfLogInfo( OSRF_LOG_MARK, "CALL:\t%s %s - %s", d->session->remote_service, \
				d->method->name, __j);\
		free(__j); \
	}
#else
#define OSRF_METHOD_VERIFY_CONTEXT(d) _OSRF_METHOD_VERIFY_CONTEXT(d);
#endif

#define OSRF_METHOD_SYSTEM          1
#define OSRF_METHOD_STREAMING       2
#define OSRF_METHOD_ATOMIC          4
#define OSRF_METHOD_CACHABLE        8

typedef struct {
	char* name;                 /**< the method name. */
	char* symbol;               /**< the symbol name (function name). */
	char* notes;                /**< public method documentation. */
	int argc;                   /**< how many args this method expects. */
	//char* paramNotes;         /**< Description of the params expected for this method. */
	int options;                /**< bitswitches setting various options for this method. */
	void* userData;             /**< Opaque pointer to application-specific data. */

	/*
	int sysmethod;
	int streaming;
	int atomic;
	int cachable;
	*/
} osrfMethod;

typedef struct {
	osrfAppSession* session;    /**< the current session. */
	osrfMethod* method;         /**< the requested method. */
	jsonObject* params;         /**< the params to the method. */
	int request;                /**< request id. */
	jsonObject* responses;      /**< array of cached responses. */
} osrfMethodContext;

/**
	Register an application
	@param appName The name of the application
	@param soFile The library (.so) file that implements this application
	@return 0 on success, -1 on error
*/
int osrfAppRegisterApplication( const char* appName, const char* soFile );

/**
	@brief Register a method for a given application.
	
	@param appName Name of the application that implements the method.
	@param methodName The fully qualified name of the method.
	@param symbolName The symbol name (function name) that implements the method.
	@param notes Public documentation for this method.
	@params argc The number of arguments this method expects.
	@param options Bit switches setting various options.
	@return 0 on success, -1 on error

	Any method with  the OSRF_METHOD_STREAMING option set will have a ".atomic"
	version of the method registered automatically.
*/
int osrfAppRegisterMethod( const char* appName, const char* methodName,
		const char* symbolName, const char* notes, int argc, int options );

int osrfAppRegisterExtendedMethod( const char* appName, const char* methodName,
		const char* symbolName, const char* notes, int argc, int options, void* );

osrfMethod* _osrfAppFindMethod( const char* appName, const char* methodName );

/**
	Runs the specified method for the specified application.
	@param appName The name of the application who's method to run
	@param methodName The name of the method to run
	@param ses The app session attached to this request
	@params reqId The request id for this request
	@param params The method parameters
*/
int osrfAppRunMethod( const char* appName, const char* methodName,
		osrfAppSession* ses, int reqId, jsonObject* params );

/**
	@brief Respond to the client with a method exception.
	@param ses The current session.
	@param request The request id.
	@param msg The debug message to send to the client.
	@return 0 on successfully sending of the message, -1 otherwise.
*/
int osrfAppRequestRespondException( osrfAppSession* ses, int request, const char* msg, ... );

int osrfAppRespond( osrfMethodContext* context, const jsonObject* data );
int osrfAppRespondComplete( osrfMethodContext* context, const jsonObject* data );

/* OSRF_METHOD_ATOMIC and/or OSRF_METHOD_CACHABLE and/or 0 for no special options */
//int osrfAppProcessMethodOptions( char* method );

/** Tell the backend process to run its child init function */
int osrfAppRunChildInit(const char* appname);

void osrfAppRunExitCode( void );

/**
	Determine whether the context looks healthy.
	Return 0 if it does, or -1 if it doesn't.
*/
int osrfMethodVerifyContext( osrfMethodContext* ctx );

#ifdef __cplusplus
}
#endif

#endif
