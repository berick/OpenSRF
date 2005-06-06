/* ---------------------------------------------------------------------------------------
	JSON parser.
 * --------------------------------------------------------------------------------------- */


#include <stdio.h>
#include "object.h"
#include "utils.h"

#ifndef JSON_PARSER_H
#define JSON_PARSER_H




/* returns NULL on error.  if string is NULL, returns an object whose is_null flag  
 * is set to true */
object* json_parse_string(char* string);

/* does the actual parsing work.  returns 0 on success.  -1 on error and
 * -2 if there was no object to build (string was all comments) 
 */
int _json_parse_string(char* string, unsigned long* index, object* obj);

/* returns 0 on success and turns obj into a string object */
int json_parse_json_string(char* string, unsigned long* index, object* obj);

/* returns 0 on success and turns obj into a number or double object */
int json_parse_json_number(char* string, unsigned long* index, object* obj);

int json_parse_json_object(char* string, unsigned long* index, object* obj);

/* returns 0 on success and turns object into an array object */
int json_parse_json_array(char* string, unsigned long* index, object* obj);

/* churns through whitespace and increments index as it goes.
 * eat_all means we should eat newlines, tabs
 */
void json_eat_ws(char* string, unsigned long* index, int eat_all);

int json_parse_json_bool(char* string, unsigned long* index, object* obj);

/* removes comments from a json string.  if the comment contains a class hint
 * and class_hint isn't NULL, an allocated char* with the class name will be
 * shoved into *class_hint.  returns 0 on success, -1 on parse error.
 * 'index' is assumed to be at the second character (*) of the comment
 */
int json_eat_comment(char* string, unsigned long* index, char** class_hint, int parse_class);

/* prints a useful error message to stderr. always returns -1 */
int json_handle_error(char* string, unsigned long* index, char* err_msg);

/* returns true if c is 0-9 */
int is_number(char c);

int json_parse_json_null(char* string, unsigned long* index, object* obj);


#endif
