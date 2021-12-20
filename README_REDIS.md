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
* Message chunking baked in to implementation
* Opens the door to direct-to-drone request delivery.
  * Messages can be popped from the request queue directy by drones
    instead of having them go through the listener.  The listener would
    just manages child procs.
* 'redis-cli monitor' command is very useful for debugging
  * TODO add to cli demo

## Limitations

* No cross-domain (i.e. cross-brick) routing.
  * Affects some Dojo/translator UI's
  * NOTE: Bricks that share a Redis instance could still cross-communicate
* Sending a request to a service that is not running will linger unanswered
  instead of resulting in a not-found response.

# Private Service Security

* A private key is set at service start time, which is only known to 
  OpenSRF services and their clients.  Clients that have the key can 
  access private services.
  * TODO: Teach osrf\_control to generate the key and pass to services
* Redis supports authentication and access control lists if we want
  to beef up the security.

## Considerations

* Redis max value size is 512MB.  May want to implement a max-size control
  within OpenSRF to prevent unintentionally sending messages of X size.
  * Unknown how a message that large would affect the system overall
* Redis supports expiration of values (similar to memcache) but expire
  times are not reset on list value modifications (lpush, lpop, etc.)

