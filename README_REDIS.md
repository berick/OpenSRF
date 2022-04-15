# OpenSRF-Over-Redis

Proof of concept project to replace XMPP / Ejabberd with Redis as the
OpenSRF message transport layer.

## Install
```sh
sudo apt install redis-server libredis-perl libhiredis-dev
```

## Benefits

* Speed
* Simplicity
  * installation
  * no account management
  * conceptual movement of data.
* No more Ejabberd (yes please!)
  * Resolves https://bugs.launchpad.net/opensrf/+bug/1703411 
  * Probably resolves other stuff
* No more OpenSRF Routers
  * All messages traverse the same data bus (Redis instance)
  * Messages have fewer hops from client to service
  * See below re: securing private services.
* Two fewer rounds of message packing and unpacking
  * XMMP: data is message-wrapper:XML -> message:JSON -> message-body:EMBEDDED-JSON
  * Redis: JSON
    * NOTE: currently message-body is still EMBEDDED-JSON within the code
      for Perl and C for Proof-of-Concept purposes.  TODO item for avoiding
      the extra message body JSON round-trip.
* Automatic time-based client reconnections (in Perl anyway, need to check C)
* 'redis-cli' very useful for debugging
  * Especially 'redis-cli monitor'

## Opportunities

* Opens the door to direct-to-drone request delivery.
  * Messages can be popped from the request queue directy by drones
    hungry-hippo style instead going through the listener.  The listener 
    would just manages child procs.  In addition to more speed, would
    resolve situations where Listeners choke cramming large messages
    down their pipe-to-child.
* Listeners could still listen for command/broadcast messages
  * Shutdown, reload, dynamically raise max children, etc.
  * Requests for data, e.g. drone stats (similar to router info messages)
* OpenSRF request "backlog" no longer required.  Unprocessed requests
  can stay in the Redis message queue instead of filling up the
  listener's network buffer.

## Limitations

* No cross-domain (i.e. cross-brick) routing.
  * Affects some Dojo/translator UI's
  * NOTE: Bricks that share a Redis instance could still cross-communicate
* Requests sent to a service that is not running will linger unanswered
  instead of resulting in a not-found response.
* Cannot query the router for active services.
  * Circ, for example, queries the router to see if Booking is running.
    Could be addressed with an opensrf.xml setting for Circ, global flag, 
    etc.

## Private Service Security

### One Possible Approach

* Services are assumed to be "private" unless explicitly set to "public"
  in opensrf.xml
* A private key is stored in opensrf.xml, which is only known to 
  OpenSRF services and their clients.  Clients that have the key can 
  access private services.  Standalone clients (e.g. websocket translator,
  Perl clients, etc.) do not read opensrf.xml and won't know the key.

### Other Options

* Redis supports authentication and access control lists if we want
  to beef up the security.

## Considerations

* Redis supports expiration of values (similar to memcache) but expire
  times are not reset on list value modifications (lpush, lpop, etc.),
  so would not help with message queues.
  * Add timestamps to messages so we can sweep them?
  * Regularly delete empty queues to reset their timeout paired with
    a long expiration timeout on queues.  (e.g. days).

