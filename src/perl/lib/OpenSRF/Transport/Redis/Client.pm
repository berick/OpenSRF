package OpenSRF::Transport::Redis::Client;
use strict;
use warnings;
use Redis;
use Time::HiRes q/time/;
use OpenSRF::Utils::Config;
use OpenSRF::Utils::JSON;
use OpenSRF::Utils::Logger qw/$logger/;
use OpenSRF::Transport::Redis::Message;

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
    if ($self->redis) {
        $self->redis->quit;
        delete $self->{redis};
    }
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
    
    # TODO
    # The upper layers turn the body into a JSON string.
    # Perl-ify it here so the final message is an un-nested JSON string.
    $msg->body(OpenSRF::Utils::JSON->JSON2perl($msg->body));

    $msg->osrf_xid($logger->get_osrf_xid);
    $msg->from($self->bus_id);

    $logger->internal("send() thread=" . $msg->thread);

    my $msg_json = $msg->to_json;

    $logger->internal("send(): $msg_json");

    $self->redis->rpush($msg->to, $msg_json);
}

sub initialize {

    my $self = shift;

    my $host = $self->params->{host}; 
    my $port = $self->params->{port}; 
    my $sock = $self->params->{sock}; 
    my $bus_id = $self->params->{bus_id};

    my ($package, $filename, $line) = caller;
    $logger->debug("Redis client connecting with bus_id $bus_id : $filename : $line");

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

    my $packet;

    if ($timeout == 0) {
        # Non-blocking list pop
        $packet = $self->redis->lpop($self->bus_id);

    } else {
        # In Redis, timeout 0 means wait indefinitely
        $packet = $self->redis->blpop($self->bus_id, $timeout == -1 ? 0 : $timeout);
    }

    # Timed out waiting for data.
    return undef unless defined $packet;

    my $json = ref $packet eq 'ARRAY' ? $packet->[1] : $packet;

    $logger->internal("recv() $json");


    my $msg = OpenSRF::Transport::Redis::Message->new(json => $json);

    return undef unless $msg;

    $logger->internal("recv() thread=" . $msg->thread);

    # TODO
    # The upper layers assume the body is a JSON string.
    # Teach the upper layers to treat the body as a part of the 
    # JSON object instead and we can avoid an extra round of
    # JSON encode/decode with each message
    $msg->body(OpenSRF::Utils::JSON->perl2JSON($msg->body));

    return $msg;
}


sub flush_socket {
    my $self = shift;
    return 0 unless $self->redis;
    # Remove any messages directed to me from the bus.
    $self->redis->del($self->bus_id);
    return 1;
}

1;


