package OpenSRF::Transport::Redis::PeerConnection;
use strict;
use base qw/OpenSRF::Transport::Redis::Client/;
use OpenSRF::Utils::Config;
use OpenSRF::Utils::Logger qw(:level);

our $_singleton_connection;

sub retrieve {
    my ($class, $app) = @_;
    return $_singleton_connection;
}

sub reset {
    return unless $_singleton_connection;
    $_singleton_connection->disconnect;
    $_singleton_connection = undef;
}


sub new {
    my ($class, $app) = @_;

    my $peer_con = $class->retrieve;
    return $peer_con if ($peer_con and $peer_con->tcp_connected);

    my $config = OpenSRF::Utils::Config->current;

    die "No suitable config found for PeerConnection\n" unless $config;

    my $conf = OpenSRF::Utils::Config->current;
    my $port = $conf->bootstrap->port || 6379;
    my $host = $conf->bootstrap->host || '127.0.0.1';
    my $sock = $conf->bootstrap->sock;

    my $self = $class->SUPER::new(
        host => $host,
        port => $port,
        sock => $sock
    );

    bless($self, $class);

    $self->app($app);

    return $_singleton_connection = $self;
}

sub process {
    my $self = shift;
    my $val = $self->SUPER::process(@_);
    return 0 unless $val;
    return OpenSRF::Transport->handler($self->app, $val);
}

sub app {
    my $self = shift;
    my $app = shift;
    $self->{app} = $app if $app;
    return $self->{app};
}

1;

