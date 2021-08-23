package OpenSRF::Transport::Redis;
use strict;
use warnings;

sub get_peer_client { 
    return 'OpenSRF::Transport::Redis::PeerConnection'; 
}

sub get_msg_envelope { 
    return 'OpenSRF::Transport::Redis::MessageWrapper'; 
}

