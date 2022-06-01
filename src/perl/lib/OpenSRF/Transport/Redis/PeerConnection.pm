package OpenSRF::Transport::Redis::PeerConnection;
use strict;
use base qw/OpenSRF::Transport::Redis::Client/;
use Digest::MD5 qw(md5_hex);
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
    my ($class, $app, $connection_type) = @_;

    my $peer_con = $class->retrieve;
    return $peer_con if ($peer_con and $peer_con->tcp_connected);

    my $conf = OpenSRF::Utils::Config->current->as_hash;

    $conf = $conf->{connections} or
        die "No 'connections' block in bootstrap configuration\n";

    $conf = $conf->{$connection_type} or
        die "No '$connection_type' connection in bootstrap configuration\n";

    $conf = $conf->{message_bus};

    my $port = $conf->{port} || 6379;
    my $host = $conf->{host} || '127.0.0.1';
    my $sock = $conf->{sock};
    my $username = $conf->{username};
    my $password = $conf->{password};

    my $bus_id = $app eq 'client' ? 'client:' : "client:$app:";
    $bus_id .= substr(md5_hex($$ . time . rand($$)), 0, 12);

    $logger->debug("PeerConnection::new() using app=$app username=$username bus_id=$bus_id");

    my $self = $class->SUPER::new(
        username => $username,
        password => $password,
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

