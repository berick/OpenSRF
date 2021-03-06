/*
 * jabberd - Jabber Open Source Server
 * Copyright (c) 2002 Jeremie Miller, Thomas Muldowney,
 *                    Ryan Eatmon, Robert Norris
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA02111-1307USA
 */


/** ! ! !  Patched version to disable offline storage for jabberd-2.0s4 ! ! ! */

#include "sm.h"

/** @file sm/mod_offline.c
  * @brief offline storage
  * @author Robert Norris
  * $Date$
  * $Revision$
  */

typedef struct _mod_offline_st {
    int dropmessages;
    int dropsubscriptions;
} *mod_offline_t;

static mod_ret_t _offline_in_sess(mod_instance_t mi, sess_t sess, pkt_t pkt) {
    st_ret_t ret;
    os_t os;
    os_object_t o;
    os_type_t ot;
    nad_t nad;
    pkt_t queued;
    int ns, elem, attr;
    char cttl[15], cstamp[18];
    time_t ttl, stamp;

    /* if they're becoming available for the first time */
    if(pkt->type == pkt_PRESENCE && pkt->to == NULL && sess->user->top == NULL) {

        ret = storage_get(pkt->sm->st, "queue", jid_user(sess->jid), NULL, &os);
        if(ret != st_SUCCESS) {
            log_debug(ZONE, "storage_get returned %d", ret);
            return mod_PASS;
        }
        
        if(os_iter_first(os))
            do {
                o = os_iter_object(os);

                if(os_object_get(o, "xml", (void **) &nad, &ot)) {
                    queued = pkt_new(pkt->sm, nad_copy(nad));
                    if(queued == NULL) {
                        log_debug(ZONE, "invalid queued packet, not delivering");
                    } else {
                        /* check expiry as necessary */
                        if((ns = nad_find_scoped_namespace(queued->nad, uri_EXPIRE, NULL)) >= 0 &&
                           (elem = nad_find_elem(queued->nad, 1, ns, "x", 1)) >= 0 &&
                           (attr = nad_find_attr(queued->nad, elem, -1, "seconds", NULL)) >= 0) {
                            snprintf(cttl, 15, "%.*s", NAD_AVAL_L(queued->nad, attr), NAD_AVAL(queued->nad, attr));
                            ttl = atoi(cttl);

                            /* it should have a x:delay stamp, because we stamp everything we store */
                            if((ns = nad_find_scoped_namespace(queued->nad, uri_DELAY, NULL)) >= 0 &&
                               (elem = nad_find_elem(queued->nad, 1, ns, "x", 1)) >= 0 &&
                               (attr = nad_find_attr(queued->nad, elem, -1, "stamp", NULL)) >= 0) {
                                snprintf(cstamp, 18, "%.*s", NAD_AVAL_L(queued->nad, attr), NAD_AVAL(queued->nad, attr));
                                stamp = datetime_in(cstamp);

                                if(stamp + ttl <= time(NULL)) {
                                    log_debug(ZONE, "queued packet has expired, dropping");
                                    pkt_free(queued);
                                    continue;
                                }
                            }
                        }

                        log_debug(ZONE, "delivering queued packet to %s", jid_full(sess->jid));
                        pkt_sess(queued, sess);
                    }
                }
            } while(os_iter_next(os));

        os_free(os);

        /* drop the spool */
        storage_delete(pkt->sm->st, "queue", jid_user(sess->jid), NULL);
    }

    /* pass it so that other modules and mod_presence can get it */
    return mod_PASS;
}

static mod_ret_t _offline_pkt_user(mod_instance_t mi, user_t user, pkt_t pkt) {
    mod_offline_t offline = (mod_offline_t) mi->mod->private;
    int ns, elem, attr;
    os_t os;
    os_object_t o;
    pkt_t event;

    /* send messages and s10ns to the top session */
    if(user->top != NULL && (pkt->type & pkt_MESSAGE || pkt->type & pkt_S10N)) {
        pkt_sess(pkt, user->top);
        return mod_HANDLED;
    }

    /* save messages and s10ns for later */
    if((pkt->type & pkt_MESSAGE && !offline->dropmessages) ||
       (pkt->type & pkt_S10N && !offline->dropsubscriptions)) {
        log_debug(ZONE, "saving message for later");

        pkt_delay(pkt, time(NULL), user->sm->id);

        /* new object */
        os = os_new();
        o = os_object_new(os);

        os_object_put(o, "xml", pkt->nad, os_type_NAD);

        /* store it */
        switch(storage_put(user->sm->st, "queue", jid_user(user->jid), os)) {
            case st_FAILED:
                os_free(os);
                return -stanza_err_INTERNAL_SERVER_ERROR;

            case st_NOTIMPL:
                os_free(os);
                return -stanza_err_SERVICE_UNAVAILABLE;     /* xmpp-im 9.5#4 */

            default:
                os_free(os);

                /* send offline events if they asked for it */
                if((ns = nad_find_scoped_namespace(pkt->nad, uri_EVENT, NULL)) >= 0 &&
                   (elem = nad_find_elem(pkt->nad, 1, ns, "x", 1)) >= 0 &&
                   nad_find_elem(pkt->nad, elem, ns, "offline", 1) >= 0) {

                    event = pkt_create(user->sm, "message", NULL, jid_full(pkt->from), jid_full(pkt->to));

                    attr = nad_find_attr(pkt->nad, 1, -1, "type", NULL);
                    if(attr >= 0)
                        nad_set_attr(event->nad, 1, -1, "type", NAD_AVAL(pkt->nad, attr), NAD_AVAL_L(pkt->nad, attr));

                    ns = nad_add_namespace(event->nad, uri_EVENT, NULL);
                    nad_append_elem(event->nad, ns, "x", 2);
                    nad_append_elem(event->nad, ns, "offline", 3);

                    nad_append_elem(event->nad, ns, "id", 3);
                    attr = nad_find_attr(pkt->nad, 1, -1, "id", NULL);
                    if(attr >= 0)
                        nad_append_cdata(event->nad, NAD_AVAL(pkt->nad, attr), NAD_AVAL_L(pkt->nad, attr), 4);

                    pkt_router(event);
                }

                pkt_free(pkt);
                return mod_HANDLED;
        }
    }

    return mod_PASS;
}

static void _offline_user_delete(mod_instance_t mi, jid_t jid) {
    os_t os;
    os_object_t o;
    os_type_t ot;
    nad_t nad;
    pkt_t queued;
    int ns, elem, attr;
    char cttl[15], cstamp[18];
    time_t ttl, stamp;

    log_debug(ZONE, "deleting queue for %s", jid_user(jid));

    /* bounce the queue */
    if(storage_get(mi->mod->mm->sm->st, "queue", jid_user(jid), NULL, &os) == st_SUCCESS) {
        if(os_iter_first(os))
            do {
                o = os_iter_object(os);

                if(os_object_get(o, "xml", (void **) &nad, &ot)) {
                    queued = pkt_new(mi->mod->mm->sm, nad);
                    if(queued == NULL) {
                        log_debug(ZONE, "invalid queued packet, not delivering");
                    } else {
                        /* check expiry as necessary */
                        if((ns = nad_find_scoped_namespace(queued->nad, uri_EXPIRE, NULL)) >= 0 &&
                           (elem = nad_find_elem(queued->nad, 1, ns, "x", 1)) >= 0 &&
                           (attr = nad_find_attr(queued->nad, elem, -1, "seconds", NULL)) >= 0) {
                            snprintf(cttl, 15, "%.*s", NAD_AVAL_L(queued->nad, attr), NAD_AVAL(queued->nad, attr));
                            ttl = atoi(cttl);

                            /* it should have a x:delay stamp, because we stamp everything we store */
                            if((ns = nad_find_scoped_namespace(queued->nad, uri_DELAY, NULL)) >= 0 &&
                               (elem = nad_find_elem(queued->nad, 1, ns, "x", 1)) >= 0 &&
                               (attr = nad_find_attr(queued->nad, elem, -1, "stamp", NULL)) >= 0) {
                                snprintf(cstamp, 18, "%.*s", NAD_AVAL_L(queued->nad, attr), NAD_AVAL(queued->nad, attr));
                                stamp = datetime_in(cstamp);

                                if(stamp + ttl <= time(NULL)) {
                                    log_debug(ZONE, "queued packet has expired, dropping");
                                    pkt_free(queued);
                                    continue;
                                }
                            }
                        }

                        log_debug(ZONE, "bouncing queued packet from %s", jid_full(queued->from));
                        pkt_router(pkt_error(queued, stanza_err_ITEM_NOT_FOUND));
                    }
                }
            } while(os_iter_next(os));

        os_free(os);
    }
    
    storage_delete(mi->sm->st, "queue", jid_user(jid), NULL);
}

static void _offline_free(module_t mod) {
    mod_offline_t offline = (mod_offline_t) mod->private;

    free(offline);
}

int offline_init(mod_instance_t mi, char *arg) {
    module_t mod = mi->mod;
    char *configval;
    mod_offline_t offline;
    int dropmessages = 0;
    int dropsubscriptions = 0;

    if(mod->init) return 0;

    configval = config_get_one(mod->mm->sm->config, "offline.dropmessages", 0);
    if (configval != NULL)
        dropmessages = 1;
    configval = config_get_one(mod->mm->sm->config, "offline.dropsubscriptions", 0);
    if (configval != NULL)
        dropsubscriptions = 1;

    offline = (mod_offline_t) malloc(sizeof(struct _mod_offline_st));
    offline->dropmessages = dropmessages;
    offline->dropsubscriptions = dropsubscriptions;

    mod->private = offline;

    mod->in_sess = _offline_in_sess;
    mod->pkt_user = _offline_pkt_user;
    mod->user_delete = _offline_user_delete;
    mod->free = _offline_free;

    return 0;
}
