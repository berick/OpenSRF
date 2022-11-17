package OpenSRF::Transport::Redis::PeerConnection;
use strict;
use warnings;
use OpenSRF::Transport::Redis::Client;

use base qw/OpenSRF::Transport::Redis::Client/;

sub construct {
    my ($class, $service, $no_cache) = @_;
    return __PACKAGE__->SUPER::new($service, $no_cache);
}

sub process {
	my $self = shift;
	my $msg = $self->SUPER::process(@_);
	return 0 unless $msg;
	return OpenSRF::Transport->handler($self->service, $msg);
}


1;
