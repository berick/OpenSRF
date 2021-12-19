# OpenSRF-Over-Redis

Proof of concept project to replace XMPP / Ejabberd with Redis as the
OpenSRF message transport layer.

## Notes

* No more Ejabberd (yes please!)
  * No more deprecated authentication fun (lp?)
* No more OpenSRF Routers
  * Messages have fewer hops from client to service
* Fewer layers of message packaging 
  * Messages on the bus are JSON instead of JSON packed in XML.
  * One less round of message packing and unpacking
* Message chunking baked in to implementation
* Opens the door to direct-to-drone request delivery.
  * Messages can be popped from the request queue directy by drones
    instead of having them go through the listener.  The listener would
    just manages child procs.

## Limitations

* No cross-domain (i.e. cross-brick) routing.
  * Affects some Dojo/translator UI's
  * NOTE: Bricks that share a Redis instance could still cross-communicate

