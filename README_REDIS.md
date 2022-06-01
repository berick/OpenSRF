# OpenSRF-Over-Redis

Proof of concept project to replace XMPP / Ejabberd with Redis as the
OpenSRF message transport layer.

## Install

### Install Redis version 6.x for ACL support.

NOTE: Redis v6 is the default version in Ubuntu 22.04

```sh

curl -fsSL https://packages.redis.io/gpg | sudo gpg --dearmor -o /usr/share/keyrings/redis-archive-keyring.gpg

echo "deb [signed-by=/usr/share/keyrings/redis-archive-keyring.gpg] https://packages.redis.io/deb $(lsb_release -cs) main" \
    | sudo tee /etc/apt/sources.list.d/redis.list

sudo apt update
sudo apt install redis-server libredis-perl libhiredis-dev 

```

### Disable Redis Snapshot Disk Persistence

Optional but recommended since disk persistence adds unnecessary overhead.

#### Edit /etc/redis/redis.conf and un-comment the *# save ""* line:

```conf
# Snapshotting can be completely disabled with a single empty string argument
# as in following example:
#
save ""
#
```

#### Restart Redis

```sh
sudo systemctl restart redis
```

### Install OpenSRF Config and Initialize/Reset Message Bus

```sh
# mileage may vary on these commands

sudo su opensrf 

# Backup the original config
mv /openils/conf/opensrf_core.xml /openils/conf/opensrf_core.xml.orig    

# From the OpenSRF repository root directory.
# Copy and modify the new config as needed.
# TODO move this example config into Evergreen since it references EG services.
cp examples/opensrf_core.xml.example /openils/conf/opensrf_core.xml

# reset/init message bus
osrf_control -l --reset-message-bus     

```
