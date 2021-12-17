package OpenSRF::Transport::Redis::Client;
use strict;
use warnings;
use Redis;
use Time::HiRes q/time/;
use OpenSRF::Utils::Config;
use OpenSRF::Utils::JSON;
use OpenSRF::Utils::Logger qw/$logger/;
use OpenSRF::Transport::Redis::Message;

my $OSRF_MSG_CHUNK_SIZE = 1024;

sub new {
    my ($class, %params) = @_;
    my $self = bless({}, ref($class) || $class);
    $self->params(\%params);
    return $self;
}

sub redis {
    my ($self, $redis) = @_;
    $self->{redis} = $redis if $redis;
    return $self->{redis};
}

sub params {
    my ($self, $params) = @_;
    $self->{params} = $params if $params;
    return $self->{params};
}

sub disconnect {
    my $self = shift;
    $self->redis->quit if $self->redis;
}

sub gather { 
    my $self = shift; 
    $self->process(0); 
}

# -------------------------------------------------

sub tcp_connected {
    my $self = shift;
    return $self->redis ? 1 : 0;
}

sub connected {
    my $self = shift;
    return $self->tcp_connected;
}


sub send {
    my $self = shift;
    my $msg = OpenSRF::Transport::Redis::Message->new(@_);
    $msg->osrf_xid($logger->get_osrf_xid);
    $msg->from($self->bus_id);

    my $msg_json = $msg->to_json;

    $logger->debug("send(): $msg_json");

    my $offset = 0;
    my $msg_len = length($msg_json);

    while (1) {
        # Break the JSON up into chunks and push a stream of them.

        my $chunk = substr($msg_json, $offset, $OSRF_MSG_CHUNK_SIZE);

        $offset += $OSRF_MSG_CHUNK_SIZE;

        if ($offset < $msg_len) {
            $self->redis->rpush($msg->to, $chunk);

        } else { 
            # EOM -- Append the final null byte
            $self->redis->rpush($msg->to, $chunk . "\0");
            last;
        }
    }
}

sub initialize {

    my $self = shift;

    my $host = $self->params->{host}; 
    my $port = $self->params->{port}; 
    my $sock = $self->params->{sock}; 
    my $bus_id = $self->params->{bus_id};

    $logger->debug("Redis client connecting with bus_id $bus_id");

    my $conf = OpenSRF::Utils::Config->current;

    return 1 if $self->redis;

    # UNIX socket file takes precedence over host:port.
    my @connect_args = $sock ? (sock => $sock) : (server => "$host:$port");

    # On disconnect, try to reconnect every 100ms up to 60 seconds.
    push(@connect_args, (reconnect => 60, every => 100_000));

    $logger->debug("Connecting to bus: @connect_args");

    unless ($self->redis(Redis->new(@connect_args))) {
        throw OpenSRF::EX::Jabber("Could not connect to Redis bus with @connect_args");
        return 0;
    }

    $self->bus_id($bus_id);

    return $self;
}

sub bus_id {
    my ($self, $bus_id) = @_;
    $self->{bus_id} = $bus_id if $bus_id;
    return $self->{bus_id};
}


sub construct {
    my ( $class, $app ) = @_;
    $class->peer_handle($class->new( $app )->initialize());
}


sub process {
    my ($self, $timeout) = @_;

    $timeout ||= 0;

    # Redis does not support fractional timeouts.
    $timeout = 1 if ($timeout > 0 && $timeout < 1);

    $timeout = int($timeout);

    unless ($self->redis) {
        throw OpenSRF::EX::JabberDisconnected 
            ("This Redis instance is no longer connected to the server ");
    }

    return $self->recv($timeout);
}

# $timeout=0 means check for data without blocking
# $timeout=-1 means block indefinitely.
sub recv {
    my ($self, $timeout) = @_;

    my $to = $self->bus_id;

    my $json = '';
    while (1) {
        my $packet;

        if ($timeout == 0) {
            # Non-blocking list pop
            $packet = $self->redis->lpop($to);
        } else {
            # In Redis, timeout 0 means wait indefinitely
            $packet = $self->redis->blpop($to, $timeout == -1 ? 0 : $timeout);
        }

        # Timed out waiting for data.
        return undef unless $packet;

        $json .= $packet->[1]; # [sender, text]

        if (substr($json, length($json) - 1, 1) eq "\0") { # EOM
            chop($json); # remove the trailing null byte
            last;
        }
    }

    # TODO: debugging
    use Data::Dumper;
    $Data::Dumper::Indent = 0;

    my $resp;

    eval { $resp = OpenSRF::Utils::JSON->JSON2perl($json); };

    if ($@) {
        $logger->error("Received invalid JSON: $@ : $json");
        return undef;
    }

    $logger->internal("recv() created object" . Dumper($resp));

    return OpenSRF::Transport::Redis::Message->new(%$resp);
}


sub flush_socket {
    my $self = shift;
    return 0 unless $self->redis;
    # Remove any messages directed to me from the bus.
    $self->redis->del($self->bus_id);
    return 1;
}

1;


