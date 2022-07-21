package OpenSRF::Transport::Redis::PeerConnection;
use strict;
use warnings;
use OpenSRF::Transport::Redis::Client;

use base qw/OpenSRF::Transport::Redis::Client/;

sub construct {
    my ($class, $connection_type, $service, $no_cache) = @_;
    __PACKAGE__->SUPER::new($connection_type, $service, $no_cache);
}

1;
