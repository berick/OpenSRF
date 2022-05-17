package OpenSRF::Transport::Redis::PeerConnection;
use strict;
use base qw/OpenSRF::Transport::Redis::Client/;
use Digest::MD5 qw(md5_hex);
use OpenSRF::Utils::Config;
use OpenSRF::Utils::Logger qw/$logger/;

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

    my $domain  = $conf->bootstrap->domain   || '';
    my $host    = $conf->bootstrap->host     || '127.0.0.1';
    my $user    = $conf->bootstrap->username || 'opensrf';
    my $pass    = $conf->bootstrap->passwd;
    my $port    = $conf->bootstrap->port     || 6379;
    my $sock    = $conf->bootstrap->sock;

    if ($domain =~ /^private/) {
        $domain = 'private';
    } else {
        $domain = 'public';
    }

    my $bus_id = "$app:" . substr(md5_hex($$ . time . rand($$)), 0, 16);

    $logger->debug("PeerConnection::new() using bus id: $bus_id");

    my $self = $class->SUPER::new(
        domain => $domain,
        username => $user,
        password => $pass,
        host => $host,
        port => $port,
        sock => $sock,
        bus_id => $bus_id
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

