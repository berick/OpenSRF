# OpenSRF-Over-Redis

Proof of concept project to replace XMPP / Ejabberd with Redis as the
OpenSRF message transport layer.

## Notes

* No more Ejabberd (yay!)
* No more OpenSRF Routers
  * Messages have fewer hops from client to service
* Fewer layers of message packaging 
  * Messages on the bus are JSON instead of JSON packed in XML.
  * One less round of escaping / unescaping
* Message chunking baked in
* Opens the door to direct-to-drone request delivery.
  * Messages can be popped from the queue directy by drones instead
    of having them go to the listener then have the listener pipe
    them do the drones.  Instead, the listener just manages child procs.

## Limitations

* No cross-domain (i.e. cross-brick) routing.
  * Affects some Dojo/translator UI's
  * Bricks that share a Redis instance could still communicate

