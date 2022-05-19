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

    # Bus ID is our address on the bus -- how people talk to us.
    my $bus_id = $app eq 'client' ? 'client:' : "client:$app:";
    $bus_id .= substr(md5_hex($$ . time . rand($$)), 0, 16);

    # Regex here so we can accommodate e.g. private.localhost
    # Over time, "domain" should be either "private" or "public"
    $domain = ($domain =~ /^private/) ? 'private' : 'public';

    # How I authenticate
    my $username = "$user\@$domain";

    $logger->info("PeerConnection::new() bus_id=$bus_id username=$username");

    my $self = $class->SUPER::new(
        channel => $domain,
        username => $username,
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

