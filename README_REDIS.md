# OpenSRF-Over-Redis

Proof of concept project to replace XMPP / Ejabberd with Redis as the
OpenSRF message transport layer.

## Install
```sh
sudo apt install redis-server libredis-perl libhiredis-dev
```

## Benefits

* Speed
* No more Ejabberd (yes please!)
  * Resolves https://bugs.launchpad.net/opensrf/+bug/1703411 
  * Maybe others?
* No more OpenSRF Routers
  * Messages have fewer hops from client to service
* One less round of message packing and unpacking
  * Messages on the bus are JSON instead of JSON packed in XML.
* Automatic time-based client reconnections (in Perl anyway, need to check C)
* Message chunking baked in to implementation
* 'redis-cli monitor' command is very useful for debugging
  * TODO add to cli demo
* OpenSRF request "backlog" no longer needed.  Unprocessed requests
  will stay in the Redis message queue.

## Opportunities

* Opens the door to direct-to-drone request delivery.
  * Messages can be popped from the request queue directy by drones
    instead of having them go through the listener.  The listener would
    just manages child procs.

## Limitations

* No cross-domain (i.e. cross-brick) routing.
  * Affects some Dojo/translator UI's
  * NOTE: Bricks that share a Redis instance could still cross-communicate
* Requests sent to a service that is not running will linger unanswered
  instead of resulting in a not-found response.

## Private Service Security

### PoC Approach

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

* Redis max value size is 512MB.  May want to implement a max-size control
  within OpenSRF to prevent unintentionally sending messages of X size.
  * Unknown how a message that large would affect the system overall
* Redis supports expiration of values (similar to memcache) but expire
  times are not reset on list value modifications (lpush, lpop, etc.)

