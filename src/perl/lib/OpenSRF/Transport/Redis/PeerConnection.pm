package OpenSRF::Transport::Redis::PeerConnection;
use strict;
use warnings;
use OpenSRF::Transport::Redis::Client;

use base qw/OpenSRF::Transport::Redis::Client/;

sub construct {
    my ($class, $service, $no_cache) = @_;
    return __PACKAGE__->SUPER::new($service, $no_cache);
}

1;
