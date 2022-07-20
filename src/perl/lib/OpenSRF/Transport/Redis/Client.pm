package OpenSRF::Transport::Redis::Client;
use strict;
use warnings;
use Redis;
use Time::HiRes q/time/;
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
    return unless $self->redis;

    if ($self->stream_name =~ /^client:/) {
        # Delete our stream since we're the only one using it.  Deleting
        # the stream also deletes our consumer group.
        $self->redis->del($self->stream_name);
    }

    $self->redis->quit;
    delete $self->{redis};
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

sub initialize {
    my $self = shift;

    my $host = $self->params->{host} || ''; 
    my $port = $self->params->{port} || 0; 
    my $sock = $self->params->{sock} || ''; 
    my $username = $self->params->{username}; 
    my $password = $self->params->{password}; 
    my $stream_name = $self->params->{stream_name};
    my $consumer_name = $self->params->{consumer_name};
    my $max_queue_size = $self->params->{max_queue_size};

    $logger->debug("Redis client connecting: ".
        "host=$host port=$port sock=$sock username=$username stream_name=$stream_name");

    return 1 if $self->redis; # already connected

    # UNIX socket file takes precedence over host:port.
    my @connect_args = $sock ? (sock => $sock) : (server => "$host:$port");

    # On disconnect, try to reconnect every 100ms up to 60 seconds.
    push(@connect_args, (reconnect => 60, every => 100_000));

    $logger->debug("Connecting to bus: @connect_args");

    unless ($self->redis(Redis->new(@connect_args))) {
        throw OpenSRF::EX::Jabber("Could not connect to Redis bus with @connect_args");
        return 0;
    }

    unless ($self->redis->auth($username, $password) eq 'OK') {
        throw OpenSRF::EX::Jabber("Cannot authenticate with Redis instance user=$username");
        return 0;
    }

    $logger->debug("Auth'ed with Redis as $username OK : stream_name=$stream_name");

    eval { 
        # This gets mad when a stream / group already exists, but 
        # Listeners share a stream/group name so dupes are possible.

        $self->redis->xgroup(   
            'create',
            $stream_name,   # stream name
            $stream_name,   # group name
            '$',            # only receive new messages
            'mkstream'      # create this stream if it's not there.
        );
    };

    if ($@) {
        $logger->info("XGROUP CREATE returned : $@");
    }

    $self->stream_name($stream_name);
    $self->consumer_name($consumer_name);
    $self->max_queue_size($max_queue_size);

    return $self;
}

sub max_queue_size {
    my ($self, $max_queue_size) = @_;
    $self->{max_queue_size} = $max_queue_size if $max_queue_size;
    return $self->{max_queue_size};
}

sub stream_name {
    my ($self, $stream_name) = @_;
    $self->{stream_name} = $stream_name if $stream_name;
    return $self->{stream_name};
}

sub consumer_name {
    my ($self, $consumer_name) = @_;
    $self->{consumer_name} = $consumer_name if $consumer_name;
    return $self->{consumer_name};
}


sub construct {
    my ($class, $app, $context) = @_;
    $class->peer_handle($class->new($app, $context)->initialize);
}

sub send {
    my $self = shift;
    my $msg = OpenSRF::Transport::Redis::Message->new(@_);
    
    $msg->body(OpenSRF::Utils::JSON->JSON2perl($msg->body));

    $msg->osrf_xid($logger->get_osrf_xid);
    $msg->from($self->stream_name);

    my $msg_json = $msg->to_json;

    $logger->internal("send(): to=" . $msg->to . " : $msg_json");

    $self->redis->xadd(
        $msg->to,                   # recipient == stream name
        'NOMKSTREAM',
        'MAXLEN', 
        '~',                        # maxlen-ish
        $self->max_queue_size,
        '*',                        # let Redis generate the ID
        'message',                  # gotta call it something 
        $msg_json
    );
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

    $logger->debug("server: watching for content at " . $self->stream_name);

    my @block;
    if ($timeout) {
        # 0 means block indefinitely in Redis
        $timeout = 0 if $timeout == -1;
        $timeout *= 1000; # milliseconds
        @block = (BLOCK => $timeout);
    }

    my $packet = $self->redis->xreadgroup(
        GROUP => $self->stream_name,
        $self->consumer_name,
        @block,
        COUNT => 1,
        STREAMS => $self->stream_name,
        '>' # new messages only
    );      

    # Timed out waiting for data.
    return undef unless defined $packet;

    # TODO make this more self-documenting.  also, too brittle?
    my $container = $packet->[0]->[1]->[0];
    my $msg_id = $container->[0];
    my $json = $container->[1]->[1];

    $logger->internal("recv() $json");

    # TODO putting this here for now -- it may live somewhere else.
    # Ideally this could happen out of band.
    # Note if we don't ACK utnil after successfully processing each
    # message, a malformed message will stay in the pending list.
    # Consider options.
    $self->redis->xack($self->stream_name, $self->stream_name, $msg_id);

    my $msg = OpenSRF::Transport::Redis::Message->new(json => $json);
    $msg->msg_id($msg_id);

    return undef unless $msg;

    $logger->internal("recv() thread=" . $msg->thread);

    # The message body is doubly encoded as JSON to, among other things,
    # support message chunking.
    $msg->body(OpenSRF::Utils::JSON->perl2JSON($msg->body));

    return $msg;
}


sub flush_socket {
    my $self = shift;
    # Remove all messages from the stream
    $self->redis->xtrim($self->stream_name, 'MAXLEN', 0);
    return 1;
}

1;


